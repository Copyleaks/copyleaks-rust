# Copyleaks Rust SDK

Copyleaks is the integrity layer for your tech stack that helps organizations and individuals understand what's human-made and original, AI-generated, or manipulated across text, image, audio and video.

This is the official Rust SDK for the Copyleaks API. It gives you async access to:

- AI Text Detection
- AI Image Detection (synchronous, multipart upload, or by URL with a webhook)
- AI Video Detection
- Plagiarism Checker, with Grammar Checker, Gen-AI Overview and Citation Checker inside a scan
- Image Plagiarism Detector
- Grammar Checker
- Text Moderation
- Private Cloud Hub repository info

## Install

```sh
cargo add copyleaks@2
cargo add tokio --features macros,rt-multi-thread
```

The crate needs Rust 1.88 or later: its dependencies (such as the `icu` crates behind `url`)
require it.

You need a Copyleaks account and an API key. Find your key on the [API Dashboard](https://api.copyleaks.com/dashboard).

Upgrading from 1.x? See the upgrade notes in the [changelog](CHANGELOG.md).

## Quickstart: log in and detect AI-generated text

```rust
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
```

A login token is valid for 48 hours. Log in once and reuse the token; the client checks its expiry
before every call and returns `Error::TokenExpired` when it is time to log in again.

Every request type has a constructor for its required fields and a chainable setter for each
optional field. The request, response and option types are `#[non_exhaustive]`, so that new API
fields can arrive in minor releases: build them with the constructors and setters (or
`Default::default()` and the setters), not with struct literals. Every response type is a typed
struct: unknown fields are ignored and missing fields are `None`.

The snippets below continue inside the quickstart's `main`: `client` is a `Client` and `token`
is the login token.

## AI Text Detection

```rust
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
```

The text must be 255 to 100,000 characters long, counted in UTF-16 code units as the server
counts them.

## AI Image Detection

```rust
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
```

To check an image by URL, submit it and receive the same `AiImageDetectionResponse` at your
webhook:

```rust
use copyleaks::ai_image::{AiImageDetectionSubmission, AI_IMAGE_1_ULTRA};

let submission = AiImageDetectionSubmission::new(
    "https://example.com/photo.png",
    "photo.png",
    AI_IMAGE_1_ULTRA,
    "https://your.server/webhook/ai-image",
)
.sandbox(true);
client.submit_ai_image_detection(&token, "my-image-2", &submission).await?;
```

## AI Video Detection

The video is checked asynchronously: Copyleaks downloads it and posts an
`ai_video::AiVideoDetectionResponse` to your webhook.

```rust
use copyleaks::ai_video::{AiVideoDetectionSubmission, AI_VIDEO_1_PRO};

let submission = AiVideoDetectionSubmission::new(
    "https://example.com/clip.mp4",
    "clip.mp4",
    AI_VIDEO_1_PRO,
    "https://your.server/webhook/ai-video",
)
.sandbox(true);
client.submit_ai_video_detection(&token, "my-video-1", &submission).await?;
```

## Plagiarism Checker

A plagiarism scan is asynchronous. The submit returns as soon as Copyleaks accepts the scan, and
Copyleaks calls your status webhook when the scan finishes.

```rust
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
```

A scan can also run the Grammar Checker, the Gen-AI Overview, the Citation Checker, AI Logic,
Internal AI Source Match and sensitive data masking:

```rust
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
```

Scan ids are 3 to 36 characters long on every endpoint. Plagiarism Checker submits also allow
only `a-z`, `0-9` and `!@$&=_()';:.,~-`, and reject ids that contain `g`, one or more `o` or `0`,
then `gle` (for example `google`). `export`, the Image Plagiarism Detector and the asynchronous
AI Image and AI Video Detection apply the same rule with a wider character set.

Also available: `submit_file_ocr` (images and scanned documents), `export`, `start`, `delete`,
`resend_webhook`, `credits_balance` and `usages_history_csv`.

### Read the webhook

Parse the body your webhook receives with the types in `copyleaks::webhooks`. When AI Text
Detection is on, the completed webhook carries the AI result as a `suspected-ai-text` alert,
which the SDK decodes for you:

```rust
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
```

The other payloads are `ErrorWebhook`, `CreditsCheckedWebhook`, `IndexedWebhook`,
`NewResultWebhook` and `ExportCompletedWebhook`.

### Export the reports

`export` uploads the scan's artifacts to your servers. Each target receives a file whose type is
in `copyleaks::exports`: `ExportedResult`, `CrawledVersion` (with the Citation Checker results)
and `AiOverview`, plus the `AiTextDetectionResponse` and `GrammarCheckResponse` reports.

```rust
use copyleaks::plagiarism::{Export, ExportTarget};

let export = Export::new("https://your.server/export/export-1/completed")
    .completion_webhook_headers(vec![("Authorization".to_owned(), "Bearer abc".to_owned())])
    .ai_detection(ExportTarget::new("https://your.server/export/export-1/ai", "PUT"))
    .writing_feedback(ExportTarget::new("https://your.server/export/export-1/writing", "PUT"))
    .overview(ExportTarget::new("https://your.server/export/export-1/overview", "PUT"))
    .crawled_version(ExportTarget::new("https://your.server/export/export-1/crawled", "PUT"));
client.export(&token, "my-plagiarism-scan-1", "export-1", &export).await?;
```

## Image Plagiarism Detector

Searches the web for copies of an image, full or modified, in one call. The image is uploaded
as `multipart/form-data`.

```rust
use copyleaks::image_plagiarism::ImagePlagiarismRequest;

let image = std::fs::read("photo.jpg")?;
let request = ImagePlagiarismRequest::new(image, "photo.jpg").sandbox(true);
let response = client.image_plagiarism_detection(&token, "my-image-3", &request).await?;
if let Some(matches) = &response.matches {
    for found in &matches.internet {
        println!("{:?} ({:?}) on {} pages", found.url, found.kind(), found.web_pages.len());
    }
}
```

## Grammar Checker

```rust
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
```

## Text Moderation

```rust
use copyleaks::moderation::{labels, TextModerationRequest, LANGUAGE_ENGLISH};

let request = TextModerationRequest::new("Text to moderate.", [labels::TOXIC_V1, labels::PROFANITY_V1])
    .language(LANGUAGE_ENGLISH)
    .sandbox(true);
let response = client.text_moderation(&token, "my-moderation-1", &request).await?;
for entry in &response.legend {
    println!("label {:?} = {:?}", entry.index, entry.id);
}
```

## Reference data and Private Cloud Hub

```rust
// Public, no login needed.
let plagiarism_types = client.supported_file_types().await?;
let ai_types = client.supported_ai_text_detection_file_types().await?;
let cross = client.allowed_cross_languages().await?;
let ocr = client.ocr_supported_languages().await?;
println!("{} / {} file types", plagiarism_types.textual.len(), ai_types.supported_ai_file_types.len());
println!("cross-language results in {:?}; OCR in {} languages", cross.result_languages, ocr.len());

// Needs the Admin or Super Admin role in the repository.
let info = client.repository_info(&token, "my-repository-id").await?;
println!("{:?}: {:?} of {:?} credits used", info.name, info.current_credits, info.max_credits);
```

## Configuration

`Client::builder()` takes:

- `timeout` and `connect_timeout`: the limits of the default HTTP client, 120 seconds per request
  and 10 seconds to connect. The request timeout also bounds an upload: raise it to send large
  files over a slow connection.
- `retry_policy`: opt-in retries, described below. Default: no retries.
- `http_client`: your own `reqwest::Client`, for a proxy or extra root certificates. The default
  client trusts only the Mozilla root certificates (rustls with webpki roots). Behind a
  TLS-inspecting proxy with a corporate CA, pass a client built with that CA
  (`reqwest::ClientBuilder::add_root_certificate`). Set its timeouts on that client.
- `api_base_url` and `identity_base_url`: they must use `https`; `http` is only accepted for
  `localhost`, `127.0.0.1` and `::1`, for local test servers.
- `user_agent`: your application's product token, such as `my-app/2.1`, sent in front of the
  SDK's own `copyleaks-rust-sdk/<version>`.

```rust
use std::time::Duration;
use copyleaks::{Client, RetryPolicy};

let client = Client::builder()
    .user_agent("my-app/2.1")
    .timeout(Duration::from_secs(300))
    .retry_policy(RetryPolicy::new(3))
    .build()?;
```

The default client uses HTTP/2 when the server offers it, so concurrent calls share one
connection, and asks for gzip-compressed responses. Uploads are written once, straight into a
buffer of the final size: a file of N bytes costs about 1.4 × N of memory, and large files are
encoded on tokio's blocking thread pool.

The crate re-exports `reqwest` (`copyleaks::reqwest`), and `http_client` takes a
`reqwest::Client`, so reqwest 0.12 is part of this crate's public API: moving to a new major
version of reqwest is a breaking change and will come in a new major version of this crate.

Response bodies are read with limits: 16 MiB for JSON and 64 MiB for the usage history CSV. A
larger body fails the call with `Error::ResponseTooLarge`; it is never truncated.

### Retries

With a `RetryPolicy`, the client retries calls that fail with HTTP 429, HTTP 503 or a connection
error, but only the calls that are safe to repeat:

- the `GET` calls, which only read;
- the `PUT` submits (`submit_file`, `submit_file_ocr`, `submit_url` and
  `submit_ai_image_detection`), whose scan id makes a repeat harmless. If an earlier attempt
  reached the server after all, the repeat fails with HTTP 409 (the scan id is already used).

The calls that spend credits or change state with `POST` or `PATCH` are never retried: AI Text
and Image Detection, the Image Plagiarism Detector, AI Video Detection, Grammar Checker, Text
Moderation, login, export, start, delete and resend webhook.

A retry waits for the server's `Retry-After` header when it sends one, and fails at once when it
asks for more than `max_retry_after` (60 seconds by default). Otherwise the wait doubles from
`initial_backoff` (1 second) up to `max_backoff` (30 seconds), with jitter.

## Errors

Every call returns `Result<T, copyleaks::Error>`:

| Variant | When |
|---|---|
| `Unauthorized { body, endpoint, .. }` | HTTP 401: the token or the API key was rejected |
| `RateLimited { body, endpoint, retry_after, .. }` | HTTP 429: too many requests |
| `UnderMaintenance { body, endpoint, retry_after, .. }` | HTTP 503: retry later |
| `Api { status, body, endpoint, .. }` | Any other error status |
| `TokenExpired` | The token expires within 5 minutes; log in again |
| `ResponseTooLarge { endpoint, limit, .. }` | A response body over the limits above |
| `Validation` | The request failed a client-side check and was not sent |
| `Http`, `Json` | Transport (including timeouts) and decoding failures |

`retry_after` is the server's `Retry-After` header, when it sent one; `Error::retry_after()` and
`Error::status()` read it from any error. The status variants keep up to 1 MiB of the server's
response body, where it usually explains the error; their messages show the first 512 bytes.
The SDK never panics on API input, never prints, and never puts the token or the API key in an
error message.

## Examples

Runnable examples are in [`examples/`](examples). They read your credentials from the
`COPYLEAKS_EMAIL` and `COPYLEAKS_API_KEY` environment variables and use sandbox mode:

```sh
COPYLEAKS_EMAIL=you@example.com COPYLEAKS_API_KEY=your-api-key cargo run --example quickstart
```

## Documentation

API documentation, guides and references: <https://docs.copyleaks.com>.

## License

MIT. See [LICENSE](LICENSE).
