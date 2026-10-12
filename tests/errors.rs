//! Error mapping: HTTP status codes, decoding and transport failures, the local token check,
//! and that no error or debug output contains the token.

mod common;

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;

use common::{client, token, TOKEN};
use copyleaks::plagiarism::{FileSubmission, ScanProperties};
use copyleaks::{AuthToken, Client, Error};
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Serves one response on a local port: a JSON list of `size` bytes, streamed with chunked
/// transfer encoding, so the response has no Content-Length. Returns the base URL.
fn serve_streamed_json(size: usize) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let base = format!("http://{}", listener.local_addr().expect("local address"));
    std::thread::spawn(move || {
        let Ok((mut stream, _)) = listener.accept() else {
            return;
        };
        let mut reader = BufReader::new(stream.try_clone().expect("clone stream"));
        let mut line = String::new();
        while reader.read_line(&mut line).is_ok_and(|read| read > 0) && line != "\r\n" {
            line.clear();
        }
        let head = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\
                    Transfer-Encoding: chunked\r\n\r\n";
        if stream.write_all(head.as_bytes()).is_err() {
            return;
        }
        let mut sent = 0;
        while sent < size {
            let mut chunk = vec![b' '; (size - sent).min(1 << 20)];
            if sent == 0 {
                chunk[..6].copy_from_slice(b"[\"en\"]");
            }
            let framed = stream
                .write_all(format!("{:x}\r\n", chunk.len()).as_bytes())
                .and_then(|()| stream.write_all(&chunk))
                .and_then(|()| stream.write_all(b"\r\n"));
            // The client stops reading at its limit and closes the connection.
            if framed.is_err() {
                return;
            }
            sent += chunk.len();
        }
        let _ = stream.write_all(b"0\r\n\r\n");
    });
    base
}

async fn credits_with_status(status: u16, body: &str) -> Error {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/scans/credits"))
        .respond_with(ResponseTemplate::new(status).set_body_string(body))
        .expect(1)
        .mount(&server)
        .await;
    client(&server)
        .credits_balance(&token())
        .await
        .expect_err("the call fails")
}

#[tokio::test]
async fn status_401_is_unauthorized_with_body() {
    let error = credits_with_status(401, "Authorization has been denied").await;
    match &error {
        Error::Unauthorized { body, endpoint } => {
            assert_eq!(body, "Authorization has been denied");
            assert_eq!(endpoint, "GET /v3/scans/credits");
        }
        other => panic!("expected Error::Unauthorized, got {other:?}"),
    }
    assert!(
        error
            .to_string()
            .ends_with(": Authorization has been denied"),
        "{error}"
    );
}

#[tokio::test]
async fn status_429_is_rate_limited_with_body() {
    let error = credits_with_status(429, "slow down").await;
    assert!(
        matches!(&error, Error::RateLimited { body, .. } if body == "slow down"),
        "{error:?}"
    );
}

#[tokio::test]
async fn status_503_is_under_maintenance_with_body() {
    let error = credits_with_status(503, "").await;
    assert!(
        matches!(&error, Error::UnderMaintenance { body, .. } if body.is_empty()),
        "{error:?}"
    );
    assert!(
        !error.to_string().ends_with(": "),
        "no empty body suffix: {error}"
    );
}

#[tokio::test]
async fn error_message_shows_at_most_512_bytes_of_the_body() {
    let body = "x".repeat(2000);
    let error = credits_with_status(400, &body).await;
    match &error {
        Error::Api { body: kept, .. } => assert_eq!(kept.len(), 2000, "the field keeps the body"),
        other => panic!("expected Error::Api, got {other:?}"),
    }
    let message = error.to_string();
    assert!(
        message.ends_with(&format!(": {}...", "x".repeat(512))),
        "{message}"
    );
}

#[tokio::test]
async fn error_body_is_kept_up_to_1_mib() {
    let error = credits_with_status(500, &"y".repeat((1 << 20) + 10)).await;
    match error {
        Error::Api { status, body, .. } => {
            assert_eq!(status, 500);
            assert_eq!(body.len(), 1 << 20);
        }
        other => panic!("expected Error::Api, got {other:?}"),
    }
}

