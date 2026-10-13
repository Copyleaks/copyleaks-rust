//! The `x-tenant` header: sent on every endpoint when a tenant is set on the
//! client, overridden per call with `with_tenant`, and absent when unset.

mod common;

use common::{builder, token, LIONS};
use copyleaks::ai_image::{AiImageDetectionRequest, AI_IMAGE_1_ULTRA};
use copyleaks::ai_text::AiTextDetectionRequest;
use copyleaks::grammar::GrammarCheckRequest;
use copyleaks::moderation::{labels, TextModerationRequest};
use copyleaks::plagiarism::{
    Delete, Export, FileSubmission, OcrSubmission, ScanProperties, Start, UrlSubmission,
};
use copyleaks::{Client, DEFAULT_USER_AGENT};
use serde_json::{json, Value};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Every endpoint: method, path, and a response body the SDK can decode.
fn endpoints() -> Vec<(&'static str, &'static str, ResponseTemplate)> {
    let ok = |body: Value| ResponseTemplate::new(200).set_body_json(body);
    vec![
        (
            "POST",
            "/v3/account/login/api",
            ok(json!({
                "access_token": "returned-token",
                ".issued": "2026-01-01T00:00:00Z",
                ".expires": "2999-01-01T00:00:00Z"
            })),
        ),
        (
            "PUT",
            "/v3/scans/submit/file/scan-1",
            ResponseTemplate::new(201),
        ),
        (
            "PUT",
            "/v3/scans/submit/ocr/scan-1",
            ResponseTemplate::new(201),
        ),
        (
            "PUT",
            "/v3/scans/submit/url/scan-1",
            ResponseTemplate::new(201),
        ),
        (
            "POST",
            "/v3/downloads/scan-1/export/export-1",
            ResponseTemplate::new(204),
        ),
        (
            "PATCH",
            "/v3/scans/start",
            ok(json!({ "success": [], "failed": [] })),
        ),
        ("PATCH", "/v3.1/scans/delete", ResponseTemplate::new(202)),
        (
            "POST",
            "/v3/scans/scan-1/webhooks/resend",
            ResponseTemplate::new(202),
        ),
        ("GET", "/v3/scans/credits", ok(json!({ "Amount": 1 }))),
        (
            "GET",
            "/v3/scans/usages/history",
            ResponseTemplate::new(200).set_body_raw("a,b\n", "text/csv"),
        ),
        ("GET", "/v3/release-logs.json", ok(json!({ "updates": [] }))),
        (
            "GET",
            "/v3/miscellaneous/supported-file-types",
            ok(json!({ "textual": [], "ocr": [] })),
        ),
        (
            "GET",
            "/v3/miscellaneous/ocr-languages-list",
            ok(json!(["en"])),
        ),
        ("POST", "/v2/writer-detector/scan-1/check", ok(json!({}))),
        ("POST", "/v1/ai-image-detector/scan-1/check", ok(json!({}))),
        ("POST", "/v1/writing-feedback/scan-1/check", ok(json!({}))),
        (
            "GET",
            "/v1/writing-feedback/correction-types/en",
            ok(json!({ "correctionTypes": [] })),
        ),
        ("POST", "/v1/text-moderation/scan-1/check", ok(json!({}))),
    ]
}

async fn mount_all(server: &MockServer) {
    for (verb, route, response) in endpoints() {
        Mock::given(method(verb))
            .and(path(route))
            .respond_with(response)
            .expect(1)
            .mount(server)
            .await;
    }
}

