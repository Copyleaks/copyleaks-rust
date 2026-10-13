//! Types shared by several products.

use serde::{Deserialize, Serialize};

/// General information about a scanned document, returned by AI Text Detection,
/// AI Image Detection, Grammar Checker and Text Moderation. Missing fields are `None`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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
