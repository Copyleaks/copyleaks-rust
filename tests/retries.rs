//! Opt-in retries: which calls are retried, on which failures, and how long the client waits.

mod common;

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::time::{Duration, Instant};

use common::{builder, client, token, LIONS};
use copyleaks::ai_text::AiTextDetectionRequest;
use copyleaks::image_plagiarism::ImagePlagiarismRequest;
use copyleaks::plagiarism::{FileSubmission, ScanProperties, Start};
use copyleaks::{Client, Error, RetryPolicy};
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// A policy with short waits, for tests.
fn fast_policy(retries: u32) -> RetryPolicy {
    RetryPolicy::new(retries)
        .initial_backoff(Duration::from_millis(10))
        .max_backoff(Duration::from_millis(50))
}

fn retrying_client(server: &MockServer, policy: RetryPolicy) -> Client {
    builder(server)
        .retry_policy(policy)
        .build()
        .expect("client")
}

/// Answers `times` requests to `url_path` with `template`, before any later mock.
async fn fail_first(server: &MockServer, url_path: &str, times: u64, template: ResponseTemplate) {
    Mock::given(path(url_path))
        .respond_with(template)
        .up_to_n_times(times)
        .expect(times)
        .with_priority(1)
        .mount(server)
        .await;
}

async fn request_count(server: &MockServer) -> usize {
    server
        .received_requests()
        .await
        .expect("recording is on")
        .len()
}

#[tokio::test]
async fn get_is_retried_after_429_honoring_retry_after_seconds() {
    let server = MockServer::start().await;
    fail_first(
        &server,
        "/v3/scans/credits",
        1,
        ResponseTemplate::new(429)
            .insert_header("retry-after", "1")
            .set_body_string("slow down"),
    )
    .await;
    Mock::given(method("GET"))
        .and(path("/v3/scans/credits"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "Amount": 5 })))
        .expect(1)
        .mount(&server)
        .await;

    // The backoff alone would wait 10 ms; Retry-After asks for 1 second.
    let started = Instant::now();
    let balance = retrying_client(&server, fast_policy(3))
        .credits_balance(&token())
        .await
        .expect("retried");
    assert_eq!(balance.amount, 5);
    assert!(
        started.elapsed() >= Duration::from_millis(950),
        "waited {:?}",
        started.elapsed()
    );
    assert_eq!(request_count(&server).await, 2);
}

#[tokio::test]
async fn get_is_retried_after_503_with_backoff() {
    let server = MockServer::start().await;
    fail_first(
        &server,
        "/v3/miscellaneous/allowed-cross-languages",
        2,
        ResponseTemplate::new(503),
    )
    .await;
    Mock::given(method("GET"))
        .and(path("/v3/miscellaneous/allowed-cross-languages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "documentLanguages": ["en"], "resultLanguages": ["fr"]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let languages = retrying_client(&server, fast_policy(2))
        .allowed_cross_languages()
        .await
        .expect("retried twice");
    assert_eq!(languages.document_languages, ["en"]);
    assert_eq!(request_count(&server).await, 3);
}

#[tokio::test]
async fn retry_after_http_date_in_the_past_retries_at_once() {
    let server = MockServer::start().await;
    fail_first(
        &server,
        "/v2/writer-detector/credits",
        1,
        ResponseTemplate::new(503).insert_header("retry-after", "Sun, 06 Nov 1994 08:49:37 GMT"),
    )
    .await;
    Mock::given(method("GET"))
        .and(path("/v2/writer-detector/credits"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "credits": 1 })))
        .expect(1)
        .mount(&server)
        .await;

    // A large backoff that a past date must override.
    let policy = RetryPolicy::new(1).initial_backoff(Duration::from_secs(30));
    let started = Instant::now();
    retrying_client(&server, policy)
        .ai_text_detection_credits(&token())
        .await
        .expect("retried");
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[tokio::test]
async fn put_submit_is_retried_with_the_same_body() {
    let server = MockServer::start().await;
    fail_first(
        &server,
        "/v3/scans/submit/file/scan-1",
        1,
        ResponseTemplate::new(503),
    )
    .await;
    Mock::given(method("PUT"))
        .and(path("/v3/scans/submit/file/scan-1"))
        .respond_with(ResponseTemplate::new(201))
        .expect(1)
        .mount(&server)
        .await;

    let submission = FileSubmission::from_bytes(
        "Hello world",
        "hello.txt",
        ScanProperties::new("https://example.com/webhook/{STATUS}"),
    );
    retrying_client(&server, fast_policy(3))
        .submit_file(&token(), "scan-1", &submission)
        .await
        .expect("retried");

    let requests = server.received_requests().await.expect("recording is on");
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].body, requests[1].body, "the same body twice");
    assert_eq!(
        requests[1].body_json::<serde_json::Value>().expect("JSON"),
        json!({
            "base64": "SGVsbG8gd29ybGQ=",
            "filename": "hello.txt",
            "properties": { "webhooks": { "status": "https://example.com/webhook/{STATUS}" } }
        })
    );
}

