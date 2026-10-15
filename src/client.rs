//! The Copyleaks API client.

use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use reqwest::header::{HeaderValue, CONTENT_TYPE, USER_AGENT};
use reqwest::multipart::{Form, Part};
use reqwest::{Method, Response, Url};
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::ai_image::{
    self, AiImageDetectionRequest, AiImageDetectionResponse, AiImageDetectionSubmission,
};
use crate::ai_text::{AiTextDetectionCredits, AiTextDetectionRequest, AiTextDetectionResponse};
use crate::ai_video::AiVideoDetectionSubmission;
use crate::body::{off_runtime, UploadBody};
use crate::grammar::{CorrectionTypes, GrammarCheckRequest, GrammarCheckResponse};
use crate::image_plagiarism::{ImagePlagiarismRequest, ImagePlagiarismResponse};
use crate::misc::{
    AllowedCrossLanguages, ReleaseNotes, SupportedAiTextFileTypes, SupportedFileTypes,
};
use crate::moderation::{TextModerationRequest, TextModerationResponse};
use crate::plagiarism::{
    CreditsBalance, Delete, Export, FileSubmission, OcrSubmission, Start, StartResponse,
    UrlSubmission,
};
use crate::repositories::RepositoryInfo;
use crate::retry::{self, RetryPolicy};
use crate::{validate, AuthToken, Error, Result};

/// The default base URL of the Copyleaks API.
pub const DEFAULT_API_BASE_URL: &str = "https://api.copyleaks.com";
/// The default base URL of the Copyleaks identity server, used by [`Client::login`].
pub const DEFAULT_IDENTITY_BASE_URL: &str = "https://id.copyleaks.com";
/// The SDK's `User-Agent` product token, `copyleaks-rust-sdk/<crate version>`, always sent.
pub const DEFAULT_USER_AGENT: &str = concat!("copyleaks-rust-sdk/", env!("CARGO_PKG_VERSION"));

/// The total time limit of each request of the default HTTP client.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);

/// The time limit to open a connection, for the default HTTP client.
const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// The largest JSON response body the client reads: 16 MiB.
const MAX_JSON_BODY: usize = 16 << 20;

/// The largest usage history CSV the client reads: 64 MiB.
const MAX_CSV_BODY: usize = 64 << 20;

/// How much of an error response body the client keeps: 1 MiB.
const MAX_ERROR_BODY: usize = 1 << 20;

/// How much of an unused response body the client reads, so that the connection can be reused.
const MAX_DRAIN_BODY: usize = 64 << 10;

#[derive(Debug)]
struct Inner {
    http: reqwest::Client,
    api_base: Url,
    identity_base: Url,
    user_agent: HeaderValue,
    retry: Option<RetryPolicy>,
}

/// An async client for the Copyleaks API. Clones are cheap and share one connection pool: create
/// one client and reuse it.
///
/// ```no_run
/// # async fn run() -> Result<(), copyleaks::Error> {
/// let client = copyleaks::Client::new()?;
/// let token = client.login("you@example.com", "your-api-key").await?;
/// let balance = client.credits_balance(&token).await?;
/// println!("{} credits left", balance.amount);
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct Client {
    inner: Arc<Inner>,
}

#[derive(Debug, Clone, Copy)]
enum Server {
    Api,
    Identity,
}

enum Payload {
    Empty,
    /// A JSON body, shared by every attempt.
    Json(Bytes),
    /// A multipart form. It can be sent once, so calls with a form are never retried.
    Form(Option<Form>),
}

impl Payload {
    fn json<T: Serialize + ?Sized>(value: &T) -> Result<Self> {
        Ok(Payload::Json(Bytes::from(serde_json::to_vec(value)?)))
    }

    /// Writes an upload body, off the async runtime when it is large.
    async fn upload(body: UploadBody) -> Result<Self> {
        let size = body.size_hint();
        let bytes = off_runtime(size, move || body.build()).await?;
        Ok(Payload::Json(Bytes::from(bytes)))
    }

    fn can_repeat(&self) -> bool {
        !matches!(self, Payload::Form(_))
    }
}

impl Client {
    /// Creates a client with the default settings of [`ClientBuilder`].
    ///
    /// # Errors
    ///
    /// [`Error::Http`] when the HTTP client cannot be created, for example when the TLS backend
    /// fails to initialize.
    pub fn new() -> Result<Self> {
        Self::builder().build()
    }

    /// Returns a builder for the HTTP client, timeouts, retries, base URLs and user agent.
    pub fn builder() -> ClientBuilder {
        ClientBuilder::default()
    }

