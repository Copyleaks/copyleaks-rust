//! Client-side checks: a request that the server would reject fails with `Error::Validation`
//! and is never sent.

mod common;

use common::{client, token, LIONS};
use copyleaks::ai_image::{AiImageDetectionRequest, AI_IMAGE_1_ULTRA};
use copyleaks::ai_text::AiTextDetectionRequest;
use copyleaks::grammar::{GrammarCheckRequest, ScoreWeights};
use copyleaks::moderation::{TextModerationLabel, TextModerationRequest};
use copyleaks::plagiarism::{
    Delete, DeleteScan, Export, ExportResult, ExportTarget, FileSubmission, ScanProperties,
    Scanning, ScanningExclude, Start, UrlSubmission,
};
use copyleaks::{Client, Error, Result};
use wiremock::matchers::any;
use wiremock::{Mock, MockServer, ResponseTemplate};

/// A mock server that fails the test if it receives any request.
async fn silent_server() -> MockServer {
    let server = MockServer::start().await;
    Mock::given(any())
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&server)
        .await;
    server
}

fn assert_validation<T: std::fmt::Debug>(result: Result<T>, what: &str) {
    match result {
        Err(Error::Validation(message)) => assert!(!message.is_empty(), "{what}"),
        other => panic!("{what}: expected Error::Validation, got {other:?}"),
    }
}

fn properties() -> ScanProperties {
    ScanProperties::new("https://example.com/webhook/{STATUS}")
}

fn file() -> FileSubmission {
    FileSubmission::from_bytes("text", "a.txt", properties())
}

/// A mock server that answers every request with `{}` and expects `calls` requests.
async fn accepting_server(calls: u64) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(any())
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .expect(calls)
        .mount(&server)
        .await;
    server
}

#[tokio::test]
async fn plagiarism_submits_check_length_charset_and_reserved_word() {
    let server = silent_server().await;
    let client = client(&server);
    let too_long = "a".repeat(37);
    for id in [
        "ab",
        too_long.as_str(),
        "Scan-1",
        "scan 1",
        "scan/1",
        "scan#1",
        "scän",
        "scan^1",
    ] {
        assert_validation(client.submit_file(&token(), id, &file()).await, id);
    }
}

#[tokio::test]
async fn product_endpoints_check_only_the_scan_id_length() {
    let server = silent_server().await;
    let client = client(&server);
    let too_long = "a".repeat(37);
    // 19 emoji are 38 UTF-16 code units.
    let emoji = "😀".repeat(19);
    for id in ["ab", too_long.as_str(), emoji.as_str()] {
        let request = AiTextDetectionRequest::new(LIONS);
        assert_validation(client.ai_text_detection(&token(), id, &request).await, id);
        let request = GrammarCheckRequest::new("Some text.");
        assert_validation(client.grammar_check(&token(), id, &request).await, id);
    }
}

#[tokio::test]
async fn product_endpoints_accept_any_characters() {
    // The server only logs a charset mismatch on the product endpoints.
    let ids = ["Scan-1", "scan 1", "scän", "google-1"];
    let server = accepting_server(2 * ids.len() as u64).await;
    let client = client(&server);
    for id in ids {
        client
            .ai_text_detection(&token(), id, &AiTextDetectionRequest::new(LIONS))
            .await
            .unwrap_or_else(|error| panic!("{id}: {error}"));
        let request = TextModerationRequest::new("Some text.", ["toxic-v1"]);
        client
            .text_moderation(&token(), id, &request)
            .await
            .unwrap_or_else(|error| panic!("{id}: {error}"));
    }
}

#[tokio::test]
async fn reserved_word_is_rejected_for_plagiarism_submits_only() {
    let server = silent_server().await;
    let client = client(&server);
    for id in ["google", "g00gle", "gogle", "my-gooogle-scan"] {
        assert_validation(client.submit_file(&token(), id, &file()).await, id);
        let url = UrlSubmission::new("https://example.com/", properties());
        assert_validation(client.submit_url(&token(), id, &url).await, id);
    }
}

#[tokio::test]
async fn lengths_are_counted_in_utf16_code_units() {
    // 127 emoji (254 UTF-16 code units, 127 chars) and one letter: 255 units.
    let text = format!("{}a", "😀".repeat(127));
    let server = accepting_server(1).await;
    client(&server)
        .ai_text_detection(&token(), "scan-1", &AiTextDetectionRequest::new(text))
        .await
        .expect("255 UTF-16 code units are enough");
}

