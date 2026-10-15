//! Webhook payloads of plagiarism scans. Parse any webhook body or exported file with [`parse`].
//!
//! The status webhook gets [`CompletedWebhook`], [`ErrorWebhook`], [`CreditsCheckedWebhook`] or
//! [`IndexedWebhook`] by its `{STATUS}`, the new result webhook [`NewResultWebhook`], and the
//! export completion webhook [`ExportCompletedWebhook`]. AI Image Detection by URL and AI Video
//! Detection post an [`AiImageDetectionResponse`](crate::ai_image::AiImageDetectionResponse) and
//! an [`AiVideoDetectionResponse`](crate::ai_video::AiVideoDetectionResponse).
//!
//! ```
//! use copyleaks::webhooks::{self, CompletedWebhook};
//!
//! # fn handle(body: &[u8]) -> Result<(), copyleaks::Error> {
//! let completed: CompletedWebhook = webhooks::parse(body)?;
//! if let Some(ai) = completed.ai_detection_result()? {
//!     println!("AI score: {:?}", ai.summary.and_then(|summary| summary.ai));
//! }
//! # Ok(())
//! # }
//! ```

use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};

use crate::ai_text::AiTextDetectionResponse;
use crate::exports::ReferencesValidation;
use crate::grammar::{Readability, ScoreCorrections, TextStatistics};
use crate::{de, Result};

/// Scan alert codes, found in [`Alert::code`].
pub mod alert_codes {
    /// AI-generated text was detected. The alert's `additionalData` holds the AI Text Detection
    /// result; decode it with [`Alert::ai_detection_result`](super::Alert::ai_detection_result).
    pub const SUSPECTED_AI_TEXT: &str = "suspected-ai-text";
}

/// Parses a webhook body, or an exported file, into one of the payload types of the SDK.
///
/// # Errors
///
/// [`Error::Json`](crate::Error::Json) when the body is not valid JSON or does not match the type.
pub fn parse<T: DeserializeOwned>(body: &[u8]) -> Result<T> {
    Ok(serde_json::from_slice(body)?)
}

/// The `completed` status webhook: the scan finished successfully.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct CompletedWebhook {
    /// The scan status: `0` success, `1` error, `2` credits checked, `3` indexed.
    pub status: Option<i32>,
    /// The developer payload you sent with the submit.
    pub developer_payload: Option<String>,
    /// General information about the scanned document.
    pub scanned_document: Option<WebhookScannedDocument>,
    /// The results, grouped by source, and the aggregated score.
    pub results: Option<Results>,
    /// The Citation Checker summary, when the scan was submitted with `references.validate`.
    /// The result of each reference is in the exported
    /// [`CrawledVersion`](crate::exports::CrawledVersion).
    pub references_validation: Option<ReferencesValidation>,
    /// The Grammar Checker scores, when the scan was submitted with `writingFeedback.enable`.
    pub writing_feedback: Option<WebhookWritingFeedback>,
    /// Scan alerts.
    pub notifications: Option<Notifications>,
}

/// The Grammar Checker scores of a plagiarism scan, in the completed webhook. Export the
/// corrections with [`Export::writing_feedback`](field@crate::plagiarism::Export::writing_feedback).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct WebhookWritingFeedback {
    /// Statistics of the text.
    pub text_statistics: Option<TextStatistics>,
    /// Correction counts, scores and weights per category, and the overall score.
    pub score: Option<ScoreCorrections>,
    /// Readability of the text.
    pub readability: Option<Readability>,
}

impl CompletedWebhook {
    /// Returns the first `suspected-ai-text` alert, or `None` when the scan has none.
    pub fn ai_detection_alert(&self) -> Option<&Alert> {
        self.notifications
            .as_ref()?
            .alerts
            .iter()
            .find(|alert| alert.code.as_deref() == Some(alert_codes::SUSPECTED_AI_TEXT))
    }

    /// The AI Text Detection result of the scan: [`Alert::ai_detection_result`] of the
    /// [`ai_detection_alert`](Self::ai_detection_alert), or `Ok(None)` without that alert. Use
    /// `ai_detection_alert` to tell the `Ok(None)` cases apart.
    ///
    /// # Errors
    ///
    /// The same as [`Alert::ai_detection_result`].
    pub fn ai_detection_result(&self) -> Result<Option<AiTextDetectionResponse>> {
        match self.ai_detection_alert() {
            Some(alert) => alert.ai_detection_result(),
            None => Ok(None),
        }
    }
}

