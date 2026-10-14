//! The error type of the SDK.

use std::time::Duration;

/// A `Result` whose error type defaults to [`Error`].
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// How much of a response body an error message shows, in bytes.
const MAX_DISPLAY_BODY: usize = 512;

/// Everything that can go wrong when calling the Copyleaks API.
///
/// The variants of an error status keep the response body (up to 1 MiB), where the server
/// usually explains the error. Error messages show at most its first 512 bytes, and never
/// contain the login token or the API key.
///
/// The enum and its struct variants are `#[non_exhaustive]`: match struct variants with `..`,
/// for example `Error::Api { status, .. }`.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The server rejected the request with a status code that has no dedicated variant.
    #[error("Copyleaks API error: HTTP {status} from {endpoint}{}", detail(.body))]
    #[non_exhaustive]
    Api {
        /// The HTTP status code.
        status: u16,
        /// The response body as text, up to 1 MiB. Often a JSON error object from the server.
        body: String,
        /// The HTTP method and path that failed, for example `PUT /v3/scans/submit/file/my-scan`.
        endpoint: String,
    },
    /// HTTP 401: the login token or the API key was rejected.
    #[error("unauthorized (HTTP 401) from {endpoint}{}", detail(.body))]
    #[non_exhaustive]
    Unauthorized {
        /// The response body as text, up to 1 MiB.
        body: String,
        /// The HTTP method and path that failed.
        endpoint: String,
    },
    /// HTTP 429: too many requests. Wait before calling again: `retry_after` when the server
    /// says how long, or with exponential backoff.
    #[error("rate limited (HTTP 429) from {endpoint}: too many requests{}", detail(.body))]
    #[non_exhaustive]
    RateLimited {
        /// The response body as text, up to 1 MiB.
        body: String,
        /// The HTTP method and path that failed.
        endpoint: String,
        /// How long the server asked to wait, from its `Retry-After` header, if it sent one.
        retry_after: Option<Duration>,
    },
    /// HTTP 503: Copyleaks is under maintenance. Retry later: after `retry_after` when the
    /// server says how long, or with exponential backoff.
    #[error("under maintenance (HTTP 503) at {endpoint}; retry later{}", detail(.body))]
    #[non_exhaustive]
    UnderMaintenance {
        /// The response body as text, up to 1 MiB.
        body: String,
        /// The HTTP method and path that failed.
        endpoint: String,
        /// How long the server asked to wait, from its `Retry-After` header, if it sent one.
        retry_after: Option<Duration>,
    },
    /// The login token has expired or expires within 5 minutes. Log in again.
    /// Checked locally before every authenticated call, so no request is sent.
    #[error("the login token has expired or expires within 5 minutes; log in again")]
    TokenExpired,
    /// A successful response body is larger than the SDK reads: 16 MiB for JSON and 64 MiB for
    /// the usage history CSV. The body is not truncated; the call fails.
    #[error("the response of {endpoint} is larger than {limit} bytes; the call failed")]
    #[non_exhaustive]
    ResponseTooLarge {
        /// The HTTP method and path of the call.
        endpoint: String,
        /// The largest body accepted, in bytes.
        limit: usize,
    },
    /// The request could not be sent or the response could not be read.
    #[error("HTTP transport error: {0}")]
    Http(#[from] reqwest::Error),
    /// A JSON body could not be decoded.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    /// The request failed a client-side check and was not sent.
    #[error("invalid request: {0}")]
    Validation(String),
}

impl Error {
    /// The HTTP status code of an error response, or `None` when the server did not answer
    /// with an error status.
    pub fn status(&self) -> Option<u16> {
        match self {
            Error::Api { status, .. } => Some(*status),
            Error::Unauthorized { .. } => Some(401),
            Error::RateLimited { .. } => Some(429),
            Error::UnderMaintenance { .. } => Some(503),
            _ => None,
        }
    }

    /// How long the server asked to wait before calling again (its `Retry-After` header), for
    /// [`Error::RateLimited`] and [`Error::UnderMaintenance`]. `None` when it did not say.
    pub fn retry_after(&self) -> Option<Duration> {
        match self {
            Error::RateLimited { retry_after, .. }
            | Error::UnderMaintenance { retry_after, .. } => *retry_after,
            _ => None,
        }
    }
}

/// `": <body>"` with the body trimmed and cut to 512 bytes, or nothing for an empty body.
fn detail(body: &str) -> String {
    let body = body.trim();
    if body.is_empty() {
        return String::new();
    }
    if body.len() <= MAX_DISPLAY_BODY {
        return format!(": {body}");
    }
    let mut cut = MAX_DISPLAY_BODY;
    while !body.is_char_boundary(cut) {
        cut -= 1;
    }
    format!(": {}...", &body[..cut])
}

#[cfg(test)]
mod tests {
    use super::detail;

    #[test]
    fn detail_trims_and_cuts_the_body() {
        assert_eq!(detail(""), "");
        assert_eq!(detail("  \n"), "");
        assert_eq!(detail(" bad scan id "), ": bad scan id");
        let long = "a".repeat(600);
        assert_eq!(detail(&long), format!(": {}...", "a".repeat(512)));
        // A multi-byte character across the cut is not split.
        let multi = format!("{}é{}", "a".repeat(511), "b".repeat(100));
        assert_eq!(detail(&multi), format!(": {}...", "a".repeat(511)));
    }
}