    /// Logs in with your account email and API key. Log in once and reuse the token: the login
    /// endpoint allows 12 requests per 15 minutes.
    ///
    /// # Errors
    ///
    /// [`Error::Unauthorized`] for a wrong email or key, [`Error::RateLimited`] after too many
    /// logins, and the other [`Error`] variants for other failures.
    pub async fn login(&self, email: &str, key: &str) -> Result<AuthToken> {
        #[derive(Serialize)]
        struct Login<'a> {
            email: &'a str,
            key: &'a str,
        }

        validate::required(email, "email")?;
        validate::required(key, "key")?;
        let url = self.url(Server::Identity, &["v3", "account", "login", "api"])?;
        let payload = Payload::json(&Login { email, key })?;
        let reply = self.send(Method::POST, url, None, payload).await?;
        reply.json().await
    }

    /// Submits a file for a plagiarism scan. Returns when Copyleaks accepts the scan; the result
    /// arrives at the status webhook.
    ///
    /// # Errors
    ///
    /// [`Error::Validation`] for a bad scan id or submission, [`Error::Api`] with status 409 when
    /// the scan id is already used, and the other [`Error`] variants for other failures.
    pub async fn submit_file(
        &self,
        token: &AuthToken,
        scan_id: &str,
        submission: &FileSubmission,
    ) -> Result<()> {
        validate::submit_scan_id(scan_id)?;
        submission.validate()?;
        let url = self.url(Server::Api, &["v3", "scans", "submit", "file", scan_id])?;
        let payload = Payload::upload(submission.upload_body()?).await?;
        self.send(Method::PUT, url, Some(token), payload)
            .await?
            .drain()
            .await;
        Ok(())
    }

    /// Submits an image or scanned document for a plagiarism scan of its OCR text, like
    /// [`Client::submit_file`].
    ///
    /// # Errors
    ///
    /// The same as [`Client::submit_file`].
    pub async fn submit_file_ocr(
        &self,
        token: &AuthToken,
        scan_id: &str,
        submission: &OcrSubmission,
    ) -> Result<()> {
        validate::submit_scan_id(scan_id)?;
        submission.validate()?;
        let url = self.url(Server::Api, &["v3", "scans", "submit", "ocr", scan_id])?;
        let payload = Payload::upload(submission.upload_body()?).await?;
        self.send(Method::PUT, url, Some(token), payload)
            .await?
            .drain()
            .await;
        Ok(())
    }

    /// Submits a URL for a plagiarism scan, like [`Client::submit_file`].
    ///
    /// # Errors
    ///
    /// The same as [`Client::submit_file`].
    pub async fn submit_url(
        &self,
        token: &AuthToken,
        scan_id: &str,
        submission: &UrlSubmission,
    ) -> Result<()> {
        validate::submit_scan_id(scan_id)?;
        submission.validate()?;
        let url = self.url(Server::Api, &["v3", "scans", "submit", "url", scan_id])?;
        let payload = Payload::json(submission)?;
        self.send(Method::PUT, url, Some(token), payload)
            .await?
            .drain()
            .await;
        Ok(())
    }

    /// Exports the artifacts of a completed scan to your servers, as set in [`Export`].
    ///
    /// # Errors
    ///
    /// [`Error::Validation`] for bad ids or a bad export, [`Error::Api`] with status 404 for an
    /// unknown scan or 409 for a used export id, and the other [`Error`] variants for other
    /// failures. The scan id and the export id are 3 to 36 characters from
    /// ``a-z0-9!@$^&+%=_(){}<>'";:/.,~`|\-`` and must not contain `g`, one or more `o` or `0`,
    /// then `gle`.
    pub async fn export(
        &self,
        token: &AuthToken,
        scan_id: &str,
        export_id: &str,
        export: &Export,
    ) -> Result<()> {
        validate::export_id(scan_id, "scan id")?;
        validate::export_id(export_id, "export id")?;
        export.validate()?;
        let url = self.url(
            Server::Api,
            &["v3", "downloads", scan_id, "export", export_id],
        )?;
        let payload = Payload::json(export)?;
        self.send(Method::POST, url, Some(token), payload)
            .await?
            .drain()
            .await;
        Ok(())
    }

    /// Starts scans that were submitted to check credits or to index only.
    ///
    /// # Errors
    ///
    /// [`Error::Validation`] for an empty or too long trigger list, and the other [`Error`]
    /// variants for other failures.
    pub async fn start(&self, token: &AuthToken, start: &Start) -> Result<StartResponse> {
        start.validate()?;
        let url = self.url(Server::Api, &["v3", "scans", "start"])?;
        let payload = Payload::json(start)?;
        let reply = self.send(Method::PATCH, url, Some(token), payload).await?;
        reply.json().await
    }

    /// Deletes completed scans. The deletion runs in the background.
    ///
    /// # Errors
    ///
    /// [`Error::Validation`] for an empty or too long scan list, and the other [`Error`] variants
    /// for other failures.
    pub async fn delete(&self, token: &AuthToken, delete: &Delete) -> Result<()> {
        delete.validate()?;
        let url = self.url(Server::Api, &["v3.1", "scans", "delete"])?;
        let payload = Payload::json(delete)?;
        self.send(Method::PATCH, url, Some(token), payload)
            .await?
            .drain()
            .await;
        Ok(())
    }

    /// Asks Copyleaks to send the status webhook of a finished scan again.
    ///
    /// # Errors
    ///
    /// [`Error::Validation`] for a scan id that is not 3 to 36 characters long, [`Error::Api`]
    /// with status 400 when the scan is still running or 404 for an unknown scan, and the other
    /// [`Error`] variants for other failures.
    pub async fn resend_webhook(&self, token: &AuthToken, scan_id: &str) -> Result<()> {
        validate::id_length(scan_id, "scan id")?;
        let url = self.url(Server::Api, &["v3", "scans", scan_id, "webhooks", "resend"])?;
        self.send(Method::POST, url, Some(token), Payload::Empty)
            .await?
            .drain()
            .await;
        Ok(())
    }

    /// Returns the credit balance of your account.
    ///
    /// # Errors
    ///
    /// [`Error::RateLimited`] after too many calls, and the other [`Error`] variants for other
    /// failures.
    pub async fn credits_balance(&self, token: &AuthToken) -> Result<CreditsBalance> {
        let url = self.url(Server::Api, &["v3", "scans", "credits"])?;
        let reply = self
            .send(Method::GET, url, Some(token), Payload::Empty)
            .await?;
        reply.json().await
    }

    /// Returns your usage history between two `dd-MM-yyyy` dates, as the raw CSV file.
    ///
    /// # Errors
    ///
    /// [`Error::Validation`] for a date that is not `dd-MM-yyyy`, [`Error::RateLimited`] after too
    /// many calls, [`Error::ResponseTooLarge`] for a CSV over 64 MiB, and the other [`Error`]
    /// variants for other failures.
    pub async fn usages_history_csv(
        &self,
        token: &AuthToken,
        start: &str,
        end: &str,
    ) -> Result<Vec<u8>> {
        validate::date(start, "start")?;
        validate::date(end, "end")?;
        let mut url = self.url(Server::Api, &["v3", "scans", "usages", "history"])?;
        url.query_pairs_mut()
            .append_pair("start", start)
            .append_pair("end", end);
        let reply = self
            .send(Method::GET, url, Some(token), Payload::Empty)
            .await?;
        reply.body(MAX_CSV_BODY).await
    }

    /// Returns the API release notes. Needs no login. Not part of the API reference; the content
    /// may change.
    ///
    /// # Errors
    ///
    /// The [`Error`] variants for transport, server and decoding failures.
    pub async fn release_notes(&self) -> Result<ReleaseNotes> {
        self.public_json(&["v3", "release-logs.json"]).await
    }

    /// Returns the file types the Plagiarism Checker accepts. Needs no login.
    ///
    /// # Errors
    ///
    /// The [`Error`] variants for transport, server and decoding failures.
    pub async fn supported_file_types(&self) -> Result<SupportedFileTypes> {
        self.public_json(&["v3", "miscellaneous", "supported-plagiarism-file-types"])
            .await
    }

    /// Returns the file types AI Text Detection accepts in a plagiarism scan. Needs no login.
    ///
    /// # Errors
    ///
    /// The [`Error`] variants for transport, server and decoding failures.
    pub async fn supported_ai_text_detection_file_types(&self) -> Result<SupportedAiTextFileTypes> {
        self.public_json(&[
            "v3",
            "miscellaneous",
            "supported-ai-text-detection-file-types",
        ])
        .await
    }

    /// Returns the OCR language codes, for [`Client::submit_file_ocr`]. Needs no login.
    ///
    /// # Errors
    ///
    /// The [`Error`] variants for transport, server and decoding failures.
    pub async fn ocr_supported_languages(&self) -> Result<Vec<String>> {
        self.public_json(&["v3", "miscellaneous", "ocr-languages-list"])
            .await
    }

    /// Returns the languages of cross-language plagiarism detection. Needs no login.
    ///
    /// # Errors
    ///
    /// The [`Error`] variants for transport, server and decoding failures.
    pub async fn allowed_cross_languages(&self) -> Result<AllowedCrossLanguages> {
        self.public_json(&["v3", "miscellaneous", "allowed-cross-languages"])
            .await
    }

    /// Returns information about a Private Cloud Hub repository. Needs the Admin or Super Admin
    /// role in it.
    ///
    /// # Errors
    ///
    /// [`Error::Validation`] for an empty repository id, or `.` or `..`, [`Error::Api`] with
    /// status 403 without the needed role, and the other [`Error`] variants for other failures.
    pub async fn repository_info(
        &self,
        token: &AuthToken,
        repository_id: &str,
    ) -> Result<RepositoryInfo> {
        validate::path_segment(repository_id, "repository id")?;
        let url = self.url(
            Server::Api,
            &["v3", "repositories", "repository", repository_id, "info"],
        )?;
        let reply = self
            .send(Method::GET, url, Some(token), Payload::Empty)
            .await?;
        reply.json().await
    }

    /// AI Text Detection: classifies the sections of a text as human-written or AI-generated.
    ///
    /// # Errors
    ///
    /// [`Error::Validation`] for a scan id that is not 3 to 36 characters long or a bad request,
    /// and the other [`Error`] variants for other failures.
    pub async fn ai_text_detection(
        &self,
        token: &AuthToken,
        scan_id: &str,
        request: &AiTextDetectionRequest,
    ) -> Result<AiTextDetectionResponse> {
        validate::id_length(scan_id, "scan id")?;
        request.validate()?;
        let url = self.url(Server::Api, &["v2", "writer-detector", scan_id, "check"])?;
        let payload = Payload::json(request)?;
        let reply = self.send(Method::POST, url, Some(token), payload).await?;
        reply.json().await
    }

    /// Returns the AI Text Detection credit balance of your account.
    ///
    /// # Errors
    ///
    /// The [`Error`] variants for transport, server and decoding failures.
    pub async fn ai_text_detection_credits(
        &self,
        token: &AuthToken,
    ) -> Result<AiTextDetectionCredits> {
        let url = self.url(Server::Api, &["v2", "writer-detector", "credits"])?;
        let reply = self
            .send(Method::GET, url, Some(token), Payload::Empty)
            .await?;
        reply.json().await
    }

    /// AI Image Detection of an image sent base64-encoded in JSON. For large images, prefer
    /// [`Client::ai_image_detection_multipart`].
    ///
    /// # Errors
    ///
    /// [`Error::Validation`] for a bad scan id or request, and the other [`Error`] variants for
    /// other failures.
    pub async fn ai_image_detection(
        &self,
        token: &AuthToken,
        scan_id: &str,
        request: &AiImageDetectionRequest,
    ) -> Result<AiImageDetectionResponse> {
        validate::id_length(scan_id, "scan id")?;
        request.validate()?;
        let url = self.url(Server::Api, &["v1", "ai-image-detector", scan_id, "check"])?;
        let payload = Payload::upload(request.upload_body()?).await?;
        let reply = self.send(Method::POST, url, Some(token), payload).await?;
        reply.json().await
    }

    /// [`Client::ai_image_detection`] with a `multipart/form-data` upload, without the base64
    /// overhead. A request created from base64 text is decoded first.
    ///
    /// # Errors
    ///
    /// [`Error::Validation`] for a bad scan id or request, or base64 text that does not decode,
    /// and the other [`Error`] variants for other failures.
    pub async fn ai_image_detection_multipart(
        &self,
        token: &AuthToken,
        scan_id: &str,
        request: &AiImageDetectionRequest,
    ) -> Result<AiImageDetectionResponse> {
        validate::id_length(scan_id, "scan id")?;
        request.validate()?;
        let url = self.url(Server::Api, &["v1", "ai-image-detector", scan_id, "check"])?;
        let content = request.content().clone();
        let image = off_runtime(content.len(), move || content.to_raw()).await?;
        let form = image_form(image, &request.filename, request.form_fields()?)?;
        let reply = self
            .send(Method::POST, url, Some(token), Payload::Form(Some(form)))
            .await?;
        reply.json().await
    }

    /// AI Image Detection of an image URL. Returns when Copyleaks accepts the scan; Copyleaks
    /// then posts an [`AiImageDetectionResponse`] to the submission's webhook.
    ///
    /// # Errors
    ///
    /// [`Error::Validation`] for a bad scan id (same rules as for [`Client::export`]) or
    /// submission, [`Error::Api`] with status 409 when the scan id is already used, and the other
    /// [`Error`] variants for other failures.
    pub async fn submit_ai_image_detection(
        &self,
        token: &AuthToken,
        scan_id: &str,
        submission: &AiImageDetectionSubmission,
    ) -> Result<()> {
        validate::scan_id(scan_id)?;
        submission.validate()?;
        let url = self.url(
            Server::Api,
            &["v1", "ai-image-detector-async", scan_id, "submit"],
        )?;
        let payload = Payload::json(submission)?;
        self.send(Method::PUT, url, Some(token), payload)
            .await?
            .drain()
            .await;
        Ok(())
    }

    /// AI Video Detection of a video URL. Returns when Copyleaks accepts the scan; Copyleaks then
    /// posts an [`AiVideoDetectionResponse`](crate::ai_video::AiVideoDetectionResponse) to the
    /// submission's webhook.
    ///
    /// # Errors
    ///
    /// The same as [`Client::submit_ai_image_detection`].
    pub async fn submit_ai_video_detection(
        &self,
        token: &AuthToken,
        scan_id: &str,
        submission: &AiVideoDetectionSubmission,
    ) -> Result<()> {
        validate::scan_id(scan_id)?;
        submission.validate()?;
        let url = self.url(Server::Api, &["v1", "ai-video-detector", scan_id, "submit"])?;
        let payload = Payload::json(submission)?;
        self.send(Method::POST, url, Some(token), payload)
            .await?
            .drain()
            .await;
        Ok(())
    }

    /// Image Plagiarism Detector: searches the web for copies of an image, full or modified.
    /// Uploads the image as `multipart/form-data`.
    ///
    /// # Errors
    ///
    /// [`Error::Validation`] for a bad scan id (same rules as for [`Client::export`]) or request,
    /// and the other [`Error`] variants for other failures.
    pub async fn image_plagiarism_detection(
        &self,
        token: &AuthToken,
        scan_id: &str,
        request: &ImagePlagiarismRequest,
    ) -> Result<ImagePlagiarismResponse> {
        validate::scan_id(scan_id)?;
        request.validate()?;
        let url = self.url(
            Server::Api,
            &["v1", "image-plagiarism-detector", scan_id, "check"],
        )?;
        let form = image_form(request.image(), &request.filename, request.form_fields())?;
        let reply = self
            .send(Method::POST, url, Some(token), Payload::Form(Some(form)))
            .await?;
        reply.json().await
    }

    /// Grammar Checker: returns grammar, spelling and sentence corrections, and writing scores.
    ///
    /// # Errors
    ///
    /// [`Error::Validation`] for a bad scan id or request, and the other [`Error`] variants for
    /// other failures.
    pub async fn grammar_check(
        &self,
        token: &AuthToken,
        scan_id: &str,
        request: &GrammarCheckRequest,
    ) -> Result<GrammarCheckResponse> {
        validate::id_length(scan_id, "scan id")?;
        request.validate()?;
        let url = self.url(Server::Api, &["v1", "writing-feedback", scan_id, "check"])?;
        let payload = Payload::json(request)?;
        let reply = self.send(Method::POST, url, Some(token), payload).await?;
        reply.json().await
    }

    /// Returns the Grammar Checker correction types, in a language such as `en`. Needs no login.
    ///
    /// # Errors
    ///
    /// [`Error::Validation`] for an empty language code, or `.` or `..`, and the other [`Error`]
    /// variants for transport, server and decoding failures.
    pub async fn grammar_correction_types(&self, language_code: &str) -> Result<CorrectionTypes> {
        validate::path_segment(language_code, "language code")?;
        self.public_json(&["v1", "writing-feedback", "correction-types", language_code])
            .await
    }

    /// Text Moderation: flags segments of a text with the requested moderation labels.
    ///
    /// # Errors
    ///
    /// [`Error::Validation`] for a bad scan id or request, and the other [`Error`] variants for
    /// other failures.
    pub async fn text_moderation(
        &self,
        token: &AuthToken,
        scan_id: &str,
        request: &TextModerationRequest,
    ) -> Result<TextModerationResponse> {
        validate::id_length(scan_id, "scan id")?;
        request.validate()?;
        let url = self.url(Server::Api, &["v1", "text-moderation", scan_id, "check"])?;
        let payload = Payload::json(request)?;
        let reply = self.send(Method::POST, url, Some(token), payload).await?;
        reply.json().await
    }

    /// A `GET` of a public endpoint, decoded from JSON.
    async fn public_json<T: DeserializeOwned + Send + 'static>(
        &self,
        segments: &[&str],
    ) -> Result<T> {
        let url = self.url(Server::Api, segments)?;
        let reply = self.send(Method::GET, url, None, Payload::Empty).await?;
        reply.json().await
    }

    /// Builds the URL of an endpoint. Each segment is percent-encoded as a path segment.
    fn url(&self, server: Server, segments: &[&str]) -> Result<Url> {
        let mut url = match server {
            Server::Api => self.inner.api_base.clone(),
            Server::Identity => self.inner.identity_base.clone(),
        };
        url.path_segments_mut()
            .map_err(|()| Error::Validation("the base URL cannot have a path".to_owned()))?
            .pop_if_empty()
            .extend(segments);
        Ok(url)
    }

    /// Sends a request with the common headers, retries it when the retry policy allows, and
    /// maps an error status to an [`Error`]. When a token is given, it is checked locally before
    /// each attempt, then sent as a bearer token.
    async fn send(
        &self,
        method: Method,
        url: Url,
        token: Option<&AuthToken>,
        mut payload: Payload,
    ) -> Result<Reply> {
        let policy = self
            .inner
            .retry
            .as_ref()
            .filter(|_| can_retry(&method) && payload.can_repeat());
        let Some(policy) = policy else {
            if let Some(token) = token {
                token.verify()?;
            }
            let response = self.attempt(&method, url, token, &mut payload).await?;
            return finish(method, response).await;
        };
        let mut retries = 0;
        loop {
            if let Some(token) = token {
                token.verify()?;
            }
            let attempt = self
                .attempt(&method, url.clone(), token, &mut payload)
                .await;
            let wait = match attempt {
                Ok(mut response) => {
                    let status = response.status().as_u16();
                    if retries >= policy.max_retries || !matches!(status, 429 | 503) {
                        return finish(method, response).await;
                    }
                    let wait = match retry::retry_after(response.headers()) {
                        Some(wait) if wait > policy.max_retry_after => {
                            return finish(method, response).await;
                        }
                        Some(wait) => wait,
                        None => policy.backoff(retries),
                    };
                    drain(&mut response).await;
                    wait
                }
                Err(error) if error.is_connect() && retries < policy.max_retries => {
                    policy.backoff(retries)
                }
                Err(error) => return Err(error.into()),
            };
            tokio::time::sleep(wait).await;
            retries += 1;
        }
    }

    /// Sends one attempt of a request.
    async fn attempt(
        &self,
        method: &Method,
        url: Url,
        token: Option<&AuthToken>,
        payload: &mut Payload,
    ) -> std::result::Result<Response, reqwest::Error> {
        let mut request = self
            .inner
            .http
            .request(method.clone(), url)
            .header(USER_AGENT, self.inner.user_agent.clone());
        if let Some(token) = token {
            request = request.bearer_auth(&token.access_token);
        }
        match payload {
            Payload::Empty => {}
            Payload::Json(body) => {
                request = request
                    .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
                    .body(body.clone());
            }
            Payload::Form(form) => {
                if let Some(form) = form.take() {
                    request = request.multipart(form);
                }
            }
        }
        request.send().await
    }
}