/// The `error` status webhook: the scan ended with an error.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct ErrorWebhook {
    /// The scan status: `1` for error.
    pub status: Option<i32>,
    /// The developer payload you sent with the submit.
    pub developer_payload: Option<String>,
    /// Why the scan failed.
    pub error: Option<WebhookError>,
}

/// Why a scan failed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct WebhookError {
    /// The error code. See <https://docs.copyleaks.com/using-the-apis/api-errors>.
    pub code: Option<i32>,
    /// A human-readable message.
    pub message: Option<String>,
    /// The error type, for example `invalid_request_error`.
    #[serde(rename = "type")]
    pub error_type: Option<String>,
    /// The machine-readable error id, for example `insufficient_credits`.
    pub id: Option<String>,
    /// A link to the documentation of this error.
    pub url: Option<String>,
    /// The parameters that caused the error.
    #[serde(default, deserialize_with = "de::vec_skip_nulls")]
    pub details: Vec<WebhookErrorDetail>,
}

/// A parameter that caused an error.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct WebhookErrorDetail {
    /// The parameter name.
    pub param: Option<String>,
    /// What is wrong with the parameter.
    pub message: Option<String>,
}

/// The `creditschecked` status webhook: the scan was submitted with
/// [`SubmitAction::CheckCredits`](crate::plagiarism::SubmitAction::CheckCredits) and its price is
/// known. Start it with [`Client::start`](crate::Client::start).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct CreditsCheckedWebhook {
    /// The scan status: `2` for credits checked.
    pub status: Option<i32>,
    /// The developer payload you sent with the submit.
    pub developer_payload: Option<String>,
    /// The price of the scan, in credits.
    pub credits: Option<i64>,
    /// General information about the scanned document.
    pub scanned_document: Option<WebhookScannedDocument>,
}

/// The `indexed` status webhook: the document was indexed without a scan.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct IndexedWebhook {
    /// The scan status: `3` for indexed.
    pub status: Option<i32>,
    /// The developer payload you sent with the submit.
    pub developer_payload: Option<String>,
}

/// The export completion webhook, sent when an [`Export`](crate::plagiarism::Export) is done.
///
/// The export succeeded when `completed` is `true` and every task is healthy with a 2xx status.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct ExportCompletedWebhook {
    /// Whether the export finished without internal errors on the Copyleaks side.
    #[serde(default, deserialize_with = "de::null_as_default")]
    pub completed: bool,
    /// The developer payload you sent with the export.
    pub developer_payload: Option<String>,
    /// The upload tasks.
    #[serde(default, deserialize_with = "de::vec_skip_nulls")]
    pub tasks: Vec<ExportTask>,
}

/// An upload task of an export.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct ExportTask {
    /// The URL the data was uploaded to.
    pub endpoint: Option<String>,
    /// Whether the upload finished without errors.
    #[serde(default, deserialize_with = "de::null_as_default")]
    pub is_healthy: bool,
    /// The HTTP status code your server returned for the upload.
    pub http_status_code: Option<i32>,
}

/// The new result webhook, sent while the scan is still running each time new results are found.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct NewResultWebhook {
    /// The developer payload you sent with the submit.
    pub developer_payload: Option<String>,
    /// The score of the scan so far.
    #[serde(default, deserialize_with = "score_object_or_number")]
    pub score: Option<Score>,
    /// New results from the internet.
    #[serde(default, deserialize_with = "de::vec_skip_nulls")]
    pub internet: Vec<InternetResult>,
    /// New results from the Shared Data Hub.
    #[serde(default, deserialize_with = "de::vec_skip_nulls")]
    pub database: Vec<DatabaseResult>,
    /// New results from your batch.
    #[serde(default, deserialize_with = "de::vec_skip_nulls")]
    pub batch: Vec<DatabaseResult>,
    /// New results from Private Cloud Hubs.
    #[serde(default, deserialize_with = "de::vec_skip_nulls")]
    pub repositories: Vec<RepositoryResult>,
    /// New matches in the Copyleaks internal database of AI-generated text.
    #[serde(
        rename = "internalAIData",
        default,
        deserialize_with = "de::vec_skip_nulls"
    )]
    pub internal_ai_data: Vec<InternalAiResult>,
}

/// Reads the new result score, which the server sends as a score object and the API docs show
/// as a bare number (the aggregated score).
fn score_object_or_number<'de, D>(deserializer: D) -> Result<Option<Score>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum ScoreOrNumber {
        Score(Score),
        Number(f64),
    }

    Ok(
        Option::<ScoreOrNumber>::deserialize(deserializer)?.map(|value| match value {
            ScoreOrNumber::Score(score) => score,
            ScoreOrNumber::Number(aggregated_score) => Score {
                aggregated_score: Some(aggregated_score),
                ..Score::default()
            },
        }),
    )
}

