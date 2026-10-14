//! The files that an [`Export`](crate::plagiarism::Export) uploads to your servers.
//!
//! Each export target receives one JSON file. Parse it with [`webhooks::parse`]:
//!
//! | Export target | File type |
//! |---|---|
//! | [`results`](field@crate::plagiarism::Export::results) | [`ExportedResult`], one per result |
//! | [`crawled_version`](field@crate::plagiarism::Export::crawled_version) | [`CrawledVersion`] |
//! | [`ai_detection`](field@crate::plagiarism::Export::ai_detection) | [`AiTextDetectionResponse`](crate::ai_text::AiTextDetectionResponse) |
//! | [`writing_feedback`](field@crate::plagiarism::Export::writing_feedback) | [`GrammarCheckResponse`](crate::grammar::GrammarCheckResponse) |
//! | [`overview`](field@crate::plagiarism::Export::overview) | [`AiOverview`] |
//! | [`pdf_report`](field@crate::plagiarism::Export::pdf_report) | the PDF file itself |
//!
//! Unknown fields are ignored, missing fields are `None`, and a missing or `null` list is empty.
//!
//! [`webhooks::parse`]: crate::webhooks::parse

use serde::{Deserialize, Deserializer, Serialize};

use crate::ai_text::{Ranges, TextPositions};
use crate::de;

/// A result of a plagiarism scan: the matched source and where it matches the scanned document.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ExportedResult {
    /// The number of matched words of each kind.
    pub statistics: Option<ResultStatistics>,
    /// The source text and the matches in it.
    pub text: Option<ResultText>,
    /// The source HTML and the matches in it. Present only when the scan included HTML.
    pub html: Option<ResultHtml>,
}

/// The number of matched words of each kind.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct ResultStatistics {
    /// Identical words.
    pub identical: Option<i64>,
    /// Nearly identical words, such as "slow" and "slowly".
    pub minor_changes: Option<i64>,
    /// Paraphrased words.
    pub related_meaning: Option<i64>,
}

/// The source text of a result and the matches in it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ResultText {
    /// The text of the source.
    pub value: Option<String>,
    /// Where the pages of the text start.
    pub pages: Option<Pages>,
    /// The matches between the source and the scanned document.
    pub comparison: Option<Comparison>,
}

/// The source HTML of a result and the matches in it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ResultHtml {
    /// The HTML of the source.
    pub value: Option<String>,
    /// The matches between the source and the scanned document.
    pub comparison: Option<Comparison>,
}

/// Where the pages of a text start.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Pages {
    /// The start position of each page; the first is the start of the first page.
    #[serde(default, deserialize_with = "de::null_as_default")]
    pub start_position: Vec<i64>,
}

/// The matches between a source and the scanned document, by kind.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Comparison {
    /// Identical matches.
    pub identical: Option<ComparisonMatches>,
    /// Nearly identical matches.
    pub minor_changes: Option<ComparisonMatches>,
    /// Paraphrased matches.
    pub related_meaning: Option<ComparisonMatches>,
}

/// Matches of one kind: match `i` is at position `i` of both `source` and `suspected`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct ComparisonMatches {
    /// For HTML matches: the group id of each match.
    pub group_id: Option<Vec<i64>>,
    /// The positions in the source.
    pub source: Option<TextPositions>,
    /// The positions in the scanned document.
    pub suspected: Option<TextPositions>,
}

/// The scanned document as Copyleaks read it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct CrawledVersion {
    /// Word counts.
    pub metadata: Option<CrawledMetadata>,
    /// The text of the document and its excluded parts.
    pub text: Option<CrawledText>,
    /// The HTML of the document and its excluded parts. Present only when the scan was
    /// submitted with `includeHtml`.
    pub html: Option<CrawledHtml>,
    /// The Citation Checker result, when the scan was submitted with `references.validate`.
    pub references_validation: Option<ReferencesValidation>,
}

/// Word counts of the scanned document.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CrawledMetadata {
    /// The number of words.
    pub words: Option<i64>,
    /// The number of excluded words.
    pub excluded: Option<i64>,
}

/// The text of the scanned document.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CrawledText {
    /// The text.
    pub value: Option<String>,
    /// The parts that were not scanned.
    pub exclude: Option<ExcludedRanges>,
    /// Where the pages start.
    pub pages: Option<Pages>,
}

/// The HTML of the scanned document.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CrawledHtml {
    /// The HTML.
    pub value: Option<String>,
    /// The parts that were not scanned.
    pub exclude: Option<ExcludedRanges>,
}

