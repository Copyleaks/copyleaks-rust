//! Upload bodies built without extra copies of the file, and CPU-heavy work moved off the async
//! runtime.
//!
//! A file to upload is kept as given: raw bytes, or a base64 string. The JSON body is written once,
//! into a buffer of the exact final size, when the request is sent. For a file of N bytes the SDK
//! allocates about 1.34 × N (the base64 text and a little JSON), and a retry reuses the same body.

use std::borrow::Cow;
use std::fmt;

use base64::Engine as _;
use bytes::Bytes;
use serde::{Serialize, Serializer};

use crate::{Error, Result};

/// Bodies at least this large are built, and responses at least this large are decoded, on
/// tokio's blocking thread pool, so a large file does not stall the async runtime.
pub(crate) const OFF_RUNTIME_THRESHOLD: usize = 8 << 20;

const BASE64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

/// The content of an upload.
#[derive(Clone, PartialEq, Eq)]
pub(crate) enum Content {
    /// Raw bytes, base64-encoded when the request is sent.
    Raw(Bytes),
    /// Base64 text given by the caller. Always valid UTF-8: it comes from a `String`.
    Base64(Bytes),
}

impl Default for Content {
    fn default() -> Self {
        Content::Base64(Bytes::new())
    }
}

impl Content {
    /// Raw bytes. A `Vec<u8>` is taken without a copy.
    pub(crate) fn raw(bytes: impl Into<Vec<u8>>) -> Self {
        Content::Raw(Bytes::from(bytes.into()))
    }

    /// Base64 text. A `String` is taken without a copy.
    pub(crate) fn base64(text: impl Into<String>) -> Self {
        Content::Base64(Bytes::from(text.into().into_bytes()))
    }

    /// True when there is nothing to upload.
    pub(crate) fn is_empty(&self) -> bool {
        match self {
            Content::Raw(bytes) => bytes.is_empty(),
            Content::Base64(text) => text.iter().all(u8::is_ascii_whitespace),
        }
    }

    /// The size of the content as given, in bytes.
    pub(crate) fn len(&self) -> usize {
        match self {
            Content::Raw(bytes) | Content::Base64(bytes) => bytes.len(),
        }
    }

    /// The base64 text: borrowed when given as base64, encoded otherwise.
    pub(crate) fn to_base64(&self) -> Cow<'_, str> {
        match self {
            Content::Raw(bytes) => Cow::Owned(BASE64.encode(bytes)),
            Content::Base64(text) => String::from_utf8_lossy(text),
        }
    }

    /// The raw bytes: shared when given as bytes, decoded otherwise. A `data:...;base64,` prefix
    /// and ASCII whitespace in base64 text are ignored.
    pub(crate) fn to_raw(&self) -> Result<Bytes> {
        match self {
            Content::Raw(bytes) => Ok(bytes.clone()),
            Content::Base64(text) => {
                let text = match text.iter().position(|&b| b == b',') {
                    Some(comma) if text.starts_with(b"data:") => &text[comma + 1..],
                    _ => &text[..],
                };
                let clean: Vec<u8> = text
                    .iter()
                    .copied()
                    .filter(|b| !b.is_ascii_whitespace())
                    .collect();
                BASE64.decode(clean).map(Bytes::from).map_err(|error| {
                    Error::Validation(format!("base64 is not valid base64: {error}"))
                })
            }
        }
    }
}

impl fmt::Debug for Content {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Content::Raw(bytes) => write!(f, "<{} bytes>", bytes.len()),
            Content::Base64(text) => write!(f, "<base64, {} characters>", text.len()),
        }
    }
}

impl Serialize for Content {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_base64())
    }
}

/// A JSON upload body: `{"base64":"<content>",<the other fields>}`.
#[derive(Clone)]
pub(crate) struct UploadBody {
    content: Content,
    /// The other fields, as a JSON object, for example `{"filename":"a.txt"}`.
    rest: Vec<u8>,
}

impl UploadBody {
    /// Serializes the other fields now (they are small), and keeps the content for
    /// [`build`](Self::build).
    pub(crate) fn new<T: Serialize + ?Sized>(content: &Content, rest: &T) -> Result<Self> {
        Ok(Self {
            content: content.clone(),
            rest: serde_json::to_vec(rest)?,
        })
    }

    /// About the size of the final body, in bytes.
    pub(crate) fn size_hint(&self) -> usize {
        let content = match &self.content {
            Content::Raw(bytes) => bytes.len() / 3 * 4,
            Content::Base64(text) => text.len(),
        };
        content + self.rest.len()
    }

