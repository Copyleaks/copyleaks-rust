# Changelog

All notable changes to this crate are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the crate follows
[Semantic Versioning](https://semver.org/).

## [1.0.0] - 2026-10-12

First release of the Copyleaks Rust SDK, with the same API coverage as the Copyleaks Python SDK.

### Added

- Async `Client` with `Client::new()` and `Client::builder()` (custom `reqwest` client, base URLs,
  tenant, application user agent). The default HTTP client has a 120-second timeout. The SDK's
  own user agent token is `copyleaks-rust-sdk/1.0.0`. Base URLs must use `https` (`http` only for
  loopback hosts).
- Bounded response reads: 16 MiB for JSON and 64 MiB for the usage history CSV
  (`Error::ResponseTooLarge`), and up to 1 MiB of an error body kept on the status errors.
- Login, and a local token expiry check (`AuthToken::verify`) before every authenticated call.
- AI Text Detection (with `explain` and `sensitivity`), AI Image Detection, Grammar Checker
  (check and correction types) and Text Moderation, with typed responses.
- Plagiarism Checker: submit file, submit OCR, submit URL, export, start, delete, resend webhook,
  credits balance and usage history (CSV).
- Release notes, supported file types and OCR languages.
- Typed webhook payloads (completed, error, credits checked, indexed, new result, export
  completed), `webhooks::parse`, and the AI Text Detection result of the completed webhook's
  `suspected-ai-text` alert (`CompletedWebhook::ai_detection_result`).
- The `x-tenant` header, set on the client or per call with `Client::with_tenant`.
- Client-side validation of scan ids, text lengths (in UTF-16 code units) and request limits.
