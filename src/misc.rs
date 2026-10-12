//! Public reference data: release notes, supported file types and OCR languages.
//! These calls need no login.

use serde::{Deserialize, Serialize};

use crate::de;

/// The API release notes, from [`Client::release_notes`](crate::Client::release_notes).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseNotes {
    /// The release notes, newest first.
    #[serde(default, deserialize_with = "de::null_as_default")]
    pub updates: Vec<ReleaseNote>,
}

/// A release note.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseNote {
    /// The kind of note, for example `NEW`.
    pub msg_type: Option<String>,
    /// When the change was released, as an ISO 8601 time.
    pub date: Option<String>,
    /// The note, as HTML.
    pub html_description: Option<String>,
}

/// The file types the Plagiarism Checker accepts, from
/// [`Client::supported_file_types`](crate::Client::supported_file_types).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SupportedFileTypes {
    /// Extensions of text files, for [`Client::submit_file`](crate::Client::submit_file).
    #[serde(default, deserialize_with = "de::null_as_default")]
    pub textual: Vec<String>,
    /// Extensions of image files, for [`Client::submit_file_ocr`](crate::Client::submit_file_ocr).
    #[serde(default, deserialize_with = "de::null_as_default")]
    pub ocr: Vec<String>,
}