fn with_id_pattern(pattern: &str) -> FileSubmission {
    let exclude = ScanningExclude::default().id_pattern(pattern);
    let properties = properties().scanning(Scanning::default().exclude(exclude));
    FileSubmission::from_bytes("text", "a.txt", properties)
}

#[tokio::test]
async fn exclude_id_pattern_is_1_to_36_characters() {
    let silent = silent_server().await;
    let rejecting = client(&silent);
    for pattern in [
        String::new(),
        "a".repeat(37),
        format!("{}a", "😀".repeat(18)),
    ] {
        assert_validation(
            rejecting
                .submit_file(&token(), "scan-1", &with_id_pattern(&pattern))
                .await,
            "idPattern length",
        );
    }

    // 36 UTF-16 code units are accepted.
    let server = accepting_server(1).await;
    client(&server)
        .submit_file(&token(), "scan-1", &with_id_pattern(&"😀".repeat(18)))
        .await
        .expect("a 36-unit idPattern");
}

#[tokio::test]
async fn ai_text_sensitivity_is_1_to_3() {
    let server = silent_server().await;
    let client = client(&server);
    for sensitivity in [0_u8, 4] {
        let request = AiTextDetectionRequest::new(LIONS).sensitivity(sensitivity);
        assert_validation(
            client.ai_text_detection(&token(), "scan-1", &request).await,
            "sensitivity",
        );
    }
}

#[tokio::test]
async fn ai_text_needs_at_least_255_characters() {
    let server = silent_server().await;
    let short = "a".repeat(254);
    assert_validation(
        client(&server)
            .ai_text_detection(&token(), "scan-1", &AiTextDetectionRequest::new(short))
            .await,
        "254 characters",
    );
}

#[tokio::test]
async fn plagiarism_submission_checks() {
    let server = silent_server().await;
    let client = client(&server);

    let no_webhook = FileSubmission::from_bytes("text", "a.txt", ScanProperties::default());
    assert_validation(
        client.submit_file(&token(), "scan-1", &no_webhook).await,
        "missing status webhook",
    );

    let no_content = FileSubmission::new("", "a.txt", properties());
    assert_validation(
        client.submit_file(&token(), "scan-1", &no_content).await,
        "empty base64",
    );

    let long_payload = FileSubmission::from_bytes(
        "text",
        "a.txt",
        properties().developer_payload("x".repeat(513)),
    );
    assert_validation(
        client.submit_file(&token(), "scan-1", &long_payload).await,
        "developer payload over 512 characters",
    );

    let sensitivity =
        FileSubmission::from_bytes("text", "a.txt", properties().sensitivity_level(6));
    assert_validation(
        client.submit_file(&token(), "scan-1", &sensitivity).await,
        "sensitivity level 6",
    );

    let expiration = FileSubmission::from_bytes("text", "a.txt", properties().expiration(2881));
    assert_validation(
        client.submit_file(&token(), "scan-1", &expiration).await,
        "expiration over 2880 hours",
    );
}

#[tokio::test]
async fn scan_management_checks() {
    let server = silent_server().await;
    let client = client(&server);

    let no_scans: [&str; 0] = [];
    assert_validation(
        client.start(&token(), &Start::new(no_scans)).await,
        "empty trigger",
    );
    let too_many: Vec<String> = (0..101).map(|i| format!("scan-{i}")).collect();
    assert_validation(
        client.start(&token(), &Start::new(too_many)).await,
        "101 scans",
    );
    assert_validation(
        client.delete(&token(), &Delete::new(no_scans)).await,
        "no scans",
    );
    let mut empty_id = Delete::new(["scan-1"]);
    empty_id.scans.push(DeleteScan::new(""));
    assert_validation(client.delete(&token(), &empty_id).await, "empty delete id");

    let export = Export::new("https://example.com/done");
    assert_validation(
        client.export(&token(), "ab", "export-1", &export).await,
        "short scan id",
    );
    assert_validation(
        client.export(&token(), "scan-1", "ex", &export).await,
        "short export id",
    );
    assert_validation(
        client
            .export(
                &token(),
                "scan-1",
                "export-1",
                &export.clone().max_retries(13),
            )
            .await,
        "13 retries",
    );
    assert_validation(
        client
            .export(&token(), "scan-1", "export-1", &Export::default())
            .await,
        "missing completion webhook",
    );
    for (scan_id, export_id) in [
        ("Scan-1", "export-1"),
        ("scan-1", "my-google"),
        ("scan 1", "export-1"),
    ] {
        assert_validation(
            client.export(&token(), scan_id, export_id, &export).await,
            "export id charset or reserved word",
        );
    }
    let no_verb = export.clone().results(vec![ExportResult::new(
        "result-1",
        "https://example.com/r",
        "",
    )]);
    assert_validation(
        client
            .export(&token(), "scan-1", "export-1", &no_verb)
            .await,
        "result without verb",
    );
    let no_endpoint = export.clone().pdf_report(ExportTarget::new("", "POST"));
    assert_validation(
        client
            .export(&token(), "scan-1", "export-1", &no_endpoint)
            .await,
        "PDF target without endpoint",
    );
    let no_crawled_verb = export
        .clone()
        .crawled_version(ExportTarget::new("https://example.com/c", ""));
    assert_validation(
        client
            .export(&token(), "scan-1", "export-1", &no_crawled_verb)
            .await,
        "crawled version target without verb",
    );

    assert_validation(
        client
            .usages_history_csv(&token(), "2020-01-01", "31-01-2020")
            .await,
        "start date format",
    );
    assert_validation(client.resend_webhook(&token(), "").await, "empty scan id");
    assert_validation(client.resend_webhook(&token(), "ab").await, "short scan id");
    assert_validation(
        client.resend_webhook(&token(), &"a".repeat(37)).await,
        "long scan id",
    );
}

