//! The endpoints added in 2.0.0, against a local mock server: the method, the path, the headers
//! and the exact body (JSON or multipart), and the decoded response.

mod common;

use common::{client, multipart, only_request, token, FormPart, NoHeader, BEARER};
use copyleaks::ai_image::{
    AiImageDetectionRequest, AiImageDetectionSubmission, MaskType, ObjectDetection,
    AI_IMAGE_1_ULTRA,
};
use copyleaks::ai_video::{AiVideoDetectionSubmission, AI_VIDEO_1_PRO};
use copyleaks::image_plagiarism::{ImageMatchType, ImagePlagiarismRequest};
use copyleaks::plagiarism::MaskingPolicy;
use copyleaks::repositories::{RepositoryRole, RepositoryStatus};
use copyleaks::{ResultWebhook, DEFAULT_USER_AGENT};
use serde_json::json;
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockBuilder, MockServer, ResponseTemplate};

/// Matches the method, the path, the SDK user agent and the bearer token.
fn authed(verb: &str, url_path: &str) -> MockBuilder {
    Mock::given(method(verb))
        .and(path(url_path))
        .and(header("user-agent", DEFAULT_USER_AGENT))
        .and(header("authorization", BEARER))
}

/// Matches the method, the path and the SDK user agent of a call without a token.
fn public(verb: &str, url_path: &str) -> MockBuilder {
    Mock::given(method(verb))
        .and(path(url_path))
        .and(header("user-agent", DEFAULT_USER_AGENT))
        .and(NoHeader("authorization"))
}

/// A tiny PNG header, enough for a mock upload.
const PNG: &[u8] = b"\x89PNG\r\n\x1a\nfake image bytes";

#[tokio::test]
async fn image_plagiarism_detection_uploads_multipart() {
    let server = MockServer::start().await;
    authed("POST", "/v1/image-plagiarism-detector/img-scan-1/check")
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "developerPayload": "my-payload",
            "scannedImage": {
                "scanId": "img-scan-1",
                "expectedCredits": 1,
                "actualCredits": 1,
                "creationTime": "2026-05-24T10:00:00Z",
                "width": 1920,
                "height": 1080,
                "filename": "photo.png"
            },
            "matches": {
                "internet": [
                    {
                        "url": "https://example.com/images/photo.jpg",
                        "matchType": 0,
                        "webPages": [{ "url": "https://example.com/blog" }, null]
                    },
                    { "url": "https://example.org/photo-sm.jpg", "matchType": 1 }
                ],
                "score": { "totalMatches": 2, "fullMatches": 1, "partialMatches": 1 }
            }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let request = ImagePlagiarismRequest::new(PNG, "photo.png")
        .sandbox(true)
        .developer_payload("my-payload");
    let response = client(&server)
        .image_plagiarism_detection(&token(), "img-scan-1", &request)
        .await
        .expect("Image Plagiarism Detector");

    let sent = only_request(&server).await;
    assert_eq!(
        multipart(&sent),
        [
            FormPart::file("image", "photo.png", "image/png", PNG),
            FormPart::text("filename", "photo.png"),
            FormPart::text("sandbox", "true"),
            FormPart::text("developerPayload", "my-payload"),
        ]
    );
    let length: usize = sent
        .headers
        .get("content-length")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse().ok())
        .expect("a known content length, not a chunked body");
    assert_eq!(length, sent.body.len());

    assert_eq!(response.developer_payload.as_deref(), Some("my-payload"));
    let image = response.scanned_image.as_ref().expect("scanned image");
    assert_eq!(image.scan_id.as_deref(), Some("img-scan-1"));
    assert_eq!((image.width, image.height), (Some(1920), Some(1080)));
    assert_eq!(image.actual_credits, Some(1));
    let matches = response.matches.as_ref().expect("matches");
    assert_eq!(matches.internet.len(), 2);
    assert_eq!(matches.internet[0].kind(), Some(ImageMatchType::Full));
    assert_eq!(
        matches.internet[0].web_pages.len(),
        1,
        "null pages are skipped"
    );
    assert_eq!(matches.internet[1].kind(), Some(ImageMatchType::Partial));
    assert!(matches.internet[1].web_pages.is_empty());
    let score = matches.score.expect("score");
    assert_eq!(score.total_matches, Some(2));
    assert_eq!(score.full_matches, Some(1));
    assert_eq!(score.partial_matches, Some(1));
}

