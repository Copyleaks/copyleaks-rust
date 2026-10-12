//! Plagiarism Checker: submit files, URLs and images (OCR) for a scan, and manage your scans.
//!
//! A submit returns as soon as Copyleaks accepts the scan. Copyleaks then reports the scan result
//! to the status webhook in [`ScanProperties::webhooks`](field@ScanProperties::webhooks); parse
//! that payload with [`crate::webhooks`].

use base64::Engine as _;
use serde::{Deserialize, Deserializer, Serialize};

use crate::{validate, Result};

/// A file to scan, for [`Client::submit_file`](crate::Client::submit_file).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileSubmission {
    /// The file content, base64-encoded. To scan plain text, base64-encode the text.
    pub base64: String,
    /// The file name as it appears in the scan report, with the extension that matches the
    /// file type. At most 255 characters.
    pub filename: String,
    /// The scan configuration.
    pub properties: ScanProperties,
}

impl FileSubmission {
    /// Creates a submission from base64-encoded content.
    pub fn new(
        base64: impl Into<String>,
        filename: impl Into<String>,
        properties: ScanProperties,
    ) -> Self {
        Self {
            base64: base64.into(),
            filename: filename.into(),
            properties,
        }
    }

    /// Creates a submission from raw bytes, which it base64-encodes.
    pub fn from_bytes(
        content: impl AsRef<[u8]>,
        filename: impl Into<String>,
        properties: ScanProperties,
    ) -> Self {
        Self::new(encode(content), filename, properties)
    }

    pub(crate) fn validate(&self) -> Result<()> {
        validate::required(&self.base64, "base64")?;
        validate::length(&self.filename, "filename", 1, 255)?;
        self.properties.validate()
    }
}

/// An image or scanned document whose text is extracted with OCR and then scanned,
/// for [`Client::submit_file_ocr`](crate::Client::submit_file_ocr).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrSubmission {
    /// The file content, base64-encoded.
    pub base64: String,
    /// The file name as it appears in the scan report, with the extension that matches the
    /// file type. At most 255 characters.
    pub filename: String,
    /// The language of the text in the image, for example `en`. See
    /// [`Client::ocr_supported_languages`](crate::Client::ocr_supported_languages).
    pub lang_code: String,
    /// The scan configuration.
    pub properties: ScanProperties,
}

impl OcrSubmission {
    /// Creates an OCR submission from base64-encoded content.
    pub fn new(
        base64: impl Into<String>,
        filename: impl Into<String>,
        lang_code: impl Into<String>,
        properties: ScanProperties,
    ) -> Self {
        Self {
            base64: base64.into(),
            filename: filename.into(),
            lang_code: lang_code.into(),
            properties,
        }
    }

    /// Creates an OCR submission from raw bytes, which it base64-encodes.
    pub fn from_bytes(
        content: impl AsRef<[u8]>,
        filename: impl Into<String>,
        lang_code: impl Into<String>,
        properties: ScanProperties,
    ) -> Self {
        Self::new(encode(content), filename, lang_code, properties)
    }

    pub(crate) fn validate(&self) -> Result<()> {
        validate::required(&self.base64, "base64")?;
        validate::length(&self.filename, "filename", 1, 255)?;
        validate::required(&self.lang_code, "langCode")?;
        self.properties.validate()
    }
}

/// A web page to scan, for [`Client::submit_url`](crate::Client::submit_url).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UrlSubmission {
    /// The URL to scan, for example `https://copyleaks.com`.
    pub url: String,
    /// The scan configuration.
    pub properties: ScanProperties,
}

impl UrlSubmission {
    /// Creates a URL submission.
    pub fn new(url: impl Into<String>, properties: ScanProperties) -> Self {
        Self {
            url: url.into(),
            properties,
        }
    }

    pub(crate) fn validate(&self) -> Result<()> {
        validate::required(&self.url, "url")?;
        self.properties.validate()
    }
}

fn encode(content: impl AsRef<[u8]>) -> String {
    base64::engine::general_purpose::STANDARD.encode(content)
}