#[tokio::test]
async fn post_and_patch_are_never_retried() {
    let server = MockServer::start().await;
    Mock::given(path("/v2/writer-detector/scan-1/check"))
        .respond_with(ResponseTemplate::new(429).insert_header("retry-after", "0"))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path("/v3/scans/start"))
        .respond_with(ResponseTemplate::new(503))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path("/v1/image-plagiarism-detector/scan-1/check"))
        .respond_with(ResponseTemplate::new(503))
        .expect(1)
        .mount(&server)
        .await;
    let client = retrying_client(&server, fast_policy(5));

    let error = client
        .ai_text_detection(&token(), "scan-1", &AiTextDetectionRequest::new(LIONS))
        .await
        .expect_err("not retried");
    assert!(matches!(error, Error::RateLimited { .. }), "{error:?}");
    assert_eq!(error.retry_after(), Some(Duration::ZERO));

    let error = client
        .start(&token(), &Start::new(["scan-1"]))
        .await
        .expect_err("not retried");
    assert!(matches!(error, Error::UnderMaintenance { .. }), "{error:?}");

    let request = ImagePlagiarismRequest::new(b"image".to_vec(), "photo.png");
    let error = client
        .image_plagiarism_detection(&token(), "scan-1", &request)
        .await
        .expect_err("not retried");
    assert_eq!(error.status(), Some(503));
    assert_eq!(request_count(&server).await, 3, "one request per call");
}

#[tokio::test]
async fn retry_after_over_the_maximum_fails_at_once() {
    let server = MockServer::start().await;
    Mock::given(path("/v3/scans/credits"))
        .respond_with(ResponseTemplate::new(429).insert_header("retry-after", "120"))
        .expect(1)
        .mount(&server)
        .await;

    let error = retrying_client(&server, fast_policy(3))
        .credits_balance(&token())
        .await
        .expect_err("too long to wait");
    match &error {
        Error::RateLimited { retry_after, .. } => {
            assert_eq!(*retry_after, Some(Duration::from_secs(120)));
        }
        other => panic!("expected Error::RateLimited, got {other:?}"),
    }
}

#[tokio::test]
async fn retries_stop_after_the_maximum_with_the_last_error() {
    let server = MockServer::start().await;
    Mock::given(path("/v3/scans/credits"))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("retry-after", "0")
                .set_body_string("still busy"),
        )
        .expect(3)
        .mount(&server)
        .await;

    let error = retrying_client(&server, fast_policy(2))
        .credits_balance(&token())
        .await
        .expect_err("gives up");
    assert!(
        matches!(&error, Error::RateLimited { body, retry_after: Some(wait), .. }
            if body == "still busy" && wait.is_zero()),
        "{error:?}"
    );
}

#[tokio::test]
async fn other_statuses_are_not_retried() {
    let server = MockServer::start().await;
    Mock::given(path("/v3/scans/credits"))
        .respond_with(ResponseTemplate::new(500))
        .expect(1)
        .mount(&server)
        .await;
    let error = retrying_client(&server, fast_policy(3))
        .credits_balance(&token())
        .await
        .expect_err("500");
    assert_eq!(error.status(), Some(500));
}

#[tokio::test]
async fn without_a_policy_nothing_is_retried() {
    let server = MockServer::start().await;
    Mock::given(path("/v3/scans/credits"))
        .respond_with(ResponseTemplate::new(503).insert_header("retry-after", "0"))
        .expect(1)
        .mount(&server)
        .await;
    let error = client(&server)
        .credits_balance(&token())
        .await
        .expect_err("not retried");
    assert!(matches!(error, Error::UnderMaintenance { .. }), "{error:?}");
}

/// A port with nothing listening on it yet.
fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind")
        .local_addr()
        .expect("address")
        .port()
}

/// Starts listening on `port` after `delay`, and answers one request with `{"credits": 9}`.
fn serve_later(port: u16, delay: Duration) {
    std::thread::spawn(move || {
        std::thread::sleep(delay);
        let listener = TcpListener::bind(("127.0.0.1", port)).expect("bind the port again");
        let Ok((mut stream, _)) = listener.accept() else {
            return;
        };
        let mut reader = BufReader::new(stream.try_clone().expect("clone stream"));
        let mut line = String::new();
        while reader.read_line(&mut line).is_ok_and(|read| read > 0) && line != "\r\n" {
            line.clear();
        }
        let body = r#"{"credits": 9}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\
             Connection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = stream.write_all(response.as_bytes());
    });
}

#[tokio::test]
async fn connection_errors_are_retried() {
    let port = free_port();
    serve_later(port, Duration::from_millis(200));
    let client = Client::builder()
        .api_base_url(format!("http://127.0.0.1:{port}"))
        .retry_policy(
            RetryPolicy::new(20)
                .initial_backoff(Duration::from_millis(100))
                .max_backoff(Duration::from_millis(100)),
        )
        .build()
        .expect("client");

    let credits = client
        .ai_text_detection_credits(&token())
        .await
        .expect("connected on a later attempt");
    assert_eq!(credits.credits, 9);
}

#[tokio::test]
async fn connection_errors_of_unsafe_calls_are_not_retried() {
    let port = free_port();
    let client = Client::builder()
        .api_base_url(format!("http://127.0.0.1:{port}"))
        .retry_policy(RetryPolicy::new(3).initial_backoff(Duration::from_secs(30)))
        .build()
        .expect("client");

    let started = Instant::now();
    let error = client
        .ai_text_detection(&token(), "scan-1", &AiTextDetectionRequest::new(LIONS))
        .await
        .expect_err("nothing listens");
    assert!(matches!(error, Error::Http(_)), "{error:?}");
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "no backoff wait"
    );
}