#[tokio::test]
async fn image_plagiarism_detection_sends_only_the_fields_set() {
    let server = MockServer::start().await;
    authed("POST", "/v1/image-plagiarism-detector/img-scan-2/check")
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({ "matches": {
            "internet": [], "score": { "totalMatches": 0, "fullMatches": 0, "partialMatches": 0 }
        } })),
        )
        .expect(1)
        .mount(&server)
        .await;

    let request = ImagePlagiarismRequest::new(PNG.to_vec(), "Photo.JPG");
    let response = client(&server)
        .image_plagiarism_detection(&token(), "img-scan-2", &request)
        .await
        .expect("Image Plagiarism Detector");

    assert_eq!(
        multipart(&only_request(&server).await),
        [
            FormPart::file("image", "Photo.JPG", "image/jpeg", PNG),
            FormPart::text("filename", "Photo.JPG"),
        ]
    );
    assert!(response.matches.expect("matches").internet.is_empty());
}

#[tokio::test]
async fn ai_image_detection_multipart_sends_the_raw_image() {
    let server = MockServer::start().await;
    authed("POST", "/v1/ai-image-detector/scan-1/check")
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "model": "ai-image-1-ultra",
            "summary": { "human": 0.25, "ai": 0.75 },
            "isAiDetected": true
        })))
        .expect(1)
        .mount(&server)
        .await;

    let request = AiImageDetectionRequest::from_bytes(PNG, "image.png", AI_IMAGE_1_ULTRA)
        .sandbox(true)
        .mask_type(MaskType::Heatmap)
        .object_detection(vec![
            ObjectDetection::new("face"),
            ObjectDetection::new("hand"),
        ]);
    let response = client(&server)
        .ai_image_detection_multipart(&token(), "scan-1", &request)
        .await
        .expect("AI Image Detection");

    assert_eq!(
        multipart(&only_request(&server).await),
        [
            FormPart::file("image", "image.png", "image/png", PNG),
            FormPart::text("filename", "image.png"),
            FormPart::text("model", "ai-image-1-ultra"),
            FormPart::text("sandbox", "true"),
            FormPart::text("maskType", "heatmap"),
            FormPart::text(
                "objectDetection",
                r#"[{"object":"face"},{"object":"hand"}]"#
            ),
        ]
    );
    assert_eq!(response.summary.and_then(|summary| summary.ai), Some(0.75));
}

#[tokio::test]
async fn ai_image_detection_multipart_decodes_a_base64_request() {
    let server = MockServer::start().await;
    authed("POST", "/v1/ai-image-detector/scan-1/check")
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .expect(1)
        .mount(&server)
        .await;

    // "image" in base64, with a line break as MIME encoders write it.
    let request = AiImageDetectionRequest::new("aW1h\nZ2U=", "photo.webp", AI_IMAGE_1_ULTRA);
    client(&server)
        .ai_image_detection_multipart(&token(), "scan-1", &request)
        .await
        .expect("AI Image Detection");

    assert_eq!(
        multipart(&only_request(&server).await),
        [
            FormPart::file("image", "photo.webp", "image/webp", b"image"),
            FormPart::text("filename", "photo.webp"),
            FormPart::text("model", "ai-image-1-ultra"),
        ]
    );
}