/// The calls that are safe to repeat: `GET`, which only reads, and `PUT`, whose scan id makes a
/// repeat harmless.
fn can_retry(method: &Method) -> bool {
    *method == Method::GET || *method == Method::PUT
}

/// Turns a response into a [`Reply`] for a success status, or into an [`Error`] that keeps the
/// start of the body, where the server usually explains the error.
async fn finish(method: Method, mut response: Response) -> Result<Reply> {
    let status = response.status();
    if status.is_success() {
        return Ok(Reply { response, method });
    }
    let retry_after = retry::retry_after(response.headers());
    let endpoint = endpoint(&method, &response);
    // A read failure keeps the bytes read before it.
    let mut bytes = Vec::new();
    let _ = read_up_to(&mut response, MAX_ERROR_BODY, &mut bytes).await;
    let body = String::from_utf8_lossy(&bytes).into_owned();
    Err(match status.as_u16() {
        401 => Error::Unauthorized { body, endpoint },
        429 => Error::RateLimited {
            body,
            endpoint,
            retry_after,
        },
        503 => Error::UnderMaintenance {
            body,
            endpoint,
            retry_after,
        },
        status => Error::Api {
            status,
            body,
            endpoint,
        },
    })
}

/// The method and path of a request, for error messages, for example `GET /v3/scans/credits`.
fn endpoint(method: &Method, response: &Response) -> String {
    format!("{method} {}", response.url().path())
}