/// The configuration of a plagiarism scan. Only the status webhook is required.
///
/// ```
/// use copyleaks::plagiarism::{AiGeneratedText, ScanProperties};
///
/// let properties = ScanProperties::new("https://your.server/webhook/{STATUS}")
///     .sandbox(true)
///     .expiration(480)
///     .sensitivity_level(3)
///     .ai_generated_text(AiGeneratedText::default().detect(true));
/// assert_eq!(properties.expiration, Some(480));
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanProperties {
    /// The webhooks that Copyleaks calls with the scan status and new results.
    pub webhooks: SubmitWebhooks,
    /// What to do with the submission. Default: scan it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<SubmitAction>,
    /// Also produce an HTML version of the results, when possible.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_html: Option<bool>,
    /// A custom string returned in the webhooks. At most 512 characters.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub developer_payload: Option<String>,
    /// Sandbox mode: free mock results, for integration testing. Turn it off in production.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<bool>,
    /// How long, in hours, Copyleaks keeps the scan after it completes. From 1 to 2880.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiration: Option<u32>,
    /// The author of the content.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<Author>,
    /// Which kinds of results to include in the report.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filters: Option<Filters>,
    /// The sources to compare the document against.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scanning: Option<Scanning>,
    /// Where to index the document.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indexing: Option<Indexing>,
    /// Which parts of the document not to scan.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclude: Option<Exclude>,
    /// PDF report settings.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pdf: Option<Pdf>,
    /// Plagiarism sensitivity, from 1 (faster) to 5 (slower, finds the smallest matches).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sensitivity_level: Option<u8>,
    /// Check the document for text manipulation. Findings are reported as scan alerts in the
    /// completed webhook.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cheat_detection: Option<bool>,
    /// The goal of the scan algorithm.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scan_method_algorithm: Option<ScanMethodAlgorithm>,
    /// AI Text Detection settings. The result arrives as a `suspected-ai-text` alert in the
    /// completed webhook; see
    /// [`CompletedWebhook::ai_detection_result`](crate::webhooks::CompletedWebhook::ai_detection_result).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ai_generated_text: Option<AiGeneratedText>,
    /// Custom properties attached to the document in a Private Cloud Hub.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_metadata: Option<Vec<CustomMetadata>>,
}

impl ScanProperties {
    /// Creates scan properties with the status webhook URL. Use the `{STATUS}` placeholder,
    /// for example `https://your.server/webhook/{STATUS}`.
    pub fn new(status_webhook: impl Into<String>) -> Self {
        Self {
            webhooks: SubmitWebhooks::new(status_webhook),
            ..Self::default()
        }
    }

    /// Replaces the webhooks.
    #[must_use]
    pub fn webhooks(mut self, webhooks: SubmitWebhooks) -> Self {
        self.webhooks = webhooks;
        self
    }

    /// Sets the `expiration` field, in hours (1 to 2880).
    #[must_use]
    pub fn expiration(mut self, hours: u32) -> Self {
        self.expiration = Some(hours);
        self
    }

    pub(crate) fn validate(&self) -> Result<()> {
        validate::required(&self.webhooks.status, "properties.webhooks.status")?;
        if let Some(payload) = &self.developer_payload {
            validate::length(payload, "properties.developerPayload", 0, 512)?;
        }
        if let Some(hours) = self.expiration {
            validate::range(hours, "properties.expiration", 1, 2880)?;
        }
        if let Some(level) = self.sensitivity_level {
            validate::range(u32::from(level), "properties.sensitivityLevel", 1, 5)?;
        }
        let id_pattern = self
            .scanning
            .as_ref()
            .and_then(|scanning| scanning.exclude.as_ref())
            .and_then(|exclude| exclude.id_pattern.as_deref());
        if let Some(pattern) = id_pattern {
            validate::length(pattern, "properties.scanning.exclude.idPattern", 1, 36)?;
        }
        Ok(())
    }
}

setters!(ScanProperties {
    action: SubmitAction,
    include_html: bool,
    developer_payload: String,
    sandbox: bool,
    author: Author,
    filters: Filters,
    scanning: Scanning,
    indexing: Indexing,
    exclude: Exclude,
    pdf: Pdf,
    sensitivity_level: u8,
    cheat_detection: bool,
    scan_method_algorithm: ScanMethodAlgorithm,
    ai_generated_text: AiGeneratedText,
    custom_metadata: Vec<CustomMetadata>,
});

