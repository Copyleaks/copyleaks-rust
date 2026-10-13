//! Text Moderation: flag hateful, explicit, toxic or otherwise risky text.

use serde::{Deserialize, Serialize};

use crate::{de, validate, Result, ScannedDocument};

/// The moderation labels. Send the ones you want in [`TextModerationRequest::labels`].
pub mod labels {
    /// Explicit sexual content.
    pub const ADULT_V1: &str = "adult-v1";
    /// Harmful language that insults, demeans or degrades.
    pub const TOXIC_V1: &str = "toxic-v1";
    /// Language that incites or glorifies physical harm.
    pub const VIOLENT_V1: &str = "violent-v1";
    /// Strong or offensive swear words.
    pub const PROFANITY_V1: &str = "profanity-v1";
    /// Content that encourages or normalizes self-harm.
    pub const SELF_HARM_V1: &str = "self-harm-v1";
    /// Targeted abuse of a specific person or group.
    pub const HARASSMENT_V1: &str = "harassment-v1";
    /// Language that demonizes or incites harm toward a group.
    pub const HATE_SPEECH_V1: &str = "hate-speech-v1";
    /// Use, abuse or distribution of drugs.
    pub const DRUGS_V1: &str = "drugs-v1";
    /// Use, possession or distribution of weapons.
    pub const FIREARMS_V1: &str = "firearms-v1";
    /// Hacking, data breaches and unauthorized access.
    pub const CYBERSECURITY_V1: &str = "cybersecurity-v1";

    /// Every label above.
    pub const ALL: [&str; 10] = [
        ADULT_V1,
        TOXIC_V1,
        VIOLENT_V1,
        PROFANITY_V1,
        SELF_HARM_V1,
        HARASSMENT_V1,
        HATE_SPEECH_V1,
        DRUGS_V1,
        FIREARMS_V1,
        CYBERSECURITY_V1,
    ];
}

/// English, the only language Text Moderation supports.
pub const LANGUAGE_ENGLISH: &str = "en";

/// A text to moderate, for [`Client::text_moderation`](crate::Client::text_moderation).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextModerationRequest {
    /// The text. From 2 to 25,000 characters.
    pub text: String,
    /// Sandbox mode: free mock results, for integration testing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<bool>,
    /// The language code of the text. Only [`LANGUAGE_ENGLISH`] is supported.
    /// Detected automatically when not set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// The labels to check, from 1 to 16. In the response, a label is referred to by its
    /// position in this list.
    pub labels: Vec<TextModerationLabel>,
}

impl TextModerationRequest {
    /// Creates a request for this text and these label ids, for example [`labels::ALL`].
    pub fn new<S: Into<String>>(
        text: impl Into<String>,
        labels: impl IntoIterator<Item = S>,
    ) -> Self {
        Self {
            text: text.into(),
            sandbox: None,
            language: None,
            labels: labels.into_iter().map(TextModerationLabel::new).collect(),
        }
    }

    pub(crate) fn validate(&self) -> Result<()> {
        validate::length(&self.text, "text", 2, 25_000)?;
        validate::count(self.labels.len(), "labels", 1, 16)?;
        for label in &self.labels {
            validate::length(&label.id, "labels[].id", 1, 32)?;
        }
        Ok(())
    }
}

setters!(TextModerationRequest {
    sandbox: bool,
    language: String,
});

/// A moderation label to check.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct TextModerationLabel {
    /// The label id, for example [`labels::TOXIC_V1`].
    pub id: String,
}

impl TextModerationLabel {
    /// Creates a label.
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }
}

/// The result of Text Moderation. Missing fields are `None`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextModerationResponse {
    /// The version of the moderation model.
    #[serde(default, deserialize_with = "de::opt_string_or_number")]
    pub model_version: Option<String>,
    /// The labelled segments of the text.
    pub moderations: Option<Moderations>,
    /// Maps the label indexes in [`ModerationRanges::labels`] to label ids.
    #[serde(default, deserialize_with = "de::null_as_default")]
    pub legend: Vec<LegendEntry>,
    /// General information about the scanned document.
    pub scanned_document: Option<ScannedDocument>,
}

/// The labelled segments of the text.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Moderations {
    /// Segments of the plain text.
    pub text: Option<ModeratedText>,
}

/// Segments of the plain text.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModeratedText {
    /// Character positions of the segments.
    pub chars: Option<ModerationRanges>,
}

/// Labelled segments as parallel lists: segment `i` starts at `starts[i]`, is `lengths[i]`
/// characters long, and has the label at index `labels[i]` of the legend.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModerationRanges {
    /// The label index of each segment. Resolve it with [`TextModerationResponse::legend`].
    #[serde(default, deserialize_with = "de::null_as_default")]
    pub labels: Vec<i32>,
    /// Start positions of the segments.
    #[serde(default, deserialize_with = "de::null_as_default")]
    pub starts: Vec<i64>,
    /// Lengths of the segments.
    #[serde(default, deserialize_with = "de::null_as_default")]
    pub lengths: Vec<i64>,
}

/// A legend entry: a label index and its id.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LegendEntry {
    /// The label index used in [`ModerationRanges::labels`].
    pub index: Option<i32>,
    /// The label id, for example `toxic-v1`.
    pub id: Option<String>,
}