#[tokio::test]
async fn json_body_over_16_mib_fails_without_truncating() {
    let server = MockServer::start().await;
    // A valid JSON document of 16 MiB + 1 byte: a string padded with spaces.
    let mut body = b"[\"en\"]".to_vec();
    body.resize((16 << 20) + 1, b' ');
    Mock::given(method("GET"))
        .and(path("/v3/miscellaneous/ocr-languages-list"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
        .expect(1)
        .mount(&server)
        .await;

    let error = client(&server)
        .ocr_supported_languages()
        .await
        .expect_err("too large");
    match &error {
        Error::ResponseTooLarge { endpoint, limit } => {
            assert_eq!(endpoint, "GET /v3/miscellaneous/ocr-languages-list");
            assert_eq!(*limit, 16 << 20);
        }
        other => panic!("expected Error::ResponseTooLarge, got {other:?}"),
    }
}

#[tokio::test]
async fn streamed_json_body_over_16_mib_fails_mid_read() {
    // No Content-Length: the limit is enforced while reading the chunks.
    let base = serve_streamed_json((16 << 20) + 1);
    let client = Client::builder()
        .api_base_url(base)
        .build()
        .expect("client");

    let error = client
        .ocr_supported_languages()
        .await
        .expect_err("too large");
    match &error {
        Error::ResponseTooLarge { endpoint, limit } => {
            assert_eq!(endpoint, "GET /v3/miscellaneous/ocr-languages-list");
            assert_eq!(*limit, 16 << 20);
        }
        other => panic!("expected Error::ResponseTooLarge, got {other:?}"),
    }
}

#[tokio::test]
async fn streamed_json_body_under_the_limit_is_read() {
    let base = serve_streamed_json((2 << 20) + 3);
    let client = Client::builder()
        .api_base_url(base)
        .build()
        .expect("client");

    let languages = client
        .ocr_supported_languages()
        .await
        .expect("2 MiB streamed body");
    assert_eq!(languages, ["en"]);
}

#[tokio::test]
async fn json_body_of_exactly_16_mib_is_read() {
    let server = MockServer::start().await;
    let mut body = b"[\"en\"]".to_vec();
    body.resize(16 << 20, b' ');
    Mock::given(method("GET"))
        .and(path("/v3/miscellaneous/ocr-languages-list"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
        .expect(1)
        .mount(&server)
        .await;

    let languages = client(&server)
        .ocr_supported_languages()
        .await
        .expect("16 MiB is accepted");
    assert_eq!(languages, ["en"]);
}

#[tokio::test]
async fn other_status_is_api_error_with_body_and_endpoint() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/v3/scans/submit/file/scan-1"))
        .respond_with(ResponseTemplate::new(409).set_body_string("scan id already exists"))
        .expect(1)
        .mount(&server)
        .await;

    let submission = FileSubmission::from_bytes(
        "text",
        "a.txt",
        ScanProperties::new("https://example.com/webhook/{STATUS}"),
    );
    let error = client(&server)
        .submit_file(&token(), "scan-1", &submission)
        .await
        .expect_err("409");

    match &error {
        Error::Api {
            status,
            body,
            endpoint,
        } => {
            assert_eq!(*status, 409);
            assert_eq!(body, "scan id already exists");
            assert_eq!(endpoint, "PUT /v3/scans/submit/file/scan-1");
        }
        other => panic!("expected Error::Api, got {other:?}"),
    }
    let message = error.to_string();
    assert!(message.contains("409"), "{message}");
    assert!(
        !message.contains(TOKEN),
        "the token must not be in the error"
    );
}

#[tokio::test]
async fn malformed_json_is_a_json_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/release-logs.json"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
        .expect(1)
        .mount(&server)
        .await;

    let error = client(&server).release_notes().await.expect_err("bad JSON");
    assert!(matches!(error, Error::Json(_)), "{error:?}");
}

#[tokio::test]
async fn wrong_json_shape_is_a_json_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/miscellaneous/ocr-languages-list"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "languages": [] })))
        .expect(1)
        .mount(&server)
        .await;

    let error = client(&server)
        .ocr_supported_languages()
        .await
        .expect_err("object instead of a list");
    assert!(matches!(error, Error::Json(_)), "{error:?}");
}

#[tokio::test]
async fn connection_failure_is_an_http_error() {
    // Nothing listens on port 1.
    let client = Client::builder()
        .api_base_url("http://127.0.0.1:1")
        .build()
        .expect("client");

    let error = client.release_notes().await.expect_err("no server");
    assert!(matches!(error, Error::Http(_)), "{error:?}");
}

#[tokio::test]
async fn expired_token_is_rejected_before_any_request() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "Amount": 1 })))
        .expect(0)
        .mount(&server)
        .await;

    let expired = AuthToken::new(TOKEN, "2020-01-01T00:00:00Z", "2020-01-03T00:00:00Z");
    let error = client(&server)
        .credits_balance(&expired)
        .await
        .expect_err("expired token");
    assert!(matches!(error, Error::TokenExpired), "{error:?}");
    assert!(!error.to_string().contains(TOKEN));
}

