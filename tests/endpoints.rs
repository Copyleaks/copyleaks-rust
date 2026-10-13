//! One test per endpoint, against a local mock server. Each test checks the HTTP method, the
//! path, the headers (Authorization, User-Agent, no x-tenant by default), the exact JSON body,
//! and the decoded response.

mod common;

use common::{client, fixture, token, NoHeader, BEARER, LIONS};
use copyleaks::ai_image::{AiImageDetectionRequest, AI_IMAGE_1_ULTRA};
use copyleaks::ai_text::AiTextDetectionRequest;
use copyleaks::grammar::{GrammarCheckRequest, ScoreWeights};
use copyleaks::moderation::{labels, TextModerationRequest, LANGUAGE_ENGLISH};
use copyleaks::plagiarism::{
    Delete, Export, ExportResult, ExportTarget, FileSubmission, OcrSubmission, ScanProperties,
    Start, StartErrorHandling, UrlSubmission,
};
use copyleaks::DEFAULT_USER_AGENT;
use serde_json::json;
use wiremock::matchers::{body_json, header, method, path, query_param};
use wiremock::{Mock, MockBuilder, MockServer, ResponseTemplate};

const WEBHOOK: &str = "https://example.com/webhook/{STATUS}";

/// Matches the method, the path, the SDK user agent, and the absence of the tenant header.
fn route(verb: &str, url_path: &str) -> MockBuilder {
    Mock::given(method(verb))
        .and(path(url_path))
        .and(header("user-agent", DEFAULT_USER_AGENT))
        .and(NoHeader("x-tenant"))
}

/// [`route`] for an authenticated JSON call.
fn authed(verb: &str, url_path: &str) -> MockBuilder {
    route(verb, url_path).and(header("authorization", BEARER))
}

/// [`route`] for a public call, which must not send a token.
fn public(verb: &str, url_path: &str) -> MockBuilder {
    route(verb, url_path).and(NoHeader("authorization"))
}

fn sandbox_properties() -> ScanProperties {
    ScanProperties::new(WEBHOOK).sandbox(true)
}

fn sandbox_properties_json() -> serde_json::Value {
    json!({ "webhooks": { "status": WEBHOOK }, "sandbox": true })
}

