//! The HTTP transport: gzip responses, HTTP/2, connection reuse after calls that return no
//! value, and large uploads.

mod common;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use base64::Engine as _;
use common::{client, gzip, only_request, token};
use copyleaks::plagiarism::{FileSubmission, ScanProperties};
use copyleaks::Client;
use serde_json::json;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn gzip_responses_are_requested_and_decoded() {
    let server = MockServer::start().await;
    let body = serde_json::to_vec(&json!({ "credits": 7 })).expect("JSON");
    Mock::given(method("GET"))
        .and(path("/v2/writer-detector/credits"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-encoding", "gzip")
                .set_body_raw(gzip(&body), "application/json"),
        )
        .expect(1)
        .mount(&server)
        .await;

    let credits = client(&server)
        .ai_text_detection_credits(&token())
        .await
        .expect("gzip body");
    assert_eq!(credits.credits, 7);
    let request = only_request(&server).await;
    let accept = request
        .headers
        .get("accept-encoding")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    assert!(accept.contains("gzip"), "accept-encoding: {accept:?}");
}

#[tokio::test]
async fn gzip_body_limits_apply_to_the_decoded_size() {
    // 17 MiB of spaces compress to a few KiB; the decoded body is over the 16 MiB limit.
    let server = MockServer::start().await;
    let mut body = b"[\"en\"]".to_vec();
    body.resize(17 << 20, b' ');
    Mock::given(method("GET"))
        .and(path("/v3/miscellaneous/ocr-languages-list"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-encoding", "gzip")
                .set_body_raw(gzip(&body), "application/json"),
        )
        .expect(1)
        .mount(&server)
        .await;

    let error = client(&server)
        .ocr_supported_languages()
        .await
        .expect_err("too large once decoded");
    assert!(
        matches!(error, copyleaks::Error::ResponseTooLarge { limit, .. } if limit == 16 << 20),
        "{error:?}"
    );
}

#[tokio::test]
async fn http2_is_available() {
    // The mock server speaks HTTP/2 without TLS when the client starts with it.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/miscellaneous/allowed-cross-languages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "documentLanguages": { "english": "en" }, "resultLanguages": { "french": "fr" }
        })))
        .expect(1)
        .mount(&server)
        .await;
    let http = copyleaks::reqwest::Client::builder()
        .http2_prior_knowledge()
        .build()
        .expect("HTTP/2 client");
    let client = common::builder(&server)
        .http_client(http)
        .build()
        .expect("client");

    let languages = client.allowed_cross_languages().await.expect("HTTP/2 call");
    assert_eq!(languages.result_codes(), ["fr"]);
}

/// Serves HTTP/1.1 on a local port, keeping connections open. Each response sends its headers,
/// waits 100 ms, then sends `body`, so a client that drops the response before reading it
/// cannot reuse the connection. Returns the base URL and the number of accepted connections.
fn serve_slow_bodies(status: &'static str, body: &'static str) -> (String, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let base = format!("http://{}", listener.local_addr().expect("local address"));
    let connections = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&connections);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            counter.fetch_add(1, Ordering::SeqCst);
            std::thread::spawn(move || {
                let mut reader = BufReader::new(stream.try_clone().expect("clone stream"));
                loop {
                    let mut length = 0;
                    let mut line = String::new();
                    loop {
                        line.clear();
                        match reader.read_line(&mut line) {
                            Ok(0) | Err(_) => return,
                            Ok(_) => {}
                        }
                        if line == "\r\n" {
                            break;
                        }
                        if let Some(value) =
                            line.to_ascii_lowercase().strip_prefix("content-length:")
                        {
                            length = value.trim().parse().unwrap_or(0);
                        }
                    }
                    let mut request_body = vec![0; length];
                    if reader.read_exact(&mut request_body).is_err() {
                        return;
                    }
                    let head = format!(
                        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\n\
                         Content-Length: {}\r\n\r\n",
                        body.len()
                    );
                    if stream.write_all(head.as_bytes()).is_err() {
                        return;
                    }
                    let _ = stream.flush();
                    std::thread::sleep(Duration::from_millis(100));
                    if stream.write_all(body.as_bytes()).is_err() {
                        return;
                    }
                }
            });
        }
    });
    (base, connections)
}

#[tokio::test]
async fn calls_without_a_result_read_the_body_so_the_connection_is_reused() {
    let (base, connections) = serve_slow_bodies("201 Created", r#"{"scannedDocument":{}}"#);
    let client = Client::builder()
        .api_base_url(base)
        .build()
        .expect("client");
    let submission = FileSubmission::from_bytes(
        "text",
        "a.txt",
        ScanProperties::new("https://example.com/webhook/{STATUS}"),
    );
    for scan_id in ["scan-1", "scan-2", "scan-3"] {
        client
            .submit_file(&token(), scan_id, &submission)
            .await
            .expect("submit");
    }
    client
        .resend_webhook(&token(), "scan-1")
        .await
        .expect("resend");
    assert_eq!(connections.load(Ordering::SeqCst), 1, "one connection");
}

#[tokio::test]
async fn large_upload_is_encoded_off_the_runtime_and_arrives_intact() {
    // 9 MiB is over the size at which the body is written on the blocking pool.
    let file: Vec<u8> = (0..9 << 20).map(|i: u32| (i % 251) as u8).collect();
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/v3/scans/submit/file/big-1"))
        .and(header("content-type", "application/json"))
        .respond_with(ResponseTemplate::new(201))
        .expect(1)
        .mount(&server)
        .await;

    let submission = FileSubmission::from_bytes(
        file.clone(),
        "big.bin",
        ScanProperties::new("https://example.com/webhook/{STATUS}"),
    );
    client(&server)
        .submit_file(&token(), "big-1", &submission)
        .await
        .expect("submit");

    let request = only_request(&server).await;
    let body: serde_json::Value = serde_json::from_slice(&request.body).expect("JSON body");
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(body["base64"].as_str().expect("base64"))
        .expect("valid base64");
    assert!(decoded == file, "the file arrives unchanged");
    assert_eq!(body["filename"], "big.bin");
    assert_eq!(
        body["properties"],
        json!({ "webhooks": { "status": "https://example.com/webhook/{STATUS}" } })
    );
}
