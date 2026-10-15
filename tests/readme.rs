//! Keeps the README's Rust snippets compiling: each snippet is copied below into a function that
//! the compiler checks, and the test fails when a README snippet is not found here line by line.

#![allow(dead_code, unused_variables)]

/// The lines of a snippet, trimmed, without blank lines.
fn lines(text: &str) -> Vec<&str> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect()
}

#[test]
fn every_readme_snippet_is_compiled_here() {
    let readme = include_str!("../README.md");
    let here = lines(include_str!("readme.rs"));
    let mut snippets = 0;
    let mut rest = readme;
    while let Some(start) = rest.find("```rust\n") {
        let body = &rest[start + "```rust\n".len()..];
        let end = body.find("```").expect("closed code block");
        let snippet = lines(&body[..end]);
        assert!(
            here.windows(snippet.len()).any(|window| window == snippet),
            "this README snippet is not in tests/readme.rs:\n{}",
            snippet.join("\n")
        );
        snippets += 1;
        rest = &body[end + 3..];
    }
    assert!(snippets >= 10, "found {snippets} snippets");
}

/// The README snippets, exactly as written there (rustfmt would change their layout).
#[rustfmt::skip]
mod snippets {
    use copyleaks::{AuthToken, Client};

    type Result = std::result::Result<(), Box<dyn std::error::Error>>;

    mod quickstart {
        use copyleaks::ai_text::AiTextDetectionRequest;
        use copyleaks::Client;

        #[tokio::main]
        async fn main() -> Result<(), Box<dyn std::error::Error>> {
            let client = Client::new()?;
            let token = client.login("you@example.com", "your-api-key").await?;

            let text = "Lions are social animals, living in groups called prides, typically consisting of \
                several females, their offspring, and a few males. Female lions are the primary hunters, \
                working together to catch prey. Lions are known for their strength, teamwork, and \
                complex social structures.";
            // Sandbox mode returns mock results and uses no credits. Turn it off in production.
            let request = AiTextDetectionRequest::new(text).sandbox(true);
            let response = client.ai_text_detection(&token, "my-first-scan", &request).await?;

            if let Some(summary) = response.summary {
                println!("AI: {:?}, human: {:?}", summary.ai, summary.human);
            }
            for section in &response.results {
                println!("classification: {:?}", section.classification); // 1 = human, 2 = AI
            }
            Ok(())
        }
    }

    async fn ai_text(client: Client, token: AuthToken) -> Result {
        use copyleaks::ai_text::AiTextDetectionRequest;

        let text = "At least 255 characters of text ...";
        let request = AiTextDetectionRequest::new(text)
            .language("en")
            .explain(true) // AI Logic explanations, in `response.explain`
            .sensitivity(2) // 1 to 3: higher also finds more heavily edited AI text
            .sandbox(true);
        let response = client.ai_text_detection(&token, "my-scan-1", &request).await?;
        println!("model {:?}, AI score {:?}", response.model_version, response.summary.and_then(|s| s.ai));

        // The AI Text Detection credits left on your account.
        let credits = client.ai_text_detection_credits(&token).await?;
        println!("{} AI Text Detection credits left", credits.credits);
        Ok(())
    }

    async fn ai_image(client: Client, token: AuthToken) -> Result {
        use copyleaks::ai_image::{AiImageDetectionRequest, MaskType, ObjectDetection, AI_IMAGE_1_ULTRA};

        let image = std::fs::read("photo.png")?; // at least 512x512 px
        let request = AiImageDetectionRequest::from_bytes(image, "photo.png", AI_IMAGE_1_ULTRA)
            .mask_type(MaskType::Heatmap) // or Binary (the default), or None for scores only
            .object_detection(vec![ObjectDetection::new("face")]) // up to 3 objects to locate
            .sandbox(true);

        // multipart/form-data upload, without the base64 overhead. `ai_image_detection` sends the
        // same request as base64 JSON.
        let response = client.ai_image_detection_multipart(&token, "my-image-1", &request).await?;
        println!("AI share of the pixels: {:?}", response.summary.and_then(|s| s.ai));
        for found in &response.detected_objects {
            println!("found {:?}", found.object);
        }
        Ok(())
    }