#[tokio::test]
async fn login() {
    let server = MockServer::start().await;
    public("POST", "/v3/account/login/api")
        .and(header("content-type", "application/json"))
        .and(body_json(
            json!({ "email": "user@example.com", "key": "my-api-key" }),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "returned-token",
            ".issued": "2018-11-24T16:15:38.2431255+02:00",
            ".expires": "2999-11-26T16:15:38.2431255+02:00"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let token = client(&server)
        .login("user@example.com", "my-api-key")
        .await
        .expect("login");

    assert_eq!(token.access_token, "returned-token");
    assert_eq!(token.issued, "2018-11-24T16:15:38.2431255+02:00");
    assert_eq!(token.expires, "2999-11-26T16:15:38.2431255+02:00");
    token.verify().expect("the returned token is valid");
}

#[tokio::test]
async fn submit_file() {
    let server = MockServer::start().await;
    authed("PUT", "/v3/scans/submit/file/scan-file-1")
        .and(header("content-type", "application/json"))
        .and(body_json(json!({
            "base64": "SGVsbG8gd29ybGQ=",
            "filename": "hello.txt",
            "properties": sandbox_properties_json()
        })))
        .respond_with(ResponseTemplate::new(201))
        .expect(1)
        .mount(&server)
        .await;

    let submission = FileSubmission::from_bytes("Hello world", "hello.txt", sandbox_properties());
    client(&server)
        .submit_file(&token(), "scan-file-1", &submission)
        .await
        .expect("submit file");
}

#[tokio::test]
async fn submit_file_ocr() {
    let server = MockServer::start().await;
    authed("PUT", "/v3/scans/submit/ocr/scan-ocr-1")
        .and(body_json(json!({
            "base64": "aW1hZ2U=",
            "filename": "page.png",
            "langCode": "en",
            "properties": sandbox_properties_json()
        })))
        .respond_with(ResponseTemplate::new(201))
        .expect(1)
        .mount(&server)
        .await;

    let submission = OcrSubmission::new("aW1hZ2U=", "page.png", "en", sandbox_properties());
    client(&server)
        .submit_file_ocr(&token(), "scan-ocr-1", &submission)
        .await
        .expect("submit OCR");
}

#[tokio::test]
async fn submit_url() {
    let server = MockServer::start().await;
    authed("PUT", "/v3/scans/submit/url/scan-url-1")
        .and(body_json(json!({
            "url": "https://example.com/",
            "properties": sandbox_properties_json()
        })))
        .respond_with(ResponseTemplate::new(201))
        .expect(1)
        .mount(&server)
        .await;

    let submission = UrlSubmission::new("https://example.com/", sandbox_properties());
    client(&server)
        .submit_url(&token(), "scan-url-1", &submission)
        .await
        .expect("submit URL");
}

#[tokio::test]
async fn export() {
    let server = MockServer::start().await;
    authed("POST", "/v3/downloads/scan-1/export/export-1")
        .and(body_json(json!({
            "completionWebhook": "https://example.com/export/export-1/completed",
            "maxRetries": 3,
            "developerPayload": "payload",
            "results": [{
                "id": "result-1",
                "endpoint": "https://example.com/export/export-1/results/result-1",
                "verb": "POST",
                "headers": [["key", "value"]]
            }],
            "pdfReport": { "endpoint": "https://example.com/export/export-1/pdf", "verb": "POST" },
            "crawledVersion": {
                "endpoint": "https://example.com/export/export-1/crawled",
                "verb": "PUT"
            }
        })))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let export = Export::new("https://example.com/export/export-1/completed")
        .max_retries(3)
        .developer_payload("payload")
        .results(vec![ExportResult::new(
            "result-1",
            "https://example.com/export/export-1/results/result-1",
            "POST",
        )
        .headers(vec![("key".to_owned(), "value".to_owned())])])
        .pdf_report(ExportTarget::new(
            "https://example.com/export/export-1/pdf",
            "POST",
        ))
        .crawled_version(ExportTarget::new(
            "https://example.com/export/export-1/crawled",
            "PUT",
        ));
    client(&server)
        .export(&token(), "scan-1", "export-1", &export)
        .await
        .expect("export");
}

#[tokio::test]
async fn start() {
    let server = MockServer::start().await;
    authed("PATCH", "/v3/scans/start")
        .and(body_json(
            json!({ "trigger": ["scan-1", "scan-2"], "errorHandling": 1 }),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "success": [{ "id": "scan-1" }, null],
            "failed": [null, { "id": "scan-2" }]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let start = Start::new(["scan-1", "scan-2"]).error_handling(StartErrorHandling::Ignore);
    let response = client(&server)
        .start(&token(), &start)
        .await
        .expect("start");

    assert_eq!(response.success, ["scan-1"]);
    assert_eq!(response.failed, ["scan-2"]);
}

#[tokio::test]
async fn delete() {
    let server = MockServer::start().await;
    authed("PATCH", "/v3.1/scans/delete")
        .and(body_json(json!({
            "scans": [{ "id": "scan-1" }, { "id": "scan-2" }],
            "purge": true,
            "completionWebhook": "https://example.com/webhook/deleted",
            "headers": [["key", "value"]]
        })))
        .respond_with(ResponseTemplate::new(202))
        .expect(1)
        .mount(&server)
        .await;

    let delete = Delete::new(["scan-1", "scan-2"])
        .purge(true)
        .completion_webhook("https://example.com/webhook/deleted")
        .headers(vec![("key".to_owned(), "value".to_owned())]);
    client(&server)
        .delete(&token(), &delete)
        .await
        .expect("delete");
}

#[tokio::test]
async fn resend_webhook() {
    let server = MockServer::start().await;
    authed("POST", "/v3/scans/scan-1/webhooks/resend")
        .respond_with(ResponseTemplate::new(202))
        .expect(1)
        .mount(&server)
        .await;

    client(&server)
        .resend_webhook(&token(), "scan-1")
        .await
        .expect("resend webhook");
}

#[tokio::test]
async fn credits_balance() {
    let server = MockServer::start().await;
    authed("GET", "/v3/scans/credits")
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "Amount": 100 })))
        .expect(1)
        .mount(&server)
        .await;

    let balance = client(&server)
        .credits_balance(&token())
        .await
        .expect("credits balance");

    assert_eq!(balance.amount, 100);
}