/// Reads up to 64 KiB of a body that is not used, so that the connection can serve the next
/// request. Errors are ignored: the connection is then simply not reused.
async fn drain(response: &mut Response) {
    let mut read = 0;
    while read <= MAX_DRAIN_BODY {
        match response.chunk().await {
            Ok(Some(chunk)) => read += chunk.len(),
            _ => break,
        }
    }
}

/// A multipart form with the image as the `image` file part, then the text fields.
fn image_form(image: Bytes, filename: &str, fields: Vec<(&'static str, String)>) -> Result<Form> {
    let length = u64::try_from(image.len()).unwrap_or(u64::MAX);
    let part = Part::stream_with_length(reqwest::Body::from(image), length)
        .file_name(filename.to_owned())
        .mime_str(ai_image::mime_type(filename))?;
    let mut form = Form::new().part("image", part);
    for (name, value) in fields {
        form = form.text(name, value);
    }
    Ok(form)
}

/// A successful response, and the method of its request.
struct Reply {
    response: Response,
    method: Method,
}

impl Reply {
    /// Decodes the body as JSON, off the async runtime when it is large. The body may be at most
    /// 16 MiB.
    async fn json<T: DeserializeOwned + Send + 'static>(self) -> Result<T> {
        let body = Bytes::from(self.body(MAX_JSON_BODY).await?);
        off_runtime(body.len(), move || {
            serde_json::from_slice::<T>(&body).map_err(Error::from)
        })
        .await
    }

    /// Reads up to 64 KiB of a body the call does not use, so that the connection is reused.
    async fn drain(mut self) {
        drain(&mut self.response).await;
    }

    /// Reads the whole body, or fails with [`Error::ResponseTooLarge`] when it is longer than
    /// `limit` bytes. The body is never truncated.
    async fn body(mut self, limit: usize) -> Result<Vec<u8>> {
        let declared = self.response.content_length();
        let declared_too_large =
            declared.is_some_and(|length| length > u64::try_from(limit).unwrap_or(u64::MAX));
        if !declared_too_large {
            let capacity = declared
                .and_then(|length| usize::try_from(length).ok())
                .unwrap_or(0);
            let mut body = Vec::with_capacity(capacity.min(limit));
            if read_up_to(&mut self.response, limit, &mut body).await? {
                return Ok(body);
            }
        }
        Err(Error::ResponseTooLarge {
            endpoint: endpoint(&self.method, &self.response),
            limit,
        })
    }
}