#[test]
fn token_verify() {
    token().verify().expect("future token");

    let past = AuthToken::new(TOKEN, "", "2020-01-01T00:00:00Z");
    assert!(matches!(past.verify(), Err(Error::TokenExpired)));

    // Expires in 2 minutes: inside the 5-minute safety margin.
    let soon = chrono_free_iso(120);
    let almost = AuthToken::new(TOKEN, "", soon);
    assert!(matches!(almost.verify(), Err(Error::TokenExpired)));

    // Expires in 10 minutes: usable.
    let later = AuthToken::new(TOKEN, "", chrono_free_iso(600));
    later.verify().expect("10 minutes left");

    // The login response format: 7 fractional digits and an offset.
    AuthToken::new(TOKEN, "", "2999-11-26T16:15:38.2431255+02:00")
        .verify()
        .expect("login response format");

    let garbage = AuthToken::new(TOKEN, "", "next tuesday");
    assert!(matches!(garbage.verify(), Err(Error::Validation(_))));

    // No expiry: treated as expired.
    let no_expiry = AuthToken::new(TOKEN, "", "");
    assert!(matches!(no_expiry.verify(), Err(Error::TokenExpired)));

    let empty = AuthToken::new("", "", "2999-01-01T00:00:00Z");
    assert!(matches!(empty.verify(), Err(Error::Validation(_))));
}

#[test]
fn token_debug_hides_the_access_token() {
    let debug = format!("{:?}", token());
    assert!(!debug.contains(TOKEN), "{debug}");
    assert!(debug.contains("2999-01-01T00:00:00Z"));
}

#[test]
fn token_round_trips_the_login_json() {
    let login = json!({
        "access_token": "abc",
        ".issued": "2018-11-24T16:15:38.2431255+02:00",
        ".expires": "2018-11-26T16:15:38.2431255+02:00"
    });
    let token: AuthToken = serde_json::from_value(login.clone()).expect("login JSON");
    assert_eq!(serde_json::to_value(token).expect("serialize"), login);
}

#[test]
fn base_url_must_be_https_or_loopback_http() {
    for url in [
        "https://api.copyleaks.com",
        "https://example.com/proxy/",
        "http://localhost:8080",
        "http://127.0.0.1:1",
        "http://[::1]:8080",
    ] {
        Client::builder()
            .api_base_url(url)
            .identity_base_url(url)
            .build()
            .unwrap_or_else(|error| panic!("{url} should be accepted: {error}"));
    }
}

#[test]
fn invalid_base_url_is_rejected() {
    for url in [
        "not a url",
        "ftp://example.com",
        "mailto:someone@example.com",
        "http://api.copyleaks.com",
        "http://10.0.0.1",
        "http://user:secret@example.com",
    ] {
        let error = Client::builder()
            .api_base_url(url)
            .build()
            .expect_err("invalid base URL");
        assert!(matches!(error, Error::Validation(_)), "{url}: {error:?}");
    }
}

/// An ISO 8601 UTC time `seconds` from now, without a date library.
fn chrono_free_iso(seconds: u64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock after 1970")
        .as_secs()
        + seconds;
    let days = i64::try_from(now / 86_400).expect("days fit");
    let rest = now % 86_400;
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    )
}

/// Converts days since 1970-01-01 to a (year, month, day) date (Howard Hinnant's algorithm).
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}