/// A list of HTTP headers, each a `(name, value)` pair. Serialized as `[["name", "value"], ...]`.
pub type Headers = Vec<(String, String)>;

/// The webhooks of a scan.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmitWebhooks {
    /// Called when the scan status changes. `{STATUS}` in the URL is replaced with `completed`,
    /// `error`, `creditschecked` or `indexed`.
    pub status: String,
    /// Called while the scan runs, each time a new result is found.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_result: Option<String>,
    /// Headers to send with the status webhook.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_headers: Option<Headers>,
    /// Headers to send with the new result webhook.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_result_headers: Option<Headers>,
}

impl SubmitWebhooks {
    /// Creates the webhooks with the status webhook URL.
    pub fn new(status: impl Into<String>) -> Self {
        Self {
            status: status.into(),
            ..Self::default()
        }
    }
}

setters!(SubmitWebhooks {
    new_result: String,
    status_headers: Headers,
    new_result_headers: Headers,
});

int_enum! {
    /// What to do with a submission (`properties.action`).
    SubmitAction {
        /// Scan the document now.
        Scan = 0,
        /// Only check how many credits the scan needs. Start it later with
        /// [`Client::start`](crate::Client::start).
        CheckCredits = 1,
        /// Only index the document, without scanning it. No credits are used.
        Index = 2,
    }
}

int_enum! {
    /// The goal of the scan algorithm (`properties.scanMethodAlgorithm`).
    ScanMethodAlgorithm {
        /// Prioritize a higher similarity score.
        MaximumCoverage = 0,
        /// Prioritize finding more sources.
        MaximumResults = 1,
    }
}

int_enum! {
    /// Whether [`Filters::domains`](field@Filters::domains) are included or excluded.
    DomainsMode {
        /// Only include results from these domains.
        Include = 0,
        /// Exclude results from these domains.
        Exclude = 1,
    }
}

int_enum! {
    /// The masking policy of a document indexed to a Private Cloud Hub.
    MaskingPolicy {
        /// Do not mask the document.
        NoMask = 0,
        /// Mask the document for other users.
        MaskOtherUsersFiles = 1,
        /// Mask the document for all users.
        MaskAllFiles = 2,
    }
}

int_enum! {
    /// The PDF report version (`properties.pdf.version`).
    PdfVersion {
        /// Version 1.
        V1 = 1,
        /// Version 2.
        V2 = 2,
        /// Version 3.
        V3 = 3,
    }
}

/// The author of the scanned content.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Author {
    /// A unique id for the author. Use the same id for the same author.
    pub id: String,
}

impl Author {
    /// Creates an author.
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }
}

/// Which kinds of results to include in the report.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Filters {
    /// Match identical words.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identical_enabled: Option<bool>,
    /// Match nearly identical words, such as "slow" and "slowly".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minor_changes_enabled: Option<bool>,
    /// Match paraphrased content.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub related_meaning_enabled: Option<bool>,
    /// Only show results with at least this many copied words.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_copied_words: Option<u32>,
    /// Block explicit adult content from the results.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub safe_search: Option<bool>,
    /// Domains to include or exclude, depending on
    /// [`Filters::domains_mode`](field@Filters::domains_mode).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domains: Option<Vec<String>>,
    /// Whether [`Filters::domains`](field@Filters::domains) are included or excluded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domains_mode: Option<DomainsMode>,
    /// Allow results from the same domain as the submitted URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_same_domain: Option<bool>,
}

impl Filters {
    /// Sets the `min_copied_words` field.
    #[must_use]
    pub fn min_copied_words(mut self, words: u32) -> Self {
        self.min_copied_words = Some(words);
        self
    }
}

setters!(Filters {
    identical_enabled: bool,
    minor_changes_enabled: bool,
    related_meaning_enabled: bool,
    safe_search: bool,
    domains: Vec<String>,
    domains_mode: DomainsMode,
    allow_same_domain: bool,
});

/// The sources to compare the document against.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Scanning {
    /// Compare with online sources.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub internet: Option<bool>,
    /// Exclude some of your own submissions from the results.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclude: Option<ScanningExclude>,
    /// Private Cloud Hubs to compare against.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repositories: Option<Vec<SearchRepository>>,
    /// Compare against the Shared Data Hub.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub copyleaks_db: Option<CopyleaksDb>,
    /// Cross-language plagiarism detection.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cross_languages: Option<CrossLanguages>,
}