    /// Writes the body into a buffer of the exact final size. Raw content is base64-encoded
    /// straight into the buffer.
    pub(crate) fn build(&self) -> Result<Vec<u8>> {
        const HEAD: &[u8] = b"{\"base64\":";
        let tail = if self.rest == b"{}" {
            1
        } else {
            self.rest.len()
        };
        let mut body;
        match &self.content {
            Content::Raw(bytes) => {
                let encoded = base64::encoded_len(bytes.len(), true).ok_or_else(|| {
                    Error::Validation("the file is too large to encode".to_owned())
                })?;
                body = Vec::with_capacity(HEAD.len() + encoded + 2 + tail);
                body.extend_from_slice(HEAD);
                body.push(b'"');
                let start = body.len();
                body.resize(start + encoded, 0);
                let written = BASE64
                    .encode_slice(bytes, &mut body[start..])
                    .map_err(|error| {
                        Error::Validation(format!("cannot encode the file: {error}"))
                    })?;
                body.truncate(start + written);
                body.push(b'"');
            }
            Content::Base64(text) => {
                // Base64 text needs no escaping, but the caller's string may hold anything, so
                // it is written as an escaped JSON string.
                body = Vec::with_capacity(HEAD.len() + text.len() + 2 + tail);
                body.extend_from_slice(HEAD);
                serde_json::to_writer(&mut body, &*String::from_utf8_lossy(text))?;
            }
        }
        if self.rest == b"{}" {
            body.push(b'}');
        } else {
            body.push(b',');
            body.extend_from_slice(self.rest.get(1..).unwrap_or_default());
        }
        Ok(body)
    }
}

/// Runs `work` on tokio's blocking thread pool when `size` is at least
/// [`OFF_RUNTIME_THRESHOLD`] and a tokio runtime is running, and in place otherwise.
pub(crate) async fn off_runtime<R, F>(size: usize, work: F) -> R
where
    R: Send + 'static,
    F: FnOnce() -> R + Clone + Send + 'static,
{
    if size < OFF_RUNTIME_THRESHOLD {
        return work();
    }
    let Ok(handle) = tokio::runtime::Handle::try_current() else {
        return work();
    };
    let fallback = work.clone();
    match handle.spawn_blocking(work).await {
        Ok(result) => result,
        Err(error) => match error.try_into_panic() {
            Ok(payload) => std::panic::resume_unwind(payload),
            // Cancelled because the runtime is shutting down: do the work here.
            Err(_) => fallback(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Serialize)]
    struct Rest<'a> {
        filename: &'a str,
    }

    #[test]
    fn raw_content_is_encoded_into_an_exact_size_buffer() {
        let raw: Vec<u8> = (0..=255u8).cycle().take(1 << 20).collect();
        let body = UploadBody::new(&Content::raw(raw.clone()), &Rest { filename: "a.bin" })
            .expect("body")
            .build()
            .expect("build");
        assert_eq!(
            body.len(),
            body.capacity(),
            "no spare capacity, no regrowth"
        );
        let encoded_len = raw.len().div_ceil(3) * 4;
        assert!(body.len() < encoded_len + 64, "{} bytes", body.len());
        let value: serde_json::Value = serde_json::from_slice(&body).expect("valid JSON");
        assert_eq!(value["filename"], "a.bin");
        let decoded = BASE64
            .decode(value["base64"].as_str().expect("base64"))
            .expect("decodes");
        assert_eq!(decoded, raw);
    }

    #[test]
    fn base64_text_is_escaped_and_other_fields_follow() {
        let body = UploadBody::new(&Content::base64("aGk=\n\"x"), &Rest { filename: "a" })
            .expect("body")
            .build()
            .expect("build");
        let value: serde_json::Value = serde_json::from_slice(&body).expect("valid JSON");
        assert_eq!(value["base64"], "aGk=\n\"x");
        assert_eq!(value["filename"], "a");
    }

    #[test]
    fn empty_rest_closes_the_object() {
        let body = UploadBody::new(&Content::raw(*b"hi"), &serde_json::json!({}))
            .expect("body")
            .build()
            .expect("build");
        assert_eq!(body, br#"{"base64":"aGk="}"#);
    }

    #[test]
    fn content_round_trips() {
        assert_eq!(&Content::raw(*b"hi").to_base64()[..], "aGk=");
        assert_eq!(&Content::base64("aGk=").to_base64()[..], "aGk=");
        assert_eq!(&Content::base64("aG\nk=").to_raw().expect("raw")[..], b"hi");
        assert_eq!(
            &Content::base64("data:image/png;base64,aGk=")
                .to_raw()
                .expect("raw")[..],
            b"hi"
        );
        assert!(Content::base64("not base64!").to_raw().is_err());
        assert!(Content::base64(" \n").is_empty());
        assert!(Content::raw(Vec::new()).is_empty());
        assert_eq!(format!("{:?}", Content::raw(*b"hi")), "<2 bytes>");
    }

    #[tokio::test]
    async fn large_work_runs_off_the_runtime_and_small_work_in_place() {
        let here = std::thread::current().id();
        let small = off_runtime(10, move || std::thread::current().id()).await;
        assert_eq!(small, here);
        let large = off_runtime(OFF_RUNTIME_THRESHOLD, move || std::thread::current().id()).await;
        assert_ne!(large, here);
    }
}
