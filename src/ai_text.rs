//! AI Text Detection: tell human-written text from AI-generated text.
//!
//! The same [`AiTextDetectionResponse`] is the decoded `additionalData` of the
//! `suspected-ai-text` alert in the plagiarism scan completed webhook; see
//! [`CompletedWebhook::ai_detection_result`](crate::webhooks::CompletedWebhook::ai_detection_result).

use serde::{Deserialize, Serialize};

use crate::{de, validate, Result, ScannedDocument};

/// A text to check, for [`Client::ai_text_detection`](crate::Client::ai_text_detection).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct AiTextDetectionRequest {
    /// The text. From 255 to 100,000 characters (UTF-16 code units).
    pub text: String,
    /// Sandbox mode: free mock results, for integration testing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<bool>,
    /// The ISO 639-1 language code of the text, for example `en`.
    /// Detected automatically when not set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// Return AI Logic explanations in [`AiTextDetectionResponse::explain`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub explain: Option<bool>,
    /// How hard to look for edited AI text, from 1 to 3. The server default is 2.
    /// `1` finds text copied from an LLM without edits, `2` also finds text with minor edits,
    /// and `3` also finds heavily edited text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sensitivity: Option<u8>,
}

impl AiTextDetectionRequest {
    /// Creates a request for this text.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            ..Self::default()
        }
    }

    pub(crate) fn validate(&self) -> Result<()> {
        validate::length(&self.text, "text", 255, 100_000)?;
        if let Some(sensitivity) = self.sensitivity {
            validate::range(u32::from(sensitivity), "sensitivity", 1, 3)?;
        }
        Ok(())
    }
}

setters!(AiTextDetectionRequest {
    sandbox: bool,
    language: String,
    explain: bool,
    sensitivity: u8,
});

/// The result of AI Text Detection. Unknown fields are ignored, missing fields are `None`,
/// and a missing or `null` list is empty.
///
/// It is also the content of the AI Text Detection report of a plagiarism scan, uploaded by
/// [`Export::ai_detection`](field@crate::plagiarism::Export::ai_detection).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct AiTextDetectionResponse {
    /// The version of the detection model, for example `v11.0`.
    #[serde(default, deserialize_with = "de::opt_string_or_number")]
    pub model_version: Option<String>,
    /// The classified sections of the text.
    #[serde(default, deserialize_with = "de::null_as_default")]
    pub results: Vec<AiTextDetectionResult>,
    /// The human and AI shares of the text.
    pub summary: Option<AiTextDetectionSummary>,
    /// The machine translation provider used before the detection.
    /// `0` means the text was not translated.
    pub translation_provider: Option<i32>,
    /// The machine-translated English text. Present only when the text was translated.
    pub translation: Option<String>,
    /// AI Logic explanations. Present only when explanations were requested.
    pub explain: Option<AiTextDetectionExplain>,
    /// General information about the scanned document.
    pub scanned_document: Option<ScannedDocument>,
    /// `"sandbox"` for sandbox scans. Not sent for regular scans.
    pub scan_type: Option<String>,
}

/// A classified section of the text.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
#[allow(deprecated)]
pub struct AiTextDetectionResult {
    /// `1` for human-written text, `2` for AI-generated text.
    pub classification: Option<i32>,
    /// The confidence of the classification. Deprecated by the API and may be missing.
    #[deprecated(note = "deprecated by the API, which may stop sending it; use `classification`")]
    pub probability: Option<f64>,
    /// Where the section is in the scanned document.
    #[serde(default, deserialize_with = "de::null_as_default")]
    pub matches: Vec<AiTextDetectionMatch>,
}

/// Where a classified section is in the scanned document.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AiTextDetectionMatch {
    /// Positions in the plain text.
    pub text: Option<TextPositions>,
    /// Positions in the HTML source. Present only for HTML submissions.
    pub html: Option<TextPositions>,
}

/// Character and word positions.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct TextPositions {
    /// Character positions.
    pub chars: Option<Ranges>,
    /// Word positions.
    pub words: Option<Ranges>,
}

/// Matched segments as parallel lists: segment `i` starts at `starts[i]` and is `lengths[i]` long.
///
/// Also accepts the `Starts` and `Lengths` keys that sandbox scans send.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Ranges {
    /// Start positions of the segments.
    #[serde(default, alias = "Starts", deserialize_with = "de::null_as_default")]
    pub starts: Vec<i64>,
    /// Lengths of the segments.
    #[serde(default, alias = "Lengths", deserialize_with = "de::null_as_default")]
    pub lengths: Vec<i64>,
    /// HTML group ids of the segments. Present only for HTML positions.
    pub group_ids: Option<Vec<i64>>,
}

/// The human and AI shares of the text, each from 0 to 1.
///
/// Also accepts the `Human` and `Ai` keys that sandbox scans send.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AiTextDetectionSummary {
    /// The share of words classified as human-written.
    #[serde(alias = "Human")]
    pub human: Option<f64>,
    /// The AI score: the share of words classified as AI-generated.
    #[serde(alias = "Ai")]
    pub ai: Option<f64>,
}

/// AI Logic explanations of the detection.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AiTextDetectionExplain {
    /// The AI patterns found in the text.
    pub patterns: Option<AiTextPatterns>,
}

/// AI patterns found in the text, with their statistics and positions.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AiTextPatterns {
    /// Statistics of each pattern.
    pub statistics: Option<AiTextPatternStatistics>,
    /// Positions of the patterns in the plain text.
    pub text: Option<TextPositions>,
    /// Positions of the patterns in the HTML source. Present only for HTML submissions.
    pub html: Option<TextPositions>,
}

/// Statistics of the AI patterns, as parallel lists with one entry per pattern.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct AiTextPatternStatistics {
    /// How often each pattern appears in AI-generated text, per million texts.
    #[serde(default, deserialize_with = "de::null_as_default")]
    pub ai_count: Vec<f64>,
    /// How often each pattern appears in human-written text, per million texts.
    #[serde(default, deserialize_with = "de::null_as_default")]
    pub human_count: Vec<f64>,
    /// The ratio between the AI and the human frequency of each pattern.
    #[serde(default, deserialize_with = "de::null_as_default")]
    pub proportion: Vec<f64>,
    /// The source of each pattern: `1` for AI, `2` for a humanizer.
    #[serde(default, deserialize_with = "de::null_as_default")]
    pub source: Vec<i32>,
}

/// The AI Text Detection credit balance of your account, from
/// [`Client::ai_text_detection_credits`](crate::Client::ai_text_detection_credits).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AiTextDetectionCredits {
    /// The credits left for AI Text Detection.
    #[serde(default, deserialize_with = "de::null_as_default")]
    pub credits: i64,
}