setters!(Scanning {
    internet: bool,
    exclude: ScanningExclude,
    repositories: Vec<SearchRepository>,
    copyleaks_db: CopyleaksDb,
    cross_languages: CrossLanguages,
});

/// Submissions to exclude from the results.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanningExclude {
    /// Exclude submissions whose scan id matches this pattern, 1 to 36 characters. `*` matches
    /// any number of characters and `.` matches exactly one, for example `abc*`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id_pattern: Option<String>,
}

setters!(ScanningExclude { id_pattern: String });

/// A Private Cloud Hub to compare the document against.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchRepository {
    /// The repository id of the Private Cloud Hub.
    pub id: String,
    /// Compare against your own submissions in the hub.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_my_submissions: Option<bool>,
    /// Compare against other users' submissions in the hub.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_others_submissions: Option<bool>,
}

impl SearchRepository {
    /// Creates a repository to scan against.
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            ..Self::default()
        }
    }
}

setters!(SearchRepository {
    include_my_submissions: bool,
    include_others_submissions: bool,
});

/// Comparison against the Shared Data Hub.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CopyleaksDb {
    /// Also compare against content that you uploaded to the Shared Data Hub.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_my_submissions: Option<bool>,
    /// Also compare against content that other users uploaded to the Shared Data Hub.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_others_submissions: Option<bool>,
}

setters!(CopyleaksDb {
    include_my_submissions: bool,
    include_others_submissions: bool,
});

/// Cross-language plagiarism detection.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct CrossLanguages {
    /// The additional languages to scan against. Do not include the document's own language.
    pub languages: Vec<Language>,
}

impl CrossLanguages {
    /// Creates the setting from language codes, for example `["fr", "es"]`.
    pub fn new<S: Into<String>>(codes: impl IntoIterator<Item = S>) -> Self {
        Self {
            languages: codes.into_iter().map(Language::new).collect(),
        }
    }
}

/// A language for cross-language plagiarism detection.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Language {
    /// The language code, for example `fr`.
    pub code: String,
}

impl Language {
    /// Creates a language from its code.
    pub fn new(code: impl Into<String>) -> Self {
        Self { code: code.into() }
    }
}

/// Where to index the document.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Indexing {
    /// The Private Cloud Hubs to index the document to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repositories: Option<Vec<IndexingRepository>>,
}

setters!(Indexing {
    repositories: Vec<IndexingRepository>,
});

/// A Private Cloud Hub to index the document to.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexingRepository {
    /// The repository id of the Private Cloud Hub.
    pub id: String,
    /// The masking policy of this document. When the hub has its own policy, the stricter one
    /// applies.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub masking_policy: Option<MaskingPolicy>,
}

impl IndexingRepository {
    /// Creates a repository to index to.
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            masking_policy: None,
        }
    }
}

setters!(IndexingRepository {
    masking_policy: MaskingPolicy,
});

/// Parts of the document not to scan.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Exclude {
    /// Exclude quoted text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quotes: Option<bool>,
    /// Exclude citations.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub citations: Option<bool>,
    /// Exclude referenced text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub references: Option<bool>,
    /// Exclude the table of contents.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub table_of_contents: Option<bool>,
    /// Exclude titles.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub titles: Option<bool>,
    /// For HTML documents, exclude text that repeats across the site, such as headers and footers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub html_template: Option<bool>,
    /// Exclude template text found in these completed scans (at most 3 scan ids).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_template_ids: Option<Vec<String>>,
    /// Exclude parts of source code.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<ExcludeCode>,
}

setters!(Exclude {
    quotes: bool,
    citations: bool,
    references: bool,
    table_of_contents: bool,
    titles: bool,
    html_template: bool,
    document_template_ids: Vec<String>,
    code: ExcludeCode,
});

/// Parts of source code not to scan.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ExcludeCode {
    /// Exclude code comments.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comments: Option<bool>,
}

setters!(ExcludeCode { comments: bool });

