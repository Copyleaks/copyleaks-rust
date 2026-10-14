//! Public reference data: release notes, supported file types, OCR languages and cross-language
//! pairs. These calls need no login.

use serde::{Deserialize, Serialize};

use crate::de;

/// The API release notes, from [`Client::release_notes`](crate::Client::release_notes).
/// This endpoint is not part of the API reference, and its content may change.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ReleaseNotes {
    /// The release notes, newest first.
    #[serde(default, deserialize_with = "de::null_as_default")]
    pub updates: Vec<ReleaseNote>,
}

/// A release note.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
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
#[non_exhaustive]
pub struct SupportedFileTypes {
    /// Extensions of text files, for [`Client::submit_file`](crate::Client::submit_file).
    #[serde(default, deserialize_with = "de::null_as_default")]
    pub textual: Vec<String>,
    /// Extensions of image files, for [`Client::submit_file_ocr`](crate::Client::submit_file_ocr).
    #[serde(default, deserialize_with = "de::null_as_default")]
    pub ocr: Vec<String>,
}

/// The file types AI Text Detection accepts in a plagiarism scan, from
/// [`Client::supported_ai_text_detection_file_types`](crate::Client::supported_ai_text_detection_file_types).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct SupportedAiTextFileTypes {
    /// Extensions of the accepted files, for example `pdf` and `docx`.
    #[serde(default, deserialize_with = "de::null_as_default")]
    pub supported_ai_file_types: Vec<String>,
}

/// The languages of cross-language plagiarism detection, from
/// [`Client::allowed_cross_languages`](crate::Client::allowed_cross_languages), as ISO 639-1
/// codes. Use them in [`CrossLanguages`](crate::plagiarism::CrossLanguages).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct AllowedCrossLanguages {
    /// The languages a scanned document can be in.
    #[serde(default, deserialize_with = "de::null_as_default")]
    pub document_languages: Vec<String>,
    /// The languages results can be found in.
    #[serde(default, deserialize_with = "de::null_as_default")]
    pub result_languages: Vec<String>,
}