/// Parts of a document that were not scanned, as parallel lists: part `i` starts at
/// `starts[i]`, is `lengths[i]` characters long and was excluded for `reasons[i]`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct ExcludedRanges {
    /// Start positions.
    #[serde(default, deserialize_with = "de::null_as_default")]
    pub starts: Vec<i64>,
    /// Lengths, in characters.
    #[serde(default, deserialize_with = "de::null_as_default")]
    pub lengths: Vec<i64>,
    /// Why each part was excluded.
    #[serde(default, deserialize_with = "de::null_as_default")]
    pub reasons: Vec<i32>,
    /// For HTML: the group id of each part.
    pub group_ids: Option<Vec<i64>>,
}

/// The Citation Checker result. The completed webhook carries the summary only; the crawled
/// version also has one result per reference.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ReferencesValidation {
    /// The reference counts.
    pub summary: Option<ReferencesSummary>,
    /// One entry per reference found. Empty in the completed webhook.
    #[serde(default, deserialize_with = "de::vec_skip_nulls")]
    pub results: Vec<ReferenceResult>,
}

/// How many references were found and validated.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct ReferencesSummary {
    /// The number of references found.
    pub total: Option<i64>,
    /// The number of academic references.
    pub academic: Option<i64>,
    /// The number of academic references fully corroborated by the Copyleaks academic
    /// citation index.
    pub academic_validated: Option<i64>,
    /// The number of non-academic references fully corroborated by their cited source.
    pub non_academic_validated: Option<i64>,
}

/// A reference found in the document.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct ReferenceResult {
    /// `academic` or `nonAcademic`.
    #[serde(rename = "type")]
    pub reference_type: Option<String>,
    /// Where the citation is in the text.
    #[serde(default, deserialize_with = "object_or_none")]
    pub text_ranges: Option<Ranges>,
    /// Where the citation is in the HTML report. `None` for plain text scans.
    #[serde(default, deserialize_with = "object_or_none")]
    pub html_ranges: Option<Ranges>,
    /// What Copyleaks read from the citation.
    pub parsed: Option<ParsedReference>,
    /// The sources found for the reference. Empty when none was found.
    #[serde(default, deserialize_with = "de::vec_skip_nulls")]
    pub suggestions: Vec<ReferenceSuggestion>,
}

/// What Copyleaks read from a citation.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ParsedReference {
    /// The cited title.
    pub title: Option<String>,
    /// The cited authors.
    #[serde(default, deserialize_with = "de::vec_skip_nulls")]
    pub authors: Vec<String>,
    /// The cited year.
    pub year: Option<i64>,
    /// The cited journal or publication. May be empty.
    pub journal: Option<String>,
    /// The cited link, for non-academic references.
    pub url: Option<String>,
}

/// A source found for a reference.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct ReferenceSuggestion {
    /// The URL of the source.
    pub url: Option<String>,
    /// The DOI of the source. May be empty.
    pub doi: Option<String>,
    /// The title of the source.
    pub title: Option<String>,
    /// The authors of the source.
    #[serde(default, deserialize_with = "de::vec_skip_nulls")]
    pub authors: Vec<String>,
    /// The year the source was published, when known.
    pub published_year: Option<i64>,
    /// The year the source was last changed, when known.
    pub modified_year: Option<i64>,
    /// The journal or publication of the source. May be empty.
    pub journal: Option<String>,
    /// Which parts of the citation the source corroborates.
    pub signals: Option<ReferenceSignals>,
}

/// Which parts of a citation a source corroborates. A part that was not checked is `None`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ReferenceSignals {
    /// Whether the title matches.
    pub title: Option<bool>,
    /// Whether the cited year equals the published or the modified year.
    pub year: Option<bool>,
    /// Whether the authors match.
    pub authors: Option<bool>,
}

/// The Gen-AI Overview of a scan.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct AiOverview {
    /// The overview, as Markdown.
    pub overview: Option<String>,
    /// The version of the model that wrote the overview.
    #[serde(default, deserialize_with = "de::opt_string_or_number")]
    pub model_version: Option<String>,
}

/// Reads an object, and anything else (such as the `[]` sent for "no ranges") as `None`.
fn object_or_none<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: serde::de::DeserializeOwned,
{
    match Option::<serde_json::Value>::deserialize(deserializer)? {
        Some(value @ serde_json::Value::Object(_)) => serde_json::from_value(value)
            .map(Some)
            .map_err(serde::de::Error::custom),
        _ => Ok(None),
    }
}
