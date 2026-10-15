//! Public reference data: release notes, supported file types, OCR languages and cross-language
//! pairs. These calls need no login.

use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize};

use crate::de;

/// The API release notes, from [`Client::release_notes`](crate::Client::release_notes).
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
/// [`Client::allowed_cross_languages`](crate::Client::allowed_cross_languages). Use the codes in
/// [`CrossLanguages`](crate::plagiarism::CrossLanguages).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct AllowedCrossLanguages {
    /// The languages a scanned document can be in.
    #[serde(default, deserialize_with = "languages")]
    pub document_languages: Vec<CrossLanguage>,
    /// The languages results can be found in.
    #[serde(default, deserialize_with = "languages")]
    pub result_languages: Vec<CrossLanguage>,
}

impl AllowedCrossLanguages {
    /// The codes of [`document_languages`](field@AllowedCrossLanguages::document_languages).
    pub fn document_codes(&self) -> Vec<&str> {
        self.document_languages
            .iter()
            .map(|language| language.code.as_str())
            .collect()
    }

    /// The codes of [`result_languages`](field@AllowedCrossLanguages::result_languages).
    pub fn result_codes(&self) -> Vec<&str> {
        self.result_languages
            .iter()
            .map(|language| language.code.as_str())
            .collect()
    }
}

/// A language of cross-language plagiarism detection.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CrossLanguage {
    /// The language name, for example `english`, when the server sends it.
    pub name: Option<String>,
    /// The ISO 639-1 code, for example `en`.
    pub code: String,
}

/// Reads a list of languages sent as `{"name": "code", ...}` or as `["code", ...]`. A missing or
/// `null` list is empty.
fn languages<'de, D>(deserializer: D) -> Result<Vec<CrossLanguage>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Languages {
        ByName(BTreeMap<String, String>),
        Codes(Vec<Option<String>>),
    }

    Ok(match Option::<Languages>::deserialize(deserializer)? {
        Some(Languages::ByName(map)) => map
            .into_iter()
            .map(|(name, code)| CrossLanguage {
                name: Some(name),
                code,
            })
            .collect(),
        Some(Languages::Codes(codes)) => codes
            .into_iter()
            .flatten()
            .map(|code| CrossLanguage { name: None, code })
            .collect(),
        None => Vec::new(),
    })
}