#[tokio::test]
async fn submit_ai_image_detection_by_url() {
    let server = MockServer::start().await;
    authed("PUT", "/v1/ai-image-detector-async/img-async-1/submit")
        .and(header("content-type", "application/json"))
        .and(body_json(json!({
            "url": "https://example.com/image1.png",
            "fileName": "image1.png",
            "model": "ai-image-1-ultra",
            "maskType": "heatmap",
            "objectDetection": [{ "object": "face" }],
            "headers": [["X-Custom-Auth", "my-token"]],
            "webhooks": {
                "url": "https://example.com/webhook/ai-image",
                "headers": [["Authorization", "Bearer hook"]]
            },
            "sandbox": true,
            "developerPayload": "order-1"
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({})))
        .expect(1)
        .mount(&server)
        .await;

    let submission = AiImageDetectionSubmission::new(
        "https://example.com/image1.png",
        "image1.png",
        AI_IMAGE_1_ULTRA,
        "https://example.com/webhook/ai-image",
    )
    .webhooks(
        ResultWebhook::new("https://example.com/webhook/ai-image")
            .headers(vec![("Authorization".to_owned(), "Bearer hook".to_owned())]),
    )
    .mask_type(MaskType::Heatmap)
    .object_detection(vec![ObjectDetection::new("face")])
    .headers(vec![("X-Custom-Auth".to_owned(), "my-token".to_owned())])
    .sandbox(true)
    .developer_payload("order-1");
    client(&server)
        .submit_ai_image_detection(&token(), "img-async-1", &submission)
        .await
        .expect("AI Image Detection by URL");
}

#[tokio::test]
async fn submit_ai_image_detection_minimal_body() {
    let server = MockServer::start().await;
    authed("PUT", "/v1/ai-image-detector-async/img-async-2/submit")
        .and(body_json(json!({
            "url": "https://example.com/image1.png",
            "fileName": "image1.png",
            "model": "ai-image-1-ultra",
            "webhooks": { "url": "https://example.com/webhook/{STATUS}" }
        })))
        .respond_with(ResponseTemplate::new(201))
        .expect(1)
        .mount(&server)
        .await;

    let submission = AiImageDetectionSubmission::new(
        "https://example.com/image1.png",
        "image1.png",
        AI_IMAGE_1_ULTRA,
        "https://example.com/webhook/{STATUS}",
    );
    client(&server)
        .submit_ai_image_detection(&token(), "img-async-2", &submission)
        .await
        .expect("AI Image Detection by URL");
}

#[tokio::test]
async fn submit_ai_video_detection() {
    let server = MockServer::start().await;
    authed("POST", "/v1/ai-video-detector/video-1/submit")
        .and(header("content-type", "application/json"))
        .and(body_json(json!({
            "url": "https://example.com/my-video.mp4",
            "headers": [["X-Custom-Auth", "my-token"]],
            "verb": "GET",
            "filename": "my-video.mp4",
            "model": "ai-video-1-pro",
            "webhooks": {
                "url": "https://example.com/webhook/ai-video",
                "headers": [["Authorization", "Bearer hook"]]
            },
            "sandbox": true,
            "developerPayload": "order-12345"
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({})))
        .expect(1)
        .mount(&server)
        .await;

    let submission = AiVideoDetectionSubmission::new(
        "https://example.com/my-video.mp4",
        "my-video.mp4",
        AI_VIDEO_1_PRO,
        "https://example.com/webhook/ai-video",
    )
    .webhooks(
        ResultWebhook::new("https://example.com/webhook/ai-video")
            .headers(vec![("Authorization".to_owned(), "Bearer hook".to_owned())]),
    )
    .headers(vec![("X-Custom-Auth".to_owned(), "my-token".to_owned())])
    .verb("GET")
    .sandbox(true)
    .developer_payload("order-12345");
    client(&server)
        .submit_ai_video_detection(&token(), "video-1", &submission)
        .await
        .expect("AI Video Detection");
}

#[tokio::test]
async fn submit_ai_video_detection_minimal_body() {
    let server = MockServer::start().await;
    authed("POST", "/v1/ai-video-detector/video-2/submit")
        .and(body_json(json!({
            "url": "https://example.com/my-video.mov",
            "filename": "my-video.mov",
            "model": "ai-video-1-pro",
            "webhooks": { "url": "https://example.com/webhook/{STATUS}" }
        })))
        .respond_with(ResponseTemplate::new(201))
        .expect(1)
        .mount(&server)
        .await;

    let submission = AiVideoDetectionSubmission::new(
        "https://example.com/my-video.mov",
        "my-video.mov",
        AI_VIDEO_1_PRO,
        "https://example.com/webhook/{STATUS}",
    );
    client(&server)
        .submit_ai_video_detection(&token(), "video-2", &submission)
        .await
        .expect("AI Video Detection");
}

#[tokio::test]
async fn ai_text_detection_credits() {
    let server = MockServer::start().await;
    authed("GET", "/v2/writer-detector/credits")
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "credits": 382 })))
        .expect(1)
        .mount(&server)
        .await;

    let credits = client(&server)
        .ai_text_detection_credits(&token())
        .await
        .expect("AI Text Detection credits");
    assert_eq!(credits.credits, 382);
    assert!(only_request(&server).await.body.is_empty());
}