#[tokio::test]
async fn product_request_checks() {
    let server = silent_server().await;
    let client = client(&server);

    for filename in ["image.gif", "image", "a.pn"] {
        let request = AiImageDetectionRequest::new("aW1n", filename, AI_IMAGE_1_ULTRA);
        assert_validation(
            client
                .ai_image_detection(&token(), "scan-1", &request)
                .await,
            filename,
        );
    }
    let no_model = AiImageDetectionRequest::new("aW1n", "image.png", "");
    assert_validation(
        client
            .ai_image_detection(&token(), "scan-1", &no_model)
            .await,
        "empty model",
    );

    assert_validation(
        client
            .grammar_check(&token(), "scan-1", &GrammarCheckRequest::new(""))
            .await,
        "empty grammar text",
    );
    let heavy =
        GrammarCheckRequest::new("text").score(ScoreWeights::default().grammar_score_weight(1.5));
    assert_validation(
        client.grammar_check(&token(), "scan-1", &heavy).await,
        "weight over 1.0",
    );

    let no_labels = TextModerationRequest::new("Some text.", Vec::<String>::new());
    assert_validation(
        client.text_moderation(&token(), "scan-1", &no_labels).await,
        "no labels",
    );
    let seventeen = TextModerationRequest::new("Some text.", (0..17).map(|i| format!("label-{i}")));
    assert_validation(
        client.text_moderation(&token(), "scan-1", &seventeen).await,
        "17 labels",
    );
    let mut long_label = TextModerationRequest::new("Some text.", ["toxic-v1"]);
    long_label
        .labels
        .push(TextModerationLabel::new("x".repeat(33)));
    assert_validation(
        client
            .text_moderation(&token(), "scan-1", &long_label)
            .await,
        "label id over 32 characters",
    );
    let one_char = TextModerationRequest::new("a", ["toxic-v1"]);
    assert_validation(
        client.text_moderation(&token(), "scan-1", &one_char).await,
        "1-character text",
    );

    assert_validation(client.login("", "key").await, "empty email");
    assert_validation(client.login("user@example.com", "").await, "empty key");
    assert_validation(client.grammar_correction_types("").await, "empty language");
}

#[tokio::test]
async fn image_extension_check_ignores_case() {
    // Nothing listens on port 1: an upper-case extension passes the checks and fails only when
    // the request is sent.
    let client = Client::builder()
        .api_base_url("http://127.0.0.1:1")
        .build()
        .expect("client");
    let request = AiImageDetectionRequest::new("aW1n", "IMAGE.PNG", AI_IMAGE_1_ULTRA);
    let error = client
        .ai_image_detection(&token(), "scan-1", &request)
        .await
        .expect_err("no server");
    assert!(matches!(error, Error::Http(_)), "{error:?}");
}

#[tokio::test]
async fn image_extension_check_accepts_every_server_type() {
    // Nothing listens on port 1: an accepted extension passes the checks and fails only when the
    // request is sent.
    let client = Client::builder()
        .api_base_url("http://127.0.0.1:1")
        .build()
        .expect("client");
    for filename in ["photo.heif", "photo.avif", "photo.webp"] {
        let request = AiImageDetectionRequest::new("aW1n", filename, AI_IMAGE_1_ULTRA);
        let error = client
            .ai_image_detection(&token(), "scan-1", &request)
            .await
            .expect_err("no server");
        assert!(matches!(error, Error::Http(_)), "{filename}: {error:?}");
    }
}