/// PDF report settings.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pdf {
    /// Create a PDF report for the scan.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub create: Option<bool>,
    /// The report title. At most 256 characters.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// A base64-encoded PNG logo for the report, at most 100 KB.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub large_logo: Option<String>,
    /// Align the report text from right to left.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rtl: Option<bool>,
    /// The report version. Deprecated by the API in favor of `reportVersion`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<PdfVersion>,
    /// The report colors.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub colors: Option<PdfColors>,
}

setters!(Pdf {
    create: bool,
    title: String,
    large_logo: String,
    rtl: bool,
    version: PdfVersion,
    colors: PdfColors,
});

/// The colors of the PDF report, as HEX strings such as `#FE5758`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfColors {
    /// The main strip in the header.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub main_strip: Option<String>,
    /// Titles.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub titles: Option<String>,
    /// Highlights of identical matches.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identical: Option<String>,
    /// Highlights of minor changes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minor_changes: Option<String>,
    /// Highlights of related meaning (paraphrased) matches.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub related_meaning: Option<String>,
}

setters!(PdfColors {
    main_strip: String,
    titles: String,
    identical: String,
    minor_changes: String,
    related_meaning: String,
});

/// AI Text Detection settings for a plagiarism scan.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct AiGeneratedText {
    /// Detect whether the text was written by AI.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detect: Option<bool>,
}

setters!(AiGeneratedText { detect: bool });

/// A custom property attached to the document in a Private Cloud Hub.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct CustomMetadata {
    /// The property name.
    pub key: String,
    /// The property value.
    pub value: String,
}

impl CustomMetadata {
    /// Creates a custom property.
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
        }
    }
}

/// An export of scan artifacts to your servers, for [`Client::export`](crate::Client::export).
/// When the export is done, Copyleaks calls the completion webhook with an
/// [`ExportCompletedWebhook`](crate::webhooks::ExportCompletedWebhook).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Export {
    /// Called once the export is done.
    pub completion_webhook: String,
    /// How many times to retry an upload before giving up. From 1 to 12.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_retries: Option<u8>,
    /// A custom string returned in the export completed webhook.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub developer_payload: Option<String>,
    /// The results to export. At most 1000.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub results: Option<Vec<ExportResult>>,
    /// Export the PDF report. Allowed only when the scan was submitted with `pdf.create`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pdf_report: Option<ExportTarget>,
    /// Export the crawled version of the scanned document.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crawled_version: Option<ExportTarget>,
}

impl Export {
    /// Creates an export with its completion webhook URL.
    pub fn new(completion_webhook: impl Into<String>) -> Self {
        Self {
            completion_webhook: completion_webhook.into(),
            ..Self::default()
        }
    }

    pub(crate) fn validate(&self) -> Result<()> {
        validate::required(&self.completion_webhook, "completionWebhook")?;
        if let Some(retries) = self.max_retries {
            validate::range(u32::from(retries), "maxRetries", 1, 12)?;
        }
        if let Some(results) = &self.results {
            validate::count(results.len(), "results", 0, 1000)?;
            for result in results {
                validate::required(&result.id, "results[].id")?;
                validate::required(&result.endpoint, "results[].endpoint")?;
                validate::required(&result.verb, "results[].verb")?;
            }
        }
        for (target, name) in [
            (&self.pdf_report, "pdfReport"),
            (&self.crawled_version, "crawledVersion"),
        ] {
            if let Some(target) = target {
                validate::required(&target.endpoint, &format!("{name}.endpoint"))?;
                validate::required(&target.verb, &format!("{name}.verb"))?;
            }
        }
        Ok(())
    }
}

setters!(Export {
    max_retries: u8,
    developer_payload: String,
    results: Vec<ExportResult>,
    pdf_report: ExportTarget,
    crawled_version: ExportTarget,
});

/// Where to upload an exported artifact.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ExportTarget {
    /// The URL to upload the data to.
    pub endpoint: String,
    /// The HTTP method of the upload, for example `POST` or `PUT`.
    pub verb: String,
    /// Headers to send with the upload, for example an `Authorization` header.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<Headers>,
}

impl ExportTarget {
    /// Creates an upload target.
    pub fn new(endpoint: impl Into<String>, verb: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            verb: verb.into(),
            headers: None,
        }
    }
}