/// Calls every endpoint once.
async fn call_all(client: &Client) {
    let token = token();
    let properties = || ScanProperties::new("https://example.com/webhook/{STATUS}").sandbox(true);

    client
        .login("user@example.com", "my-api-key")
        .await
        .expect("login");
    client
        .submit_file(
            &token,
            "scan-1",
            &FileSubmission::from_bytes("text", "a.txt", properties()),
        )
        .await
        .expect("submit file");
    client
        .submit_file_ocr(
            &token,
            "scan-1",
            &OcrSubmission::new("aW1n", "a.png", "en", properties()),
        )
        .await
        .expect("submit OCR");
    client
        .submit_url(
            &token,
            "scan-1",
            &UrlSubmission::new("https://example.com/", properties()),
        )
        .await
        .expect("submit URL");
    client
        .export(
            &token,
            "scan-1",
            "export-1",
            &Export::new("https://example.com/done"),
        )
        .await
        .expect("export");
    client
        .start(&token, &Start::new(["scan-1"]))
        .await
        .expect("start");
    client
        .delete(&token, &Delete::new(["scan-1"]))
        .await
        .expect("delete");
    client
        .resend_webhook(&token, "scan-1")
        .await
        .expect("resend webhook");
    client
        .credits_balance(&token)
        .await
        .expect("credits balance");
    client
        .usages_history_csv(&token, "01-01-2020", "31-01-2020")
        .await
        .expect("usages history");
    client.release_notes().await.expect("release notes");
    client
        .supported_file_types()
        .await
        .expect("supported file types");
    client
        .ocr_supported_languages()
        .await
        .expect("OCR languages");
    client
        .ai_text_detection(&token, "scan-1", &AiTextDetectionRequest::new(LIONS))
        .await
        .expect("AI Text Detection");
    client
        .ai_image_detection(
            &token,
            "scan-1",
            &AiImageDetectionRequest::new("aW1n", "a.png", AI_IMAGE_1_ULTRA),
        )
        .await
        .expect("AI Image Detection");
    client
        .grammar_check(&token, "scan-1", &GrammarCheckRequest::new("Some text."))
        .await
        .expect("Grammar Checker");
    client
        .grammar_correction_types("en")
        .await
        .expect("correction types");
    client
        .text_moderation(
            &token,
            "scan-1",
            &TextModerationRequest::new("Some text.", labels::ALL),
        )
        .await
        .expect("Text Moderation");
}

/// Asserts that every endpoint was called once, with this tenant header (or none) and the SDK
/// user agent.
async fn assert_tenant(server: &MockServer, expected: Option<&str>) {
    let requests = server.received_requests().await.expect("request recording");
    assert_eq!(
        requests.len(),
        endpoints().len(),
        "one request per endpoint"
    );
    for request in &requests {
        let tenant = request
            .headers
            .get("x-tenant")
            .map(|value| value.to_str().expect("ASCII header"));
        assert_eq!(tenant, expected, "x-tenant on {}", request.url.path());
        let user_agent = request
            .headers
            .get("user-agent")
            .map(|value| value.to_str().expect("ASCII header"));
        assert_eq!(
            user_agent,
            Some(DEFAULT_USER_AGENT),
            "user agent on {}",
            request.url.path()
        );
    }
}

#[tokio::test]
async fn client_tenant_is_sent_on_every_endpoint() {
    let server = MockServer::start().await;
    mount_all(&server).await;
    let client = builder(&server).tenant("t1").build().expect("client");

    call_all(&client).await;

    assert_tenant(&server, Some("t1")).await;
}

#[tokio::test]
async fn with_tenant_overrides_the_client_tenant_on_every_endpoint() {
    let server = MockServer::start().await;
    mount_all(&server).await;
    let client = builder(&server).tenant("t1").build().expect("client");

    let per_call = client.with_tenant("t2");
    call_all(&per_call).await;

    assert_tenant(&server, Some("t2")).await;
    assert_eq!(per_call.tenant(), Some("t2"));
    assert_eq!(
        client.tenant(),
        Some("t1"),
        "with_tenant does not change the original client"
    );
}

#[tokio::test]
async fn no_tenant_header_when_unset() {
    let server = MockServer::start().await;
    mount_all(&server).await;
    let client = builder(&server).build().expect("client");

    call_all(&client).await;

    assert_tenant(&server, None).await;
    assert_eq!(client.tenant(), None);
}

#[tokio::test]
async fn empty_tenant_sends_no_header() {
    let server = MockServer::start().await;
    mount_all(&server).await;
    let client = builder(&server).tenant("t1").build().expect("client");

    call_all(&client.with_tenant("")).await;

    assert_tenant(&server, None).await;
}

#[tokio::test]
async fn app_user_agent_goes_before_the_sdk_token() {
    let server = MockServer::start().await;
    let expected = format!("my-app/1.0 {DEFAULT_USER_AGENT}");
    Mock::given(method("GET"))
        .and(path("/v3/release-logs.json"))
        .and(wiremock::matchers::header("user-agent", expected.as_str()))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "updates": [] })))
        .expect(1)
        .mount(&server)
        .await;
    let client = builder(&server)
        .user_agent(" my-app/1.0 ")
        .build()
        .expect("client");

    client.release_notes().await.expect("release notes");
}

#[tokio::test]
async fn custom_http_client_is_used() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3/release-logs.json"))
        .and(wiremock::matchers::header("user-agent", DEFAULT_USER_AGENT))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "updates": [] })))
        .expect(1)
        .mount(&server)
        .await;
    let http = copyleaks::reqwest::Client::builder()
        .user_agent("ignored-default-agent")
        .build()
        .expect("reqwest client");
    let client = builder(&server).http_client(http).build().expect("client");

    client.release_notes().await.expect("release notes");
}
