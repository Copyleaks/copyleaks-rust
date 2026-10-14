//! The Copyleaks API client.

use std::sync::Arc;
use std::time::Duration;

use reqwest::header::USER_AGENT;
use reqwest::{Method, Response, Url};
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::ai_image::{AiImageDetectionRequest, AiImageDetectionResponse};
use crate::ai_text::{AiTextDetectionRequest, AiTextDetectionResponse};
use crate::grammar::{CorrectionTypes, GrammarCheckRequest, GrammarCheckResponse};
use crate::misc::{ReleaseNotes, SupportedFileTypes};
use crate::moderation::{TextModerationRequest, TextModerationResponse};
use crate::plagiarism::{
    CreditsBalance, Delete, Export, FileSubmission, OcrSubmission, Start, StartResponse,
    UrlSubmission,
};
use crate::{validate, AuthToken, Error, Result};

/// The default base URL of the Copyleaks API.
pub const DEFAULT_API_BASE_URL: &str = "https://api.copyleaks.com";
/// The default base URL of the Copyleaks identity server, used by [`Client::login`].
pub const DEFAULT_IDENTITY_BASE_URL: &str = "https://id.copyleaks.com";
/// The SDK's own `User-Agent` product token, `copyleaks-rust-sdk/<crate version>`, sent on every
/// request after the token set with
/// [`ClientBuilder::user_agent`](method@ClientBuilder::user_agent), if any.
pub const DEFAULT_USER_AGENT: &str = concat!("copyleaks-rust-sdk/", env!("CARGO_PKG_VERSION"));

/// The timeout of each request of the default HTTP client.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);

/// The largest JSON response body the client reads: 16 MiB.
const MAX_JSON_BODY: usize = 16 << 20;

/// The largest usage history CSV the client reads: 64 MiB.
const MAX_CSV_BODY: usize = 64 << 20;

/// How much of an error response body the client keeps: 1 MiB.
const MAX_ERROR_BODY: usize = 1 << 20;

#[derive(Debug)]
struct Inner {
    http: reqwest::Client,
    api_base: Url,
    identity_base: Url,
    user_agent: String,
}

/// An async client for the Copyleaks API.
///
/// Cloning is cheap: clones share one connection pool. Create one client and reuse it.
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

impl Client {
    /// Creates a client with the default settings: the production URLs and an HTTP client with a
    /// 120-second timeout per request.
    ///
    /// # Errors
    ///
    /// [`Error::Http`] when the HTTP client cannot be created, for example when the TLS backend
    /// fails to initialize.
    pub fn new() -> Result<Self> {
        Self::builder().build()
    }

    /// Returns a builder to configure the HTTP client, base URLs and user agent.
    pub fn builder() -> ClientBuilder {
        ClientBuilder::default()
    }