/// Reads the body chunk by chunk into `body`, up to `limit` bytes. Returns `true` when the whole
/// body was read, and `false` when it is longer than `limit` and was cut. On an error, `body`
/// keeps the bytes read before it.
async fn read_up_to(response: &mut Response, limit: usize, body: &mut Vec<u8>) -> Result<bool> {
    while let Some(chunk) = response.chunk().await? {
        let room = limit - body.len();
        if chunk.len() > room {
            body.extend_from_slice(&chunk[..room]);
            return Ok(false);
        }
        body.extend_from_slice(&chunk);
    }
    Ok(true)
}

/// Configures a [`Client`]. Create it with [`Client::builder`].
///
/// ```
/// use std::time::Duration;
/// use copyleaks::RetryPolicy;
///
/// # fn run() -> Result<(), copyleaks::Error> {
/// let client = copyleaks::Client::builder()
///     .user_agent("my-app/1.0")
///     .timeout(Duration::from_secs(300))
///     .retry_policy(RetryPolicy::new(3))
///     .build()?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Default)]
pub struct ClientBuilder {
    http_client: Option<reqwest::Client>,
    api_base_url: Option<String>,
    identity_base_url: Option<String>,
    user_agent: Option<String>,
    timeout: Option<Duration>,
    connect_timeout: Option<Duration>,
    retry: Option<RetryPolicy>,
}