setters!(ExportTarget { headers: Headers });

/// A result to export, and where to upload it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ExportResult {
    /// The result id, from the completed webhook.
    pub id: String,
    /// The URL to upload the data to.
    pub endpoint: String,
    /// The HTTP method of the upload, for example `POST` or `PUT`.
    pub verb: String,
    /// Headers to send with the upload.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<Headers>,
}

impl ExportResult {
    /// Creates a result to export.
    pub fn new(
        id: impl Into<String>,
        endpoint: impl Into<String>,
        verb: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            endpoint: endpoint.into(),
            verb: verb.into(),
            headers: None,
        }
    }
}

setters!(ExportResult { headers: Headers });

/// Starts scans that were submitted with [`SubmitAction::CheckCredits`] or
/// [`SubmitAction::Index`], for [`Client::start`](crate::Client::start).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Start {
    /// The scan ids to start. At most 100.
    pub trigger: Vec<String>,
    /// What to do when some of the scans cannot start. Default: cancel all.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_handling: Option<StartErrorHandling>,
}

impl Start {
    /// Creates a start request for these scan ids.
    pub fn new<S: Into<String>>(scan_ids: impl IntoIterator<Item = S>) -> Self {
        Self {
            trigger: scan_ids.into_iter().map(Into::into).collect(),
            error_handling: None,
        }
    }

    pub(crate) fn validate(&self) -> Result<()> {
        validate::count(self.trigger.len(), "trigger", 1, 100)
    }
}

setters!(Start {
    error_handling: StartErrorHandling,
});

int_enum! {
    /// What [`Client::start`](crate::Client::start) does when some of the scans cannot start.
    StartErrorHandling {
        /// Cancel the whole request.
        Cancel = 0,
        /// Start the scans that can start.
        Ignore = 1,
    }
}

/// The result of [`Client::start`](crate::Client::start).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartResponse {
    /// The scan ids that started.
    #[serde(default, deserialize_with = "scan_ids")]
    pub success: Vec<String>,
    /// The scan ids that did not start.
    #[serde(default, deserialize_with = "scan_ids")]
    pub failed: Vec<String>,
}

/// Reads a list of scan ids sent as strings or as `{"id": "..."}` objects, skipping `null` entries.
fn scan_ids<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Row {
        Id(String),
        Object { id: Option<String> },
    }

    let rows = Option::<Vec<Option<Row>>>::deserialize(deserializer)?;
    Ok(rows
        .unwrap_or_default()
        .into_iter()
        .flatten()
        .filter_map(|row| match row {
            Row::Id(id) => Some(id),
            Row::Object { id } => id,
        })
        .collect())
}

/// Deletes scans, for [`Client::delete`](crate::Client::delete). Only completed scans can be
/// deleted. The deletion runs in the background.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Delete {
    /// The scans to delete. At most 10000.
    pub scans: Vec<DeleteScan>,
    /// Remove every trace of the scans, including from the Shared Data Hub and Private Cloud Hubs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub purge: Option<bool>,
    /// Called (POST, no body) when the deletion is done.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion_webhook: Option<String>,
    /// Headers to send with the completion webhook.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<Headers>,
}

impl Delete {
    /// Creates a delete request for these scan ids.
    pub fn new<S: Into<String>>(scan_ids: impl IntoIterator<Item = S>) -> Self {
        Self {
            scans: scan_ids.into_iter().map(DeleteScan::new).collect(),
            ..Self::default()
        }
    }

    pub(crate) fn validate(&self) -> Result<()> {
        validate::count(self.scans.len(), "scans", 1, 10_000)?;
        for scan in &self.scans {
            validate::required(&scan.id, "scans[].id")?;
        }
        Ok(())
    }
}

setters!(Delete {
    purge: bool,
    completion_webhook: String,
    headers: Headers,
});

/// A scan to delete.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct DeleteScan {
    /// The scan id.
    pub id: String,
}

impl DeleteScan {
    /// Creates a scan to delete.
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }
}

/// The credit balance of your account, from
/// [`Client::credits_balance`](crate::Client::credits_balance).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreditsBalance {
    /// Remaining credits. One credit scans up to 250 words.
    #[serde(rename = "Amount", alias = "amount", default)]
    pub amount: i64,
}