    /// Logs in with your account email and API key, and returns a token valid for 48 hours.
    ///
    /// Log in once and reuse the token: the login endpoint allows 12 requests per 15 minutes.
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
        let reply = self
            .send(Method::POST, url, None, Some(&Login { email, key }))
            .await?;
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
        self.send(Method::PUT, url, Some(token), Some(submission))
            .await?;
        Ok(())
    }

    /// Submits an image or scanned document whose text is extracted with OCR, for a plagiarism
    /// scan. Returns when Copyleaks accepts the scan; the result arrives at the status webhook.
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
        self.send(Method::PUT, url, Some(token), Some(submission))
            .await?;
        Ok(())
    }

    /// Submits a URL for a plagiarism scan. Returns when Copyleaks accepts the scan; the result
    /// arrives at the status webhook.
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
        self.send(Method::PUT, url, Some(token), Some(submission))
            .await?;
        Ok(())
    }

    /// Exports the artifacts of a completed scan (results, PDF report, crawled version) to your
    /// servers. When the export is done, Copyleaks calls the export's completion webhook.
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
        self.send(Method::POST, url, Some(token), Some(export))
            .await?;
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
        let reply = self
            .send(Method::PATCH, url, Some(token), Some(start))
            .await?;
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
        self.send(Method::PATCH, url, Some(token), Some(delete))
            .await?;
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
        self.send::<()>(Method::POST, url, Some(token), None)
            .await?;
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
        let reply = self.send::<()>(Method::GET, url, Some(token), None).await?;
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
        let reply = self.send::<()>(Method::GET, url, Some(token), None).await?;
        reply.body(MAX_CSV_BODY).await
    }

    /// Returns the API release notes. Needs no login.
    ///
    /// # Errors
    ///
    /// The [`Error`] variants for transport, server and decoding failures.
    pub async fn release_notes(&self) -> Result<ReleaseNotes> {
        let url = self.url(Server::Api, &["v3", "release-logs.json"])?;
        let reply = self.send::<()>(Method::GET, url, None, None).await?;
        reply.json().await
    }

    /// Returns the file types the Plagiarism Checker accepts. Needs no login.
    ///
    /// # Errors
    ///
    /// The [`Error`] variants for transport, server and decoding failures.
    pub async fn supported_file_types(&self) -> Result<SupportedFileTypes> {
        let url = self.url(
            Server::Api,
            &["v3", "miscellaneous", "supported-file-types"],
        )?;
        let reply = self.send::<()>(Method::GET, url, None, None).await?;
        reply.json().await
    }

    /// Returns the language codes that OCR supports, for [`Client::submit_file_ocr`].
    /// Needs no login.
    ///
    /// # Errors
    ///
    /// The [`Error`] variants for transport, server and decoding failures.
    pub async fn ocr_supported_languages(&self) -> Result<Vec<String>> {
        let url = self.url(Server::Api, &["v3", "miscellaneous", "ocr-languages-list"])?;
        let reply = self.send::<()>(Method::GET, url, None, None).await?;
        reply.json().await
    }

    /// AI Text Detection: classifies the sections of a text as human-written or AI-generated.
    ///
    /// # Errors
    ///
    /// [`Error::Validation`] for a scan id that is not 3 to 36 characters long, a text shorter
    /// than 255 characters, or a sensitivity outside 1 to 3, and the other [`Error`] variants
    /// for other failures.
    pub async fn ai_text_detection(
        &self,
        token: &AuthToken,
        scan_id: &str,
        request: &AiTextDetectionRequest,
    ) -> Result<AiTextDetectionResponse> {
        validate::id_length(scan_id, "scan id")?;
        request.validate()?;
        let url = self.url(Server::Api, &["v2", "writer-detector", scan_id, "check"])?;
        let reply = self
            .send(Method::POST, url, Some(token), Some(request))
            .await?;
        reply.json().await
    }

    /// AI Image Detection: finds whether an image, or part of it, was generated by AI.
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
        let reply = self
            .send(Method::POST, url, Some(token), Some(request))
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
        let reply = self
            .send(Method::POST, url, Some(token), Some(request))
            .await?;
        reply.json().await
    }

    /// Returns the Grammar Checker correction types, with their texts in the given language
    /// (for example `en`). Needs no login.
    ///
    /// # Errors
    ///
    /// The [`Error`] variants for transport, server and decoding failures.
    pub async fn grammar_correction_types(&self, language_code: &str) -> Result<CorrectionTypes> {
        validate::required(language_code, "language code")?;
        let url = self.url(
            Server::Api,
            &["v1", "writing-feedback", "correction-types", language_code],
        )?;
        let reply = self.send::<()>(Method::GET, url, None, None).await?;
        reply.json().await
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
        let reply = self
            .send(Method::POST, url, Some(token), Some(request))
            .await?;
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

    /// Sends a request with the common headers, and maps an error status to an [`Error`].
    /// When a token is given, it is checked locally first, then sent as a bearer token.
    async fn send<B: Serialize + ?Sized>(
        &self,
        method: Method,
        url: Url,
        token: Option<&AuthToken>,
        body: Option<&B>,
    ) -> Result<Reply> {
        let endpoint = format!("{method} {}", url.path());
        let mut request = self
            .inner
            .http
            .request(method, url)
            .header(USER_AGENT, self.inner.user_agent.as_str());
        if let Some(token) = token {
            token.verify()?;
            request = request.bearer_auth(&token.access_token);
        }
        if let Some(body) = body {
            request = request.json(body);
        }
        let mut response = request.send().await?;
        let status = response.status();
        if status.is_success() {
            return Ok(Reply { response, endpoint });
        }
        // Keep the start of the body: the server usually explains the error there. A read
        // failure keeps the bytes read before it.
        let mut bytes = Vec::new();
        let _ = read_up_to(&mut response, MAX_ERROR_BODY, &mut bytes).await;
        let body = String::from_utf8_lossy(&bytes).into_owned();
        Err(match status.as_u16() {
            401 => Error::Unauthorized { body, endpoint },
            429 => Error::RateLimited { body, endpoint },
            503 => Error::UnderMaintenance { body, endpoint },
            status => Error::Api {
                status,
                body,
                endpoint,
            },
        })
    }
}

