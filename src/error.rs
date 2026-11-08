//! The error type of the SDK.

use std::time::Duration;

use serde_json::{Map, Value};

/// A `Result` whose error type defaults to [`Error`].
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// How much of a response body an error message shows, in bytes.
const MAX_DISPLAY_BODY: usize = 512;

/// Everything that can go wrong when calling the Copyleaks API.
///
/// Errors for an HTTP error status keep up to 1 MiB of the response body; messages show at most
/// 512 bytes of it and never contain the login token or the API key. Match struct variants with
/// `..`, for example `Error::Api { status, .. }`.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The server rejected the request with a status code that has no dedicated variant.
    #[error("Copyleaks API error: HTTP {status} from {endpoint}{}", detail(.body))]
    #[non_exhaustive]
    Api {
        /// The HTTP status code.
        status: u16,
        /// The response body as text, often a JSON error object.
        body: String,
        /// The HTTP method and path that failed, for example `PUT /v3/scans/submit/file/my-scan`.
        endpoint: String,
    },
    /// HTTP 401: the login token or the API key was rejected.
    #[error("unauthorized (HTTP 401) from {endpoint}{}", detail(.body))]
    #[non_exhaustive]
    Unauthorized {
        /// The response body as text.
        body: String,
        /// The HTTP method and path that failed.
        endpoint: String,
    },
    /// HTTP 429: too many requests. Wait before calling again: `retry_after` when the server
    /// says how long, or with exponential backoff.
    #[error("rate limited (HTTP 429) from {endpoint}: too many requests{}", detail(.body))]
    #[non_exhaustive]
    RateLimited {
        /// The response body as text.
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
        /// The response body as text.
        body: String,
        /// The HTTP method and path that failed.
        endpoint: String,
        /// How long the server asked to wait, from its `Retry-After` header, if it sent one.
        retry_after: Option<Duration>,
    },
    /// The login token has expired or expires within 5 minutes; log in again. No request was sent.
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

    /// Parses the response body of an error status ([`Error::Api`], [`Error::Unauthorized`],
    /// [`Error::RateLimited`] or [`Error::UnderMaintenance`]) as the error object of the API.
    /// `None` for the other variants, and when the body is not a JSON object with an `error`
    /// object. It never fails; see [`ApiErrorBody`] for how each field is read.
    pub fn api_error_body(&self) -> Option<ApiErrorBody> {
        match self {
            Error::Api { body, .. }
            | Error::Unauthorized { body, .. }
            | Error::RateLimited { body, .. }
            | Error::UnderMaintenance { body, .. } => ApiErrorBody::parse(body),
            _ => None,
        }
    }
}

/// The error object that the API sends in the body of an error response:
/// `{"error": {"type": ..., "id": ..., "code": ..., "message": ..., "url": ..., "details": [...]}}`.
/// See <https://docs.copyleaks.com/using-the-apis/api-errors/>. Read it with
/// [`Error::api_error_body`].
///
/// A field that the server did not send, or sent with an unexpected JSON type, is `None` (and
/// `details` is empty). It is the same object as
/// [`WebhookError`](crate::webhooks::WebhookError), which an error webhook carries.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct ApiErrorBody {
    /// The error type: one of the constants of this type, such as
    /// [`ApiErrorBody::INVALID_REQUEST_ERROR`], or a type that the API adds later.
    pub error_type: Option<String>,
    /// The machine-readable error id, for example `missing_parameter`.
    pub id: Option<String>,
    /// The numeric error code, from a JSON integer or a numeric string (surrounding whitespace
    /// ignored).
    pub code: Option<i32>,
    /// A human-readable message.
    pub message: Option<String>,
    /// A link to the documentation of this error.
    pub url: Option<String>,
    /// The parameters that caused the error: the entries of `details` that are JSON objects.
    /// Empty when the server sent none.
    pub details: Vec<ApiErrorDetail>,
}

/// A parameter that caused an API error.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct ApiErrorDetail {
    /// The parameter name.
    pub param: Option<String>,
    /// What is wrong with the parameter.
    pub message: Option<String>,
}

/// The error types of [`ApiErrorBody::error_type`], from
/// <https://docs.copyleaks.com/using-the-apis/api-errors/>. They also apply to
/// [`WebhookError::error_type`](crate::webhooks::WebhookError::error_type).
impl ApiErrorBody {
    /// The `error_type` of an error caused by the request, such as a missing parameter.
    pub const INVALID_REQUEST_ERROR: &'static str = "invalid_request_error";
    /// The `error_type` of an authentication error.
    pub const AUTHENTICATION_ERROR: &'static str = "authentication_error";
    /// The `error_type` of a payment error.
    pub const PAYMENT_ERROR: &'static str = "payment_error";
    /// The `error_type` of an error on the Copyleaks side.
    pub const API_ERROR: &'static str = "api_error";

    /// Reads `{"error": {...}}`; `None` when `body` is not a JSON object with an `error` object.
    fn parse(body: &str) -> Option<Self> {
        let value: Value = serde_json::from_str(body).ok()?;
        let error = value.get("error")?.as_object()?;
        let code = match error.get("code") {
            Some(Value::Number(number)) => number.as_i64().and_then(|n| i32::try_from(n).ok()),
            Some(Value::String(text)) => text.trim().parse().ok(),
            _ => None,
        };
        let details = match error.get("details") {
            Some(Value::Array(items)) => items
                .iter()
                .filter_map(Value::as_object)
                .map(|detail| ApiErrorDetail {
                    param: text(detail, "param"),
                    message: text(detail, "message"),
                })
                .collect(),
            _ => Vec::new(),
        };
        Some(ApiErrorBody {
            error_type: text(error, "type"),
            id: text(error, "id"),
            code,
            message: text(error, "message"),
            url: text(error, "url"),
            details,
        })
    }
}