    async fn ai_image_by_url(client: Client, token: AuthToken) -> Result {
        use copyleaks::ai_image::{AiImageDetectionSubmission, AI_IMAGE_1_ULTRA};

        let submission = AiImageDetectionSubmission::new(
            "https://example.com/photo.png",
            "photo.png",
            AI_IMAGE_1_ULTRA,
            "https://your.server/webhook/ai-image",
        )
        .sandbox(true);
        client.submit_ai_image_detection(&token, "my-image-2", &submission).await?;
        Ok(())
    }

    async fn ai_video(client: Client, token: AuthToken) -> Result {
        use copyleaks::ai_video::{AiVideoDetectionSubmission, AI_VIDEO_1_PRO};

        let submission = AiVideoDetectionSubmission::new(
            "https://example.com/clip.mp4",
            "clip.mp4",
            AI_VIDEO_1_PRO,
            "https://your.server/webhook/ai-video",
        )
        .sandbox(true);
        client.submit_ai_video_detection(&token, "my-video-1", &submission).await?;
        Ok(())
    }

    fn ai_video_webhook(body: &[u8]) -> Result {
        use copyleaks::ai_video::AiVideoDetectionResponse;
        use copyleaks::webhooks;

        // `body` is the raw request body (`&[u8]`) your web framework received.
        let video: AiVideoDetectionResponse = webhooks::parse(body)?;
        if let Some(error) = &video.error {
            eprintln!("detection failed: {:?} {:?}", error.id, error.message);
        } else {
            println!("AI-generated: {:?}", video.is_ai_detected);
        }
        Ok(())
    }

    async fn plagiarism(client: Client, token: AuthToken) -> Result {
        use copyleaks::plagiarism::{AiGeneratedText, FileSubmission, ScanProperties, UrlSubmission};

        // {STATUS} is replaced with completed, error, creditschecked or indexed.
        let properties = ScanProperties::new("https://your.server/webhook/{STATUS}")
            .sandbox(true)
            .ai_generated_text(AiGeneratedText::default().detect(true));

        let file = FileSubmission::from_bytes("Text to scan.", "essay.txt", properties.clone());
        client.submit_file(&token, "my-plagiarism-scan-1", &file).await?;

        // `verb` and `headers` fetch pages that need a POST or a cookie.
        let page = UrlSubmission::new("https://example.com/article", properties)
            .headers(vec![("Cookie".to_owned(), "session=abc".to_owned())]);
        client.submit_url(&token, "my-plagiarism-scan-2", &page).await?;
        Ok(())
    }

    fn scan_features() {
        use copyleaks::grammar::ScoreWeights;
        use copyleaks::plagiarism::{
            AiGeneratedText, AiGeneratedTextExplain, Course, InternalAiSourceMatch, Overview, References,
            ScanProperties, SensitiveDataProtection, WritingFeedback,
        };

        let properties = ScanProperties::new("https://your.server/webhook/{STATUS}")
            .ai_generated_text(
                AiGeneratedText::default()
                    .detect(true)
                    .sensitivity(2)
                    .explain(AiGeneratedTextExplain::default().enable(true)),
            )
            .writing_feedback(
                WritingFeedback::default()
                    .enable(true)
                    .score(ScoreWeights::default().grammar_score_weight(0.5)),
            )
            .overview(Overview::default().enable(true))
            .references(References::default().validate(true))
            .internal_ai_source_match(InternalAiSourceMatch::default().enable(true))
            .sensitive_data_protection(SensitiveDataProtection::default().email_address(true))
            .scan_time_zone("America/New_York")
            .course(Course::new("cs-101"));
    }

    fn read_webhook(body: &[u8]) -> Result {
        use copyleaks::webhooks::{self, CompletedWebhook};

        // `body` is the raw request body (`&[u8]`) your web framework received.
        let completed: CompletedWebhook = webhooks::parse(body)?;
        let score = completed.results.as_ref().and_then(|r| r.score).and_then(|s| s.aggregated_score);
        println!("plagiarism score: {score:?}");

        match completed.ai_detection_result() {
            Ok(Some(ai)) => println!("AI score: {:?}", ai.summary.and_then(|s| s.ai)),
            Ok(None) => println!("no AI alert in this scan"),
            // The alert data is not valid JSON. Acknowledge the webhook anyway.
            Err(error) => eprintln!("could not read the AI alert: {error}"),
        }

        // Grammar Checker scores, the Citation Checker summary and Internal AI Source Match results.
        let overall = completed
            .writing_feedback
            .as_ref()
            .and_then(|feedback| feedback.score.as_ref())
            .and_then(|score| score.overall_score);
        let references = completed.references_validation.as_ref().and_then(|r| r.summary);
        let internal_ai = completed.results.as_ref().map_or(0, |r| r.internal_ai_data.len());
        println!("writing {overall:?}, references {references:?}, {internal_ai} internal AI matches");
        Ok(())
    }

