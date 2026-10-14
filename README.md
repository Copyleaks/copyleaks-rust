# Copyleaks Rust SDK

Copyleaks is the integrity layer for your tech stack that helps organizations and individuals understand what's human-made and original, AI-generated, or manipulated across text, image, audio and video.

This is the official Rust SDK for the Copyleaks API. It gives you async access to:

- AI Text Detection
- AI Image Detection
- Plagiarism Checker
- Grammar Checker
- Text Moderation

## Install

```sh
cargo add copyleaks
cargo add tokio --features macros,rt-multi-thread
```

The crate needs Rust 1.88 or later: its dependencies (such as the `icu` crates behind `url`)
require it.

You need a Copyleaks account and an API key. Find your key on the [API Dashboard](https://api.copyleaks.com/dashboard).

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
optional field. Every response type is a typed struct: unknown fields are ignored and missing
fields are `None`.

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
```

The text must be 255 to 100,000 characters long, counted in UTF-16 code units as the server
counts them.

## AI Image Detection

```rust
use copyleaks::ai_image::{AiImageDetectionRequest, AI_IMAGE_1_ULTRA};

let image = std::fs::read("photo.png")?; // at least 512x512 px
let request = AiImageDetectionRequest::from_bytes(image, "photo.png", AI_IMAGE_1_ULTRA).sandbox(true);
let response = client.ai_image_detection(&token, "my-image-1", &request).await?;
println!("AI share of the pixels: {:?}", response.summary.and_then(|s| s.ai));
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

let page = UrlSubmission::new("https://example.com/article", properties);
client.submit_url(&token, "my-plagiarism-scan-2", &page).await?;
```

Scan ids are 3 to 36 characters long on every endpoint. Plagiarism Checker submits also allow
only `a-z`, `0-9` and `!@$&=_()';:.,~-`, and reject ids that contain `g`, one or more `o` or `0`,
then `gle` (for example `google`). `export` applies the same rule to its scan id and export id,
with a wider character set.

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
```

The other payloads are `ErrorWebhook`, `CreditsCheckedWebhook`, `IndexedWebhook`,
`NewResultWebhook` and `ExportCompletedWebhook`.

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

## Configuration

`Client::builder()` takes:

- `http_client`: your own `reqwest::Client`. The default client has a 120-second timeout per
  request and trusts only the Mozilla root certificates (rustls with webpki roots). Behind a
  TLS-inspecting proxy with a corporate CA, pass a client built with that CA
  (`reqwest::ClientBuilder::add_root_certificate`). The SDK never retries; add retries in your
  own code if you need them.
- `api_base_url` and `identity_base_url`: they must use `https`; `http` is only accepted for
  `localhost`, `127.0.0.1` and `::1`, for local test servers.
- `user_agent`: your application's product token, such as `my-app/2.1`, sent in front of the
  SDK's own `copyleaks-rust-sdk/<version>`.

The crate re-exports `reqwest` (`copyleaks::reqwest`), and `http_client` takes a
`reqwest::Client`, so reqwest 0.12 is part of this crate's public API: moving to a new major
version of reqwest is a breaking change and will come in a new major version of this crate.

Response bodies are read with limits: 16 MiB for JSON and 64 MiB for the usage history CSV. A
larger body fails the call with `Error::ResponseTooLarge`; it is never truncated.

## Errors

Every call returns `Result<T, copyleaks::Error>`:

| Variant | When |
|---|---|
| `Unauthorized { body, endpoint }` | HTTP 401: the token or the API key was rejected |
| `RateLimited { body, endpoint }` | HTTP 429: too many requests |
| `UnderMaintenance { body, endpoint }` | HTTP 503: retry with exponential backoff |
| `Api { status, body, endpoint }` | Any other error status |
| `TokenExpired` | The token expires within 5 minutes; log in again |
| `ResponseTooLarge { endpoint, limit }` | A response body over the limits above |
| `Validation` | The request failed a client-side check and was not sent |
| `Http`, `Json` | Transport (including timeouts) and decoding failures |

The status variants keep up to 1 MiB of the server's response body, where it usually explains
the error; their messages show the first 512 bytes. The SDK never panics on API input, never
prints, and never puts the token or the API key in an error message.

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