/// The string member `key` of `object`, or `None` when it is missing or not a string.
fn text(object: &Map<String, Value>, key: &str) -> Option<String> {
    object.get(key).and_then(Value::as_str).map(str::to_owned)
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
    use super::{detail, ApiErrorBody, ApiErrorDetail};

    fn detail_of(param: Option<&str>, message: Option<&str>) -> ApiErrorDetail {
        ApiErrorDetail {
            param: param.map(str::to_owned),
            message: message.map(str::to_owned),
        }
    }

    #[test]
    fn api_error_body_reads_the_full_envelope() {
        let body = r#"{"error":{"type":"invalid_request_error","id":"missing_parameter","code":1,
            "message":"Bad request. One or several required parameters are missing or incorrect.",
            "url":"https://docs.copyleaks.com/using-the-apis/api-errors/#error-reference",
            "details":[{"param":"labels","message":"Id Label EXAMPLE-v1 is not supported"}]}}"#;
        let expected = ApiErrorBody {
            error_type: Some(ApiErrorBody::INVALID_REQUEST_ERROR.to_owned()),
            id: Some("missing_parameter".to_owned()),
            code: Some(1),
            message: Some(
                "Bad request. One or several required parameters are missing or incorrect."
                    .to_owned(),
            ),
            url: Some(
                "https://docs.copyleaks.com/using-the-apis/api-errors/#error-reference".to_owned(),
            ),
            details: vec![detail_of(
                Some("labels"),
                Some("Id Label EXAMPLE-v1 is not supported"),
            )],
        };
        assert_eq!(ApiErrorBody::parse(body), Some(expected));
    }

    #[test]
    fn api_error_body_leaves_missing_fields_empty() {
        let parsed = ApiErrorBody::parse(r#"{"error":{"type":"payment_error","code":7}}"#);
        let expected = ApiErrorBody {
            error_type: Some(ApiErrorBody::PAYMENT_ERROR.to_owned()),
            code: Some(7),
            ..ApiErrorBody::default()
        };
        assert_eq!(parsed, Some(expected));
        assert_eq!(
            ApiErrorBody::parse(r#"{"error":{}}"#),
            Some(ApiErrorBody::default())
        );
        // A field of an unexpected JSON type is empty; an unknown type comes through.
        let parsed = ApiErrorBody::parse(
            r#"{"error":{"type":"future_error","id":true,"message":["m"],"url":null}}"#,
        );
        let expected = ApiErrorBody {
            error_type: Some("future_error".to_owned()),
            ..ApiErrorBody::default()
        };
        assert_eq!(parsed, Some(expected));
    }

    #[test]
    fn api_error_body_reads_the_code_from_an_integer_or_a_numeric_string() {
        let code = |body: &str| ApiErrorBody::parse(body).and_then(|parsed| parsed.code);
        assert_eq!(code(r#"{"error":{"code":42}}"#), Some(42));
        assert_eq!(code(r#"{"error":{"code":"42"}}"#), Some(42));
        assert_eq!(code(r#"{"error":{"code":" 7 "}}"#), Some(7));
        assert_eq!(code(r#"{"error":{"code":"-3"}}"#), Some(-3));
        assert_eq!(code(r#"{"error":{"code":"1 2"}}"#), None);
        assert_eq!(code(r#"{"error":{"code":" "}}"#), None);
        assert_eq!(code(r#"{"error":{"code":"forty-two"}}"#), None);
        assert_eq!(code(r#"{"error":{"code":1.5}}"#), None);
        assert_eq!(code(r#"{"error":{"code":4294967296}}"#), None);
        assert_eq!(code(r#"{"error":{"code":{"value":1}}}"#), None);
        assert_eq!(code(r#"{"error":{"code":null}}"#), None);
    }

    #[test]
    fn api_error_body_is_none_without_an_error_object() {
        for body in [
            "",
            "<html>Bad Gateway</html>",
            "Authorization has been denied",
            r#"{"message":"the server explains"}"#,
            r#"{"error":"boom"}"#,
            r#"{"error":null}"#,
            r#"[{"error":{"type":"api_error"}}]"#,
            "null",
        ] {
            assert_eq!(ApiErrorBody::parse(body), None, "{body}");
        }
    }

    #[test]
    fn api_error_body_keeps_the_details_that_are_objects() {
        let details = |body: &str| ApiErrorBody::parse(body).map(|parsed| parsed.details);
        assert_eq!(
            details(r#"{"error":{"details":{"param":"a"}}}"#),
            Some(Vec::new())
        );
        assert_eq!(details(r#"{"error":{"details":null}}"#), Some(Vec::new()));
        assert_eq!(
            details(
                r#"{"error":{"details":[null,"text",1,{"param":2,"message":"m"},{},{"param":"p"}]}}"#
            ),
            Some(vec![
                detail_of(None, Some("m")),
                detail_of(None, None),
                detail_of(Some("p"), None),
            ])
        );
    }

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