impl ClientBuilder {
    /// Uses your own `reqwest` client, for example for a proxy or a corporate CA (the default
    /// client trusts only the Mozilla roots). Set timeouts on that client:
    /// [`timeout`](Self::timeout) and [`connect_timeout`](Self::connect_timeout) apply to the
    /// default client only.
    #[must_use]
    pub fn http_client(mut self, client: reqwest::Client) -> Self {
        self.http_client = Some(client);
        self
    }

    /// The time limit of each request of the default HTTP client, from connecting to reading the
    /// whole response, uploads included. Default: 120 seconds.
    #[must_use]
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// The time limit to open a connection, for the default HTTP client. Default: 10 seconds.
    #[must_use]
    pub fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = Some(timeout);
        self
    }

    /// Retries the calls that are safe to repeat; see [`RetryPolicy`]. Default: no retries.
    #[must_use]
    pub fn retry_policy(mut self, policy: RetryPolicy) -> Self {
        self.retry = Some(policy);
        self
    }

    /// Overrides the API base URL. Default: [`DEFAULT_API_BASE_URL`]. Must be `https`; `http` is
    /// accepted only for `localhost`, `127.0.0.1` and `::1`.
    #[must_use]
    pub fn api_base_url(mut self, url: impl Into<String>) -> Self {
        self.api_base_url = Some(url.into());
        self
    }

    /// Overrides the identity server base URL, used by [`Client::login`]. Default:
    /// [`DEFAULT_IDENTITY_BASE_URL`]. Same `https` rule as [`api_base_url`](Self::api_base_url).
    #[must_use]
    pub fn identity_base_url(mut self, url: impl Into<String>) -> Self {
        self.identity_base_url = Some(url.into());
        self
    }

    /// Adds your application's product token in front of [`DEFAULT_USER_AGENT`], for example
    /// `my-app/2.1 copyleaks-rust-sdk/2.0.0`.
    #[must_use]
    pub fn user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = Some(user_agent.into());
        self
    }

    /// Creates the client.
    ///
    /// # Errors
    ///
    /// [`Error::Validation`] for a base URL that is not `https` (or `http` on a loopback host),
    /// a user agent that is not a valid header value, or a timeout set together with
    /// [`http_client`](Self::http_client); and [`Error::Http`] when the default HTTP client
    /// cannot be created.
    pub fn build(self) -> Result<Client> {
        let api_base = base_url(
            self.api_base_url.as_deref().unwrap_or(DEFAULT_API_BASE_URL),
            "api_base_url",
        )?;
        let identity_base = base_url(
            self.identity_base_url
                .as_deref()
                .unwrap_or(DEFAULT_IDENTITY_BASE_URL),
            "identity_base_url",
        )?;
        let http = match self.http_client {
            Some(_) if self.timeout.is_some() || self.connect_timeout.is_some() => {
                return Err(Error::Validation(
                    "timeout and connect_timeout apply to the default HTTP client; set them on \
                     the client passed to http_client"
                        .to_owned(),
                ));
            }
            Some(client) => client,
            None => default_http_client(
                self.timeout.unwrap_or(DEFAULT_TIMEOUT),
                self.connect_timeout.unwrap_or(DEFAULT_CONNECT_TIMEOUT),
            )?,
        };
        let user_agent = match self.user_agent.as_deref().map(str::trim) {
            Some(app) if !app.is_empty() => format!("{app} {DEFAULT_USER_AGENT}"),
            _ => DEFAULT_USER_AGENT.to_owned(),
        };
        let user_agent = HeaderValue::from_str(&user_agent).map_err(|_| {
            Error::Validation("user_agent must be visible ASCII characters".to_owned())
        })?;
        Ok(Client {
            inner: Arc::new(Inner {
                http,
                api_base,
                identity_base,
                user_agent,
                retry: self.retry,
            }),
        })
    }
}