#[tokio::test]
async fn usages_history_csv() {
    let server = MockServer::start().await;
    let csv = "Date,Credits\n01-01-2020,5\n";
    authed("GET", "/v3/scans/usages/history")
        .and(query_param("start", "01-01-2020"))
        .and(query_param("end", "31-01-2020"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(csv, "text/csv"))
        .expect(1)
        .mount(&server)
        .await;

    let body = client(&server)
        .usages_history_csv(&token(), "01-01-2020", "31-01-2020")
        .await
        .expect("usages history");

    assert_eq!(body, csv.as_bytes());
}

#[tokio::test]
async fn release_notes() {
    let server = MockServer::start().await;
    public("GET", "/v3/release-logs.json")
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "updates": [{
                "msgType": "NEW",
                "date": "2023-12-07T09:00:00Z",
                "htmlDescription": "Writing Feedback API"
            }]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let notes = client(&server)
        .release_notes()
        .await
        .expect("release notes");

    assert_eq!(notes.updates.len(), 1);
    assert_eq!(notes.updates[0].msg_type.as_deref(), Some("NEW"));
    assert_eq!(
        notes.updates[0].date.as_deref(),
        Some("2023-12-07T09:00:00Z")
    );
    assert_eq!(
        notes.updates[0].html_description.as_deref(),
        Some("Writing Feedback API")
    );
}

#[tokio::test]
async fn supported_file_types() {
    let server = MockServer::start().await;
    public("GET", "/v3/miscellaneous/supported-file-types")
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "textual": ["pdf", "docx", "txt"],
            "ocr": ["png", "jpg"]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let types = client(&server)
        .supported_file_types()
        .await
        .expect("supported file types");

    assert_eq!(types.textual, ["pdf", "docx", "txt"]);
    assert_eq!(types.ocr, ["png", "jpg"]);
}

#[tokio::test]
async fn ocr_supported_languages() {
    let server = MockServer::start().await;
    public("GET", "/v3/miscellaneous/ocr-languages-list")
        .respond_with(ResponseTemplate::new(200).set_body_json(json!(["af", "en", "zu"])))
        .expect(1)
        .mount(&server)
        .await;

    let languages = client(&server)
        .ocr_supported_languages()
        .await
        .expect("OCR languages");

    assert_eq!(languages, ["af", "en", "zu"]);
}

#[tokio::test]
async fn ai_text_detection() {
    let server = MockServer::start().await;
    authed("POST", "/v2/writer-detector/scan-1/check")
        .and(body_json(json!({
            "text": LIONS,
            "sandbox": true,
            "language": "en",
            "explain": true,
            "sensitivity": 2
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(fixture("ai_text_response.json")))
        .expect(1)
        .mount(&server)
        .await;

    let request = AiTextDetectionRequest::new(LIONS)
        .sandbox(true)
        .language("en")
        .explain(true)
        .sensitivity(2);
    let response = client(&server)
        .ai_text_detection(&token(), "scan-1", &request)
        .await
        .expect("AI Text Detection");

    assert_eq!(response.model_version.as_deref(), Some("v11.0"));
    assert_eq!(response.results.len(), 1);
    let result = &response.results[0];
    assert_eq!(result.classification, Some(2));
    assert_eq!(result.probability, Some(0.7316979));
    let positions = result.matches[0].text.as_ref().expect("text positions");
    let chars = positions.chars.as_ref().expect("chars");
    assert_eq!(chars.starts, [0]);
    assert_eq!(chars.lengths, [554]);
    assert_eq!(positions.words.as_ref().expect("words").lengths, [73]);
    let summary = response.summary.expect("summary");
    assert_eq!(summary.ai, Some(1.0));
    assert_eq!(summary.human, Some(0.0));
    let statistics = response
        .explain
        .as_ref()
        .and_then(|explain| explain.patterns.as_ref())
        .and_then(|patterns| patterns.statistics.as_ref())
        .expect("explain statistics");
    assert_eq!(statistics.ai_count.len(), 3);
    assert_eq!(statistics.source, [1, 1, 1]);
    let document = response
        .scanned_document
        .as_ref()
        .expect("scanned document");
    assert_eq!(document.scan_id.as_deref(), Some("scan-id"));
    assert_eq!(document.total_words, Some(73));
    assert_eq!(document.actual_credits, Some(1));
    assert_eq!(response.scan_type, None);
}

#[tokio::test]
async fn ai_image_detection() {
    let server = MockServer::start().await;
    authed("POST", "/v1/ai-image-detector/scan-1/check")
        .and(body_json(json!({
            "base64": "aW1hZ2U=",
            "filename": "image.png",
            "model": "ai-image-1-ultra",
            "sandbox": true
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(fixture("ai_image_response.json")))
        .expect(1)
        .mount(&server)
        .await;

    let request =
        AiImageDetectionRequest::from_bytes("image", "image.png", AI_IMAGE_1_ULTRA).sandbox(true);
    let response = client(&server)
        .ai_image_detection(&token(), "scan-1", &request)
        .await
        .expect("AI Image Detection");

    assert_eq!(response.model.as_deref(), Some("ai-image-1-ultra"));
    let mask = response.result.as_ref().expect("mask");
    assert_eq!(mask.starts, [0, 393_216]);
    assert_eq!(mask.lengths, [196_608, 393_216]);
    let summary = response.summary.expect("summary");
    assert_eq!(summary.ai, Some(0.75));
    assert_eq!(summary.human, Some(0.25));
    assert_eq!(response.is_ai_detected, Some(true));
    let info = response.image_info.as_ref().expect("image info");
    let shape = info.shape.expect("shape");
    assert_eq!((shape.width, shape.height), (Some(768), Some(1024)));
    let metadata = info.metadata.as_ref().expect("metadata");
    assert_eq!(metadata.issued_by.as_deref(), Some("OpenAI"));
    assert_eq!(metadata.app_or_device_used.as_deref(), Some("OpenAI-API"));
    assert_eq!(
        response
            .scanned_document
            .as_ref()
            .and_then(|document| document.expected_credits),
        Some(1)
    );
}

#[tokio::test]
async fn grammar_check() {
    let server = MockServer::start().await;
    let text = "Lions are the only cat that live in groups, called pride.";
    authed("POST", "/v1/writing-feedback/scan-1/check")
        .and(body_json(json!({
            "text": text,
            "sandbox": true,
            "language": "en",
            "score": {
                "grammarScoreWeight": 0.2,
                "mechanicsScoreWeight": 0.3,
                "sentenceStructureScoreWeight": 0.5,
                "wordChoiceScoreWeight": 0.4
            }
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(fixture("grammar_response.json")))
        .expect(1)
        .mount(&server)
        .await;

    let weights = ScoreWeights::default()
        .grammar_score_weight(0.2)
        .mechanics_score_weight(0.3)
        .sentence_structure_score_weight(0.5)
        .word_choice_score_weight(0.4);
    let request = GrammarCheckRequest::new(text)
        .sandbox(true)
        .language("en")
        .score(weights);
    let response = client(&server)
        .grammar_check(&token(), "scan-1", &request)
        .await
        .expect("Grammar Checker");

    let score = response.score.as_ref().expect("score");
    let corrections = score.corrections.as_ref().expect("score corrections");
    assert_eq!(corrections.overall_score, Some(79));
    assert_eq!(corrections.grammar_corrections_count, Some(2));
    assert_eq!(corrections.word_choice_score_weight, Some(1.0));
    let readability = score.readability.as_ref().expect("readability");
    assert_eq!(
        readability.readability_level_text.as_deref(),
        Some("10th to 12th Grader")
    );
    assert_eq!(
        score.statistics.expect("statistics").sentence_count,
        Some(5)
    );
    let chars = response
        .corrections
        .as_ref()
        .and_then(|corrections| corrections.text.as_ref())
        .and_then(|text| text.chars.as_ref())
        .expect("correction ranges");
    assert_eq!(chars.types, [5, 18]);
    assert_eq!(chars.starts, [13, 22]);
    assert_eq!(chars.lengths, [2, 10]);
    assert_eq!(chars.operation_texts, ["an ", "plagiarism "]);
    assert_eq!(
        response
            .scanned_document
            .as_ref()
            .and_then(|document| document.total_words),
        Some(61)
    );
}

#[tokio::test]
async fn grammar_correction_types() {
    let server = MockServer::start().await;
    public("GET", "/v1/writing-feedback/correction-types/en")
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "correctionTypes": [{
                "title": "General",
                "description": "A general correction detected.",
                "message": "A general correction detected",
                "id": 1,
                "category": 2
            }]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let types = client(&server)
        .grammar_correction_types("en")
        .await
        .expect("correction types");

    assert_eq!(types.correction_types.len(), 1);
    let general = &types.correction_types[0];
    assert_eq!(general.id, Some(1));
    assert_eq!(general.category, Some(2));
    assert_eq!(general.title.as_deref(), Some("General"));
    assert_eq!(
        general.message.as_deref(),
        Some("A general correction detected")
    );
}

#[tokio::test]
async fn text_moderation() {
    let server = MockServer::start().await;
    authed("POST", "/v1/text-moderation/scan-1/check")
        .and(body_json(json!({
            "text": "This is some text to scan.",
            "sandbox": true,
            "language": "en",
            "labels": [{ "id": "toxic-v1" }, { "id": "profanity-v1" }]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(fixture("moderation_response.json")))
        .expect(1)
        .mount(&server)
        .await;

    let request = TextModerationRequest::new(
        "This is some text to scan.",
        [labels::TOXIC_V1, labels::PROFANITY_V1],
    )
    .sandbox(true)
    .language(LANGUAGE_ENGLISH);
    let response = client(&server)
        .text_moderation(&token(), "scan-1", &request)
        .await
        .expect("Text Moderation");

    assert_eq!(response.model_version.as_deref(), Some("v1"));
    let chars = response
        .moderations
        .as_ref()
        .and_then(|moderations| moderations.text.as_ref())
        .and_then(|text| text.chars.as_ref())
        .expect("moderation ranges");
    assert_eq!(chars.labels, [1]);
    assert_eq!(chars.starts, [15]);
    assert_eq!(chars.lengths, [4]);
    assert_eq!(response.legend.len(), 3);
    assert_eq!(response.legend[1].index, Some(1));
    assert_eq!(response.legend[1].id.as_deref(), Some("profanity-v1"));
    assert_eq!(
        response
            .scanned_document
            .as_ref()
            .and_then(|document| document.total_words),
        Some(86)
    );
}