/// General information about the scanned document, in a status webhook.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct WebhookScannedDocument {
    /// The scan id you provided.
    pub scan_id: Option<String>,
    /// Total number of words found in the scanned text.
    pub total_words: Option<i64>,
    /// Number of excluded words in the scanned text.
    pub total_excluded: Option<i64>,
    /// Credits used for the scan.
    pub credits: Option<i64>,
    /// The number of credits the scan was expected to cost.
    pub expected_credits: Option<i64>,
    /// When the scan was created, as an ISO 8601 time (UTC).
    pub creation_time: Option<String>,
    /// Metadata of the scanned document.
    pub metadata: Option<Metadata>,
    /// The features that ran on the scan.
    pub enabled: Option<EnabledFeatures>,
    /// The language detected in the scanned document, for example `en`.
    pub detected_language: Option<String>,
}

/// The features that ran on a scan, in
/// [`WebhookScannedDocument::enabled`](field@WebhookScannedDocument::enabled).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct EnabledFeatures {
    /// Plagiarism detection.
    pub plagiarism_detection: Option<bool>,
    /// AI Text Detection.
    pub ai_detection: Option<bool>,
    /// AI Logic explanations.
    pub explainable_ai: Option<bool>,
    /// Grammar Checker.
    pub writing_feedback: Option<bool>,
    /// The PDF report.
    pub pdf_report: Option<bool>,
    /// Cheat detection.
    pub cheat_detection: Option<bool>,
    /// Gen-AI Overview.
    pub overview: Option<bool>,
    /// Citation Checker.
    pub references_validation: Option<bool>,
    /// AI Source Match.
    pub ai_source_match: Option<bool>,
    /// Internal AI Source Match.
    pub internal_ai_source_match: Option<bool>,
}

/// Metadata of a document.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Metadata {
    /// The URL after all HTTP redirects.
    pub final_url: Option<String>,
    /// The canonical URL of the document.
    pub canonical_url: Option<String>,
    /// When the document was published.
    pub publish_date: Option<String>,
    /// When the document was created.
    pub creation_date: Option<String>,
    /// When the document was last modified.
    pub last_modification_date: Option<String>,
    /// The author of the document.
    pub author: Option<String>,
    /// The organization of the document.
    pub organization: Option<String>,
    /// The file name of the document.
    pub filename: Option<String>,
    /// For Private Cloud Hub results: the email of the user who added the document.
    pub submitted_by: Option<String>,
}

/// The results of a completed scan, grouped by source.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Results {
    /// Results from the internet.
    #[serde(default, deserialize_with = "de::vec_skip_nulls")]
    pub internet: Vec<InternetResult>,
    /// Results from the Shared Data Hub.
    #[serde(default, deserialize_with = "de::vec_skip_nulls")]
    pub database: Vec<DatabaseResult>,
    /// Results from your batch.
    #[serde(default, deserialize_with = "de::vec_skip_nulls")]
    pub batch: Vec<DatabaseResult>,
    /// Results from Private Cloud Hubs.
    #[serde(default, deserialize_with = "de::vec_skip_nulls")]
    pub repositories: Vec<RepositoryResult>,
    /// Matches in the Copyleaks internal database of AI-generated text, when the scan was
    /// submitted with `internalAiSourceMatch.enable`.
    #[serde(
        rename = "internalAIData",
        default,
        deserialize_with = "de::vec_skip_nulls"
    )]
    pub internal_ai_data: Vec<InternalAiResult>,
    /// The aggregated score of all results.
    pub score: Option<Score>,
}

/// A match in the Copyleaks internal database of AI-generated text.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct InternalAiResult {
    /// A short id of the match.
    pub id: Option<String>,
    /// The title of the matched entry.
    pub title: Option<String>,
    /// An excerpt of the matched content.
    pub introduction: Option<String>,
    /// Total matched words.
    pub matched_words: Option<i64>,
    /// Identical words.
    pub identical_words: Option<i64>,
    /// Similar but not identical words.
    pub similar_words: Option<i64>,
    /// Paraphrased words.
    pub paraphrased_words: Option<i64>,
    /// The number of words in the matched entry.
    pub total_words: Option<i64>,
    /// Result tags.
    #[serde(default, deserialize_with = "de::vec_skip_nulls")]
    pub tags: Vec<Tag>,
}

