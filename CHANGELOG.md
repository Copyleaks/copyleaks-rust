# Changelog

All notable changes to this crate are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the crate follows
[Semantic Versioning](https://semver.org/).

## [2.0.0] - 2026-10-14

A major release. Request, option and response types are now `#[non_exhaustive]`, so that later
releases can add API fields without breaking your code. Read the upgrade notes below.

### Added

- Image Plagiarism Detector: `Client::image_plagiarism_detection` uploads an image as
  `multipart/form-data` and returns the copies found on the web (module `image_plagiarism`).
- AI Video Detection: `Client::submit_ai_video_detection` submits a video URL, and Copyleaks posts
  an `ai_video::AiVideoDetectionResponse` to your webhook.
- AI Image Detection by URL: `Client::submit_ai_image_detection` with an
  `AiImageDetectionSubmission`; the result is posted to your webhook.
- AI Image Detection with a multipart upload, without the base64 overhead:
  `Client::ai_image_detection_multipart`.
- AI Image Detection options `mask_type` (`MaskType::Binary`, `Heatmap` or `None`) and
  `object_detection`, with the response fields `AiImageMask::labels` and
  `AiImageDetectionResponse::detected_objects`.
- `Client::ai_text_detection_credits`, `Client::allowed_cross_languages` (each language with its
  name and code), `Client::supported_ai_text_detection_file_types`, and
  `Client::repository_info` for Private Cloud Hub repositories (module `repositories`).
- Scan properties: `sensitive_data_protection`, `writing_feedback` (Grammar Checker in a scan),
  `overview` (Gen-AI Overview), `internal_ai_source_match`, `references` (Citation Checker),
  `scan_time_zone`, `course`, `assignment`, `institution`, `AiGeneratedText::sensitivity` and
  `AiGeneratedText::explain`, `Scanning::include`, `ScanningExclude::backlinks_domains` and
  `ScanningExclude::text`, `Indexing::copyleaks_db`, and the PDF colors `ai_detection` and
  `writing_feedback`.
- Submit URL: `verb` and `headers`, to fetch pages that need a `POST` or a cookie.
- Export: `completion_webhook_headers`, and the `ai_detection`, `writing_feedback` and `overview`
  targets. The exported files have types in the new `exports` module: `ExportedResult`,
  `CrawledVersion` (with the Citation Checker results) and `AiOverview`.
- Webhooks: `Results::internal_ai_data`, `NewResultWebhook::internal_ai_data`,
  `CompletedWebhook::references_validation`, `CompletedWebhook::writing_feedback`, and
  `WebhookScannedDocument::expected_credits`, `enabled` and `detected_language`.
- Opt-in retries: `ClientBuilder::retry_policy(RetryPolicy::new(3))` retries the calls that are
  safe to repeat (`GET` calls and the `PUT` submits) after HTTP 429, HTTP 503 or a connection
  error. A retry waits for the server's `Retry-After` header when there is one, and backs off
  exponentially otherwise. Calls that spend credits with `POST` are never retried.
- `ClientBuilder::timeout` and `ClientBuilder::connect_timeout` for the default HTTP client.
- `retry_after` on `Error::RateLimited` and `Error::UnderMaintenance`, read from the
  `Retry-After` header, and the helpers `Error::retry_after()` and `Error::status()`.
- `from_value` and `value` on the integer enums, and the enums `RepositoryRole`,
  `RepositoryStatus` and `ImageMatchType`.
- `copyleaks::Headers` and `copyleaks::ResultWebhook`.

### Changed

- The default HTTP client uses HTTP/2 when the server offers it, asks for gzip-compressed
  responses, and gives up connecting after 10 seconds. The 120-second limit per request stays.
- Uploads (file, OCR and AI image) keep the file as you give it and write the request body once,
  into a buffer of the exact size, when the request is sent. An upload now needs about 1.4 times
  the file size in memory, down from 4 to 5 times. Large bodies are encoded, and large responses
  decoded, on tokio's blocking thread pool instead of the async runtime.
- `from_bytes` takes `impl Into<Vec<u8>>`, so a `Vec<u8>` is moved in without a copy.
- `Client::supported_file_types` calls the documented path
  `/v3/miscellaneous/supported-plagiarism-file-types`.
- `CompletedWebhook::ai_detection_result` decodes the alert data in one pass.
- The token expiry is parsed once per token, and the `User-Agent` header is built once per
  client.
- New client-side checks follow the server's rules: the new fields above,
  `UrlSubmission::verb` (`GET`, `POST` or `PUT`), and `Export::developer_payload` (at most 512
  characters).

### Fixed

- Calls that return no value (the submits, export, delete and resend webhook) now read the
  response body, so the connection goes back to the pool instead of being closed.

### Deprecated

- `Pdf::version` and `PdfVersion`, deprecated by the API. Use `Pdf::report_version`.
- `AiTextDetectionResult::probability`, deprecated by the API, which may stop sending it.

### Upgrade notes

- Request, option and response types are `#[non_exhaustive]`. Struct literals, including
  `Type { field, ..Default::default() }`, no longer compile outside the crate. Build values with
  the constructors and setters instead, for example `ScanProperties::new(url).sandbox(true)` or
  `Filters::default().safe_search(true)`. You can still read every field, and assign a field of a
  value you already have, for example `request.sandbox = Some(true)`. Patterns that destructure
  these types need `..`.
- `AuthToken` is `#[non_exhaustive]` too: create one with `AuthToken::new`, or deserialize it.
- The error variants with fields are `#[non_exhaustive]`: match them with `..`, for example
  `Error::Api { status, .. }`. `RateLimited` and `UnderMaintenance` have a new `retry_after`
  field.
- The enums `SubmitAction`, `ScanMethodAlgorithm`, `DomainsMode`, `MaskingPolicy`,
  `StartErrorHandling` and `PdfVersion` are `#[non_exhaustive]`: a `match` on them needs a
  wildcard arm.
- `FileSubmission`, `OcrSubmission` and `AiImageDetectionRequest` no longer have a public
  `base64` field: the content is private, so that it can be kept as raw bytes. Create them with
  `new` (base64 text) or `from_bytes` (raw bytes). Their `Debug` output shows the content size
  instead of the content.
- `from_bytes` takes `impl Into<Vec<u8>>` instead of `impl AsRef<[u8]>`. Byte slices, byte
  arrays, `Vec<u8>`, `&str` and `String` work as before. For another type, pass `.as_ref()`.
- `ScanProperties`, `FileSubmission`, `OcrSubmission` and `UrlSubmission` implement `PartialEq`
  but no longer `Eq`, because the Grammar Checker weights in `ScanProperties::writing_feedback`
  are `f64`.
- `ClientBuilder::build` fails with `Error::Validation` when `timeout` or `connect_timeout` is
  set together with `http_client` (set the timeouts on your own client), and when `user_agent`
  is not a valid header value.
- The SDK uses more `reqwest` features (`http2`, `gzip` and `multipart`) and depends on `tokio`
  (`rt` and `time`) and `bytes` directly. The minimum Rust version is still 1.88.
- Retries are off by default: nothing changes until you set a `RetryPolicy`.

## [1.0.0] - 2026-10-12

First release of the Copyleaks Rust SDK.

### Added

- Async `Client` with `Client::new()` and `Client::builder()` (custom `reqwest` client, base URLs,
  application user agent). The default HTTP client has a 120-second timeout. The SDK's own user
  agent token is `copyleaks-rust-sdk/1.0.0`. Base URLs must use `https` (`http` only for loopback
  hosts).
- Bounded response reads: 16 MiB for JSON and 64 MiB for the usage history CSV
  (`Error::ResponseTooLarge`), and up to 1 MiB of an error body kept on the status errors.
- Login, and a local token expiry check (`AuthToken::verify`) before every authenticated call.
- AI Text Detection (with `explain` and `sensitivity`), AI Image Detection, Grammar Checker
  (check and correction types) and Text Moderation, with typed responses.
- Plagiarism Checker: submit file, submit OCR, submit URL, export, start, delete, resend webhook,
  credits balance and usage history (CSV).
- Scan properties `aiSourceMatch` (`AiSourceMatch`), `displayLanguage`
  (`ScanProperties::display_language`) and `pdf.reportVersion` (`Pdf::report_version`). The display language and the report version are checked against the
  values the server accepts, case-sensitively.
- Release notes, supported file types and OCR languages.
- Typed webhook payloads (completed, error, credits checked, indexed, new result, export
  completed), `webhooks::parse`, and the AI Text Detection result of the completed webhook's
  `suspected-ai-text` alert (`CompletedWebhook::ai_detection_result`).
- Client-side validation of scan ids, text lengths (in UTF-16 code units) and request limits.