/// A successful response, and the method and path of its request.
struct Reply {
    response: Response,
    endpoint: String,
}

impl Reply {
    /// Decodes the body as JSON. The body may be at most 16 MiB.
    async fn json<T: DeserializeOwned>(self) -> Result<T> {
        let body = self.body(MAX_JSON_BODY).await?;
        Ok(serde_json::from_slice(&body)?)
    }

    /// Reads the whole body, or fails with [`Error::ResponseTooLarge`] when it is longer than
    /// `limit` bytes. The body is never truncated.
    async fn body(mut self, limit: usize) -> Result<Vec<u8>> {
        let declared_too_large = self
            .response
            .content_length()
            .is_some_and(|length| length > u64::try_from(limit).unwrap_or(u64::MAX));
        if !declared_too_large {
            let mut body = Vec::new();
            if read_up_to(&mut self.response, limit, &mut body).await? {
                return Ok(body);
            }
        }
        Err(Error::ResponseTooLarge {
            endpoint: self.endpoint,
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
/// # fn run() -> Result<(), copyleaks::Error> {
/// let client = copyleaks::Client::builder()
///     .user_agent("my-app/1.0")
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
}

impl ClientBuilder {
    /// Uses your own `reqwest` client instead of the default one, which has a 120-second timeout
    /// per request. Use it to set other timeouts, a proxy, or extra root certificates, for
    /// example a corporate CA: the default client trusts only the Mozilla root certificates.
    #[must_use]
    pub fn http_client(mut self, client: reqwest::Client) -> Self {
        self.http_client = Some(client);
        self
    }

    /// Overrides the API base URL. Default: [`DEFAULT_API_BASE_URL`].
    ///
    /// The URL must use `https`; `http` is only accepted for `localhost`, `127.0.0.1` and `::1`,
    /// for local test servers.
    #[must_use]
    pub fn api_base_url(mut self, url: impl Into<String>) -> Self {
        self.api_base_url = Some(url.into());
        self
    }

    /// Overrides the identity server base URL, used by [`Client::login`].
    /// Default: [`DEFAULT_IDENTITY_BASE_URL`]. The same `https` rule as
    /// [`api_base_url`](Self::api_base_url) applies.
    #[must_use]
    pub fn identity_base_url(mut self, url: impl Into<String>) -> Self {
        self.identity_base_url = Some(url.into());
        self
    }

    /// Adds your application's product token, such as `my-app/2.1`, in front of the SDK's own
    /// token [`DEFAULT_USER_AGENT`], which is always sent: `my-app/2.1 copyleaks-rust-sdk/1.0.0`.
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
    /// and [`Error::Http`] when the default HTTP client cannot be created.
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
            Some(client) => client,
            None => reqwest::Client::builder()
                .timeout(DEFAULT_TIMEOUT)
                .build()?,
        };
        let user_agent = match self.user_agent.as_deref().map(str::trim) {
            Some(app) if !app.is_empty() => format!("{app} {DEFAULT_USER_AGENT}"),
            _ => DEFAULT_USER_AGENT.to_owned(),
        };
        Ok(Client {
            inner: Arc::new(Inner {
                http,
                api_base,
                identity_base,
                user_agent,
            }),
        })
    }
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
