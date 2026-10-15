//! The official Copyleaks API SDK for Rust: AI Text, Image and Video Detection, the Plagiarism
//! Checker, the Image Plagiarism Detector, the Grammar Checker, Text Moderation and Private Cloud
//! Hubs. Every call is an async method of [`Client`] that returns [`Result`].
//!
//! ```no_run
//! # async fn run() -> Result<(), copyleaks::Error> {
//! use copyleaks::ai_text::AiTextDetectionRequest;
//! use copyleaks::Client;
//!
//! let client = Client::new()?;
//! let token = client.login("you@example.com", "your-api-key").await?;
//! let text = "Lions are social animals, living in groups called prides, typically consisting of \
//!     several females, their offspring, and a few males. Female lions are the primary hunters, \
//!     working together to catch prey. Lions are known for their strength, teamwork, and complex \
//!     social structures.";
//! let request = AiTextDetectionRequest::new(text).sandbox(true);
//! let response = client.ai_text_detection(&token, "my-first-scan", &request).await?;
//! println!("{:?}", response.summary);
//! # Ok(())
//! # }
//! ```
//!
//! Request and response types are `#[non_exhaustive]`: build requests with their constructors and
//! setters, not struct literals. Responses ignore unknown fields, and a missing or `null` list is
//! empty. API docs: <https://docs.copyleaks.com>.

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
/// The `reqwest` crate (0.12), for
/// [`ClientBuilder::http_client`](method@ClientBuilder::http_client). It is part of the public
/// API: a new major version of reqwest means a new major version here.
pub use reqwest;
pub use retry::RetryPolicy;

/// The version of this crate.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
