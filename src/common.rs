//! Types shared by several products.

use serde::{Deserialize, Serialize};

/// A list of HTTP headers, each a `(name, value)` pair. Serialized as `[["name", "value"], ...]`.
pub type Headers = Vec<(String, String)>;

/// General information about a scanned document, returned by AI Text Detection,
/// AI Image Detection, Grammar Checker and Text Moderation. Missing fields are `None`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct ScannedDocument {
    /// The scan id you provided.
    pub scan_id: Option<String>,
    /// Total number of words found in the scanned text.
    pub total_words: Option<i64>,
    /// Number of excluded words in the scanned text.
    pub total_excluded: Option<i64>,
    /// The number of credits spent on the scan.
    pub actual_credits: Option<i64>,
    /// The number of credits the scan was expected to cost.
    pub expected_credits: Option<i64>,
    /// When the scan was created, as an ISO 8601 time (UTC).
    pub creation_time: Option<String>,
}

/// Where Copyleaks posts the result of an asynchronous scan: AI Image Detection submitted by URL
/// ([`Client::submit_ai_image_detection`](crate::Client::submit_ai_image_detection)) and AI Video
/// Detection ([`Client::submit_ai_video_detection`](crate::Client::submit_ai_video_detection)).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct ResultWebhook {
    /// The URL that receives the result with a `POST` request.
    pub url: String,
    /// Headers to send with the webhook request, for example an `Authorization` header.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<Headers>,
}

impl ResultWebhook {
    /// Creates the webhook with the URL that receives the result.
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            headers: None,
        }
    }
}

setters!(ResultWebhook { headers: Headers });
