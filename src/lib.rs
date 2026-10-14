//! The official Copyleaks API SDK for Rust.
//!
//! Copyleaks is the integrity layer for your tech stack that helps organizations and individuals
//! understand what's human-made and original, AI-generated, or manipulated across text, image,
//! audio and video.
//!
//! This crate gives async access to:
//!
//! - AI Text Detection: [`Client::ai_text_detection`]
//! - AI Image Detection: [`Client::ai_image_detection`],
//!   [`Client::ai_image_detection_multipart`] and, by URL with a webhook,
//!   [`Client::submit_ai_image_detection`]
//! - AI Video Detection: [`Client::submit_ai_video_detection`]
//! - Plagiarism Checker: [`Client::submit_file`], [`Client::submit_url`],
//!   [`Client::submit_file_ocr`] and the scan management calls, with the webhook payloads in
//!   [`webhooks`] and the exported files in [`exports`]
//! - Image Plagiarism Detector: [`Client::image_plagiarism_detection`]
//! - Grammar Checker: [`Client::grammar_check`]
//! - Text Moderation: [`Client::text_moderation`]
//! - Private Cloud Hub: [`Client::repository_info`]
//!
//! # Quickstart
//!
//! ```no_run
//! # async fn run() -> Result<(), copyleaks::Error> {
//! use copyleaks::ai_text::AiTextDetectionRequest;
//! use copyleaks::Client;
//!
//! let client = Client::new()?;
//! let token = client.login("you@example.com", "your-api-key").await?;
//!
//! let text = "Lions are social animals, living in groups called prides, typically consisting of \
//!     several females, their offspring, and a few males. Female lions are the primary hunters, \
//!     working together to catch prey. Lions are known for their strength, teamwork, and complex \
//!     social structures.";
//! let request = AiTextDetectionRequest::new(text).sandbox(true);
//! let response = client.ai_text_detection(&token, "my-first-scan", &request).await?;
//! if let Some(summary) = response.summary {
//!     println!("AI: {:?}, human: {:?}", summary.ai, summary.human);
//! }
//! # Ok(())
//! # }
//! ```
//!
//! # Building requests
//!
//! Every request type has a constructor for its required fields and a chainable setter for each
//! optional field. The request, response and option types are `#[non_exhaustive]`, so that new
//! API fields can be added without a breaking change: build them with the constructors and
//! setters (or `Default::default()` and the setters), not with struct literals.
//!
//! Every call returns [`Result`]. The library never panics on API input and never prints.
//! Full API documentation: <https://docs.copyleaks.com>.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

#[macro_use]
mod macros;

pub mod ai_image;
pub mod ai_text;
pub mod ai_video;
mod auth;
mod body;
mod client;
mod common;
mod de;
mod error;
pub mod exports;
pub mod grammar;
pub mod image_plagiarism;
pub mod misc;
pub mod moderation;
pub mod plagiarism;
pub mod repositories;
mod retry;
mod validate;
pub mod webhooks;

pub use auth::AuthToken;
pub use client::{
    Client, ClientBuilder, DEFAULT_API_BASE_URL, DEFAULT_IDENTITY_BASE_URL, DEFAULT_USER_AGENT,
};
pub use common::{Headers, ResultWebhook, ScannedDocument};
pub use error::{Error, Result};
/// The `reqwest` crate (0.12) this crate is built on, for
/// [`ClientBuilder::http_client`](method@ClientBuilder::http_client).
///
/// reqwest is part of this crate's public API: a new major version of reqwest comes with a new
/// major version of this crate.
pub use reqwest;
pub use retry::RetryPolicy;

/// The version of this crate.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