    async fn export(client: Client, token: AuthToken) -> Result {
        use copyleaks::plagiarism::{Export, ExportTarget};

        let export = Export::new("https://your.server/export/export-1/completed")
            .completion_webhook_headers(vec![("Authorization".to_owned(), "Bearer abc".to_owned())])
            .ai_detection(ExportTarget::new("https://your.server/export/export-1/ai", "PUT"))
            .writing_feedback(ExportTarget::new("https://your.server/export/export-1/writing", "PUT"))
            .overview(ExportTarget::new("https://your.server/export/export-1/overview", "PUT"))
            .crawled_version(ExportTarget::new("https://your.server/export/export-1/crawled", "PUT"));
        client.export(&token, "my-plagiarism-scan-1", "export-1", &export).await?;
        Ok(())
    }

    async fn image_plagiarism(client: Client, token: AuthToken) -> Result {
        use copyleaks::image_plagiarism::ImagePlagiarismRequest;

        let image = std::fs::read("photo.jpg")?;
        let request = ImagePlagiarismRequest::new(image, "photo.jpg").sandbox(true);
        let response = client.image_plagiarism_detection(&token, "my-image-3", &request).await?;
        if let Some(matches) = &response.matches {
            for found in &matches.internet {
                println!("{:?} ({:?}) on {} pages", found.url, found.kind(), found.web_pages.len());
            }
        }
        Ok(())
    }

    async fn grammar(client: Client, token: AuthToken) -> Result {
        use copyleaks::grammar::GrammarCheckRequest;

        let request = GrammarCheckRequest::new("Lions is social animals.").language("en").sandbox(true);
        let response = client.grammar_check(&token, "my-grammar-1", &request).await?;
        let overall = response
            .score
            .as_ref()
            .and_then(|score| score.corrections.as_ref())
            .and_then(|corrections| corrections.overall_score);
        println!("overall score: {overall:?}");

        // The names of the correction type ids in `response.corrections`. Public, no login needed.
        let types = client.grammar_correction_types("en").await?;
        Ok(())
    }

    async fn moderation(client: Client, token: AuthToken) -> Result {
        use copyleaks::moderation::{labels, TextModerationRequest, LANGUAGE_ENGLISH};

        let request = TextModerationRequest::new("Text to moderate.", [labels::TOXIC_V1, labels::PROFANITY_V1])
            .language(LANGUAGE_ENGLISH)
            .sandbox(true);
        let response = client.text_moderation(&token, "my-moderation-1", &request).await?;
        for entry in &response.legend {
            println!("label {:?} = {:?}", entry.index, entry.id);
        }
        Ok(())
    }

    async fn reference_data(client: Client, token: AuthToken) -> Result {
        // Public, no login needed.
        let plagiarism_types = client.supported_file_types().await?;
        let ai_types = client.supported_ai_text_detection_file_types().await?;
        let cross = client.allowed_cross_languages().await?;
        let ocr = client.ocr_supported_languages().await?;
        println!("{} / {} file types", plagiarism_types.textual.len(), ai_types.supported_ai_file_types.len());
        println!("cross-language results in {:?}; OCR in {} languages", cross.result_codes(), ocr.len());

        // Needs the Admin or Super Admin role in the repository.
        let info = client.repository_info(&token, "my-repository-id").await?;
        println!("{:?}: {:?} of {:?} credits used", info.name, info.current_credits, info.max_credits);
        Ok(())
    }

    fn configuration() -> Result {
        use std::time::Duration;
        use copyleaks::{Client, RetryPolicy};

        let client = Client::builder()
            .user_agent("my-app/2.1")
            .timeout(Duration::from_secs(300))
            .retry_policy(RetryPolicy::new(3))
            .build()?;
        Ok(())
    }
}