/// The default HTTP client: HTTP/2 when the server offers it, gzip responses, and the timeouts.
fn default_http_client(timeout: Duration, connect_timeout: Duration) -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .timeout(timeout)
        .connect_timeout(connect_timeout)
        .gzip(true)
        .build()?)
}

/// Parses a base URL: `https`, or `http` on a loopback host. The error does not repeat the URL,
/// which may hold credentials.
fn base_url(value: &str, name: &str) -> Result<Url> {
    let url = Url::parse(value).map_err(|_| {
        Error::Validation(format!(
            "{name} needs an absolute URL, such as {DEFAULT_API_BASE_URL}"
        ))
    })?;
    let loopback = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
    let secure = url.scheme() == "https" || (url.scheme() == "http" && loopback);
    if !secure || url.cannot_be_a_base() || url.host_str().is_none() {
        return Err(Error::Validation(format!(
            "{name} needs an https URL; http is only accepted for localhost, 127.0.0.1 and ::1"
        )));
    }
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_get_and_put_are_retried() {
        assert!(can_retry(&Method::GET));
        assert!(can_retry(&Method::PUT));
        assert!(!can_retry(&Method::POST));
        assert!(!can_retry(&Method::PATCH));
        assert!(!Payload::Form(None).can_repeat());
        assert!(Payload::Empty.can_repeat());
    }

    #[test]
    fn default_timeouts() {
        assert_eq!(DEFAULT_TIMEOUT, Duration::from_secs(120));
        assert_eq!(DEFAULT_CONNECT_TIMEOUT, Duration::from_secs(10));
        default_http_client(DEFAULT_TIMEOUT, DEFAULT_CONNECT_TIMEOUT).expect("default client");
    }

    #[test]
    fn invalid_user_agent_is_rejected() {
        let error = ClientBuilder::default()
            .user_agent("my-app/1.0\n")
            .build()
            .err();
        assert!(error.is_none(), "trimmed");
        let error = ClientBuilder::default()
            .user_agent("my\napp")
            .build()
            .expect_err("newline");
        assert!(matches!(error, Error::Validation(_)), "{error:?}");
    }
}