#[tokio::test]
async fn allowed_cross_languages_is_public() {
    let server = MockServer::start().await;
    public("GET", "/v3/miscellaneous/allowed-cross-languages")
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "documentLanguages": ["da", "nl", "en"],
            "resultLanguages": ["sq", "bg", "vi"]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let languages = client(&server)
        .allowed_cross_languages()
        .await
        .expect("cross languages");
    assert_eq!(languages.document_languages, ["da", "nl", "en"]);
    assert_eq!(languages.result_languages, ["sq", "bg", "vi"]);
}

#[tokio::test]
async fn supported_ai_text_detection_file_types_is_public() {
    let server = MockServer::start().await;
    public(
        "GET",
        "/v3/miscellaneous/supported-ai-text-detection-file-types",
    )
    .respond_with(ResponseTemplate::new(200).set_body_json(json!({
        "supportedAiFileTypes": ["pdf", "docx", "LaTeX", "html"]
    })))
    .expect(1)
    .mount(&server)
    .await;

    let types = client(&server)
        .supported_ai_text_detection_file_types()
        .await
        .expect("AI text file types");
    assert_eq!(
        types.supported_ai_file_types,
        ["pdf", "docx", "LaTeX", "html"]
    );
}

#[tokio::test]
async fn repository_info() {
    let server = MockServer::start().await;
    authed("GET", "/v3/repositories/repository/my-repo%20123/info")
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "private-data-hub-id",
            "name": "Private Data Hub Name",
            "description": "Your Description",
            "permission": 4,
            "status": 0,
            "maxCredits": 1000,
            "currentCredits": 900,
            "maskingPolicy": 1,
            "creationTime": "2024-09-09T10:43:52"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let info = client(&server)
        .repository_info(&token(), "my-repo 123")
        .await
        .expect("repository info");
    assert_eq!(info.id.as_deref(), Some("private-data-hub-id"));
    assert_eq!(info.name.as_deref(), Some("Private Data Hub Name"));
    assert_eq!(info.description.as_deref(), Some("Your Description"));
    assert_eq!(info.max_credits, Some(1000));
    assert_eq!(info.current_credits, Some(900));
    assert_eq!(info.creation_time.as_deref(), Some("2024-09-09T10:43:52"));
    assert_eq!(
        info.permission.and_then(RepositoryRole::from_value),
        Some(RepositoryRole::SuperAdmin)
    );
    assert_eq!(
        info.status.and_then(RepositoryStatus::from_value),
        Some(RepositoryStatus::Running)
    );
    assert_eq!(
        info.masking_policy.and_then(MaskingPolicy::from_value),
        Some(MaskingPolicy::MaskOtherUsersFiles)
    );
    assert_eq!(RepositoryStatus::from_value(99), None);
    assert_eq!(RepositoryRole::Viewer.value(), 1);
}