/// A result from the internet.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct InternetResult {
    /// The result id. Use it to export the result.
    pub id: Option<String>,
    /// The document title.
    pub title: Option<String>,
    /// A short introduction of the document.
    pub introduction: Option<String>,
    /// Total matched words between this result and the scanned document.
    pub matched_words: Option<i64>,
    /// The public URL of the source.
    pub url: Option<String>,
    /// Metadata of the source.
    pub metadata: Option<Metadata>,
    /// Result tags.
    #[serde(default, deserialize_with = "de::vec_skip_nulls")]
    pub tags: Vec<Tag>,
}

/// A result from the Shared Data Hub or from your batch.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct DatabaseResult {
    /// The result id. Use it to export the result.
    pub id: Option<String>,
    /// The document title.
    pub title: Option<String>,
    /// A short introduction of the document.
    pub introduction: Option<String>,
    /// Total matched words between this result and the scanned document.
    pub matched_words: Option<i64>,
    /// The scan id of the matched document, when you submitted it.
    pub scan_id: Option<String>,
    /// Metadata of the source.
    pub metadata: Option<Metadata>,
    /// Result tags.
    #[serde(default, deserialize_with = "de::vec_skip_nulls")]
    pub tags: Vec<Tag>,
}

/// A result from a Private Cloud Hub.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct RepositoryResult {
    /// The result id. Use it to export the result.
    pub id: Option<String>,
    /// The document title.
    pub title: Option<String>,
    /// A short introduction of the document.
    pub introduction: Option<String>,
    /// Total matched words between this result and the scanned document.
    pub matched_words: Option<i64>,
    /// The repository id of the hub that has the result.
    pub repository_id: Option<String>,
    /// The scan id of the matched document in the hub.
    pub scan_id: Option<String>,
    /// Metadata of the source, including who submitted it.
    pub metadata: Option<Metadata>,
    /// Result tags.
    #[serde(default, deserialize_with = "de::vec_skip_nulls")]
    pub tags: Vec<Tag>,
}

/// A result tag.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Tag {
    /// The tag type id.
    pub code: Option<String>,
    /// A short name of the tag.
    pub title: Option<String>,
    /// What the tag means.
    pub description: Option<String>,
}

/// A plagiarism score.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Score {
    /// Number of identical words.
    pub identical_words: Option<i64>,
    /// Number of nearly identical words, such as "slow" and "slowly".
    pub minor_changed_words: Option<i64>,
    /// Number of paraphrased words.
    pub related_meaning_words: Option<i64>,
    /// The percentage of similar words. Excluded text, such as quotes, is not counted.
    pub aggregated_score: Option<f64>,
}

/// Scan alerts of a completed scan.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Notifications {
    /// The scan alerts. `null` entries are skipped.
    #[serde(default, deserialize_with = "de::vec_skip_nulls")]
    pub alerts: Vec<Alert>,
}

/// A scan alert.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Alert {
    /// The alert category. `2` is AI content detection.
    pub category: Option<i32>,
    /// The alert code, unique per kind of alert. See [`alert_codes`].
    pub code: Option<String>,
    /// A human-readable title.
    pub title: Option<String>,
    /// A human-readable message.
    pub message: Option<String>,
    /// A link to a page about this alert. Not sent for every alert.
    pub help_link: Option<String>,
    /// The importance, from `0` (very low) to `4` (very high).
    pub severity: Option<i32>,
    /// More data about the alert, as a JSON string.
    pub additional_data: Option<String>,
}

impl Alert {
    /// Decodes the `additional_data` of a `suspected-ai-text` alert into the AI Text Detection
    /// result, ignoring trailing NUL characters and ASCII whitespace. `Ok(None)` for another
    /// alert code, missing or empty data, or valid JSON that is not an object.
    ///
    /// # Errors
    ///
    /// [`Error::Json`](crate::Error::Json) when the data is not valid JSON, or a field has a type
    /// the result cannot accept.
    pub fn ai_detection_result(&self) -> Result<Option<AiTextDetectionResponse>> {
        if self.code.as_deref() != Some(alert_codes::SUSPECTED_AI_TEXT) {
            return Ok(None);
        }
        let data = self
            .additional_data
            .as_deref()
            .unwrap_or_default()
            .trim_end_matches(is_trailing_padding);
        if data.is_empty() {
            return Ok(None);
        }
        // Decode an object straight into the result. Anything else is checked to be valid JSON,
        // without building it, and gives no result.
        if data.trim_start().starts_with('{') {
            return Ok(Some(serde_json::from_str(data)?));
        }
        serde_json::from_str::<serde::de::IgnoredAny>(data)?;
        Ok(None)
    }
}

/// NUL, and the ASCII whitespace: tab, line feed, vertical tab, form feed, carriage return
/// and space.
fn is_trailing_padding(c: char) -> bool {
    matches!(c, '\0' | '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | ' ')
}
