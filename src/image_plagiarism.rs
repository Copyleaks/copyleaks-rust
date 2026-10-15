//! Image Plagiarism Detector: search the web for copies of an image, full or modified, with one
//! synchronous call:
//! [`Client::image_plagiarism_detection`](crate::Client::image_plagiarism_detection).
//!
//! ```no_run
//! # async fn run(client: copyleaks::Client, token: copyleaks::AuthToken) -> Result<(), Box<dyn std::error::Error>> {
//! use copyleaks::image_plagiarism::ImagePlagiarismRequest;
//!
//! let image = std::fs::read("photo.jpg")?;
//! let request = ImagePlagiarismRequest::new(image, "photo.jpg").sandbox(true);
//! let response = client.image_plagiarism_detection(&token, "my-image-1", &request).await?;
//! if let Some(score) = response.matches.as_ref().and_then(|matches| matches.score) {
//!     println!("{:?} matches", score.total_matches);
//! }
//! # Ok(())
//! # }
//! ```

use bytes::Bytes;
use serde::{Deserialize, Serialize};

use crate::{de, validate, Error, Result};

/// The file extensions the API accepts.
const EXTENSIONS: [&str; 7] = [".jpg", ".jpeg", ".png", ".gif", ".bmp", ".webp", ".ico"];

/// The largest image the API accepts: 20 MB.
const MAX_IMAGE_BYTES: usize = 20 << 20;

/// An image to search for, for
/// [`Client::image_plagiarism_detection`](crate::Client::image_plagiarism_detection). Under
/// 20 MB and at most 75 megapixels.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct ImagePlagiarismRequest {
    /// The image file, sent as the `image` part.
    image: Bytes,
    /// File name with extension (`.jpg`, `.jpeg`, `.png`, `.gif`, `.bmp`, `.webp`, `.ico`), at
    /// most 255 characters.
    pub filename: String,
    /// Sandbox mode: free mock results, for integration testing.
    pub sandbox: Option<bool>,
    /// A custom string returned unchanged in
    /// [`ImagePlagiarismResponse::developer_payload`](field@ImagePlagiarismResponse::developer_payload).
    pub developer_payload: Option<String>,
}

impl ImagePlagiarismRequest {
    /// Creates a request from the raw image bytes, taking a `Vec<u8>` without a copy.
    pub fn new(image: impl Into<Vec<u8>>, filename: impl Into<String>) -> Self {
        Self {
            image: Bytes::from(image.into()),
            filename: filename.into(),
            sandbox: None,
            developer_payload: None,
        }
    }

    /// The size of the image, in bytes.
    pub fn image_len(&self) -> usize {
        self.image.len()
    }

    pub(crate) fn validate(&self) -> Result<()> {
        if self.image.is_empty() {
            return Err(Error::Validation("image is required".to_owned()));
        }
        if self.image.len() > MAX_IMAGE_BYTES {
            return Err(Error::Validation(format!(
                "image must be at most {MAX_IMAGE_BYTES} bytes, got {}",
                self.image.len()
            )));
        }
        validate::required(&self.filename, "filename")?;
        validate::length(&self.filename, "filename", 1, 255)?;
        validate::extension(&self.filename, &EXTENSIONS)
    }

    pub(crate) fn image(&self) -> Bytes {
        self.image.clone()
    }

    /// The text fields of the multipart form, in order: `filename`, then the optional `sandbox`
    /// and `developerPayload`.
    pub(crate) fn form_fields(&self) -> Vec<(&'static str, String)> {
        let mut fields = vec![("filename", self.filename.clone())];
        if let Some(sandbox) = self.sandbox {
            fields.push(("sandbox", sandbox.to_string()));
        }
        if let Some(payload) = &self.developer_payload {
            fields.push(("developerPayload", payload.clone()));
        }
        fields
    }
}

setters!(ImagePlagiarismRequest {
    sandbox: bool,
    developer_payload: String,
});

/// The result of the Image Plagiarism Detector.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct ImagePlagiarismResponse {
    /// The developer payload you sent, unchanged.
    pub developer_payload: Option<String>,
    /// Information about the image and the scan.
    pub scanned_image: Option<ScannedImage>,
    /// The copies of the image found on the web.
    pub matches: Option<ImagePlagiarismMatches>,
}

/// Information about the submitted image and the scan.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct ScannedImage {
    /// The scan id you provided.
    pub scan_id: Option<String>,
    /// The number of credits the scan was expected to cost.
    pub expected_credits: Option<i64>,
    /// The number of credits spent on the scan.
    pub actual_credits: Option<i64>,
    /// When the scan was created, as an ISO 8601 time (UTC).
    pub creation_time: Option<String>,
    /// The image width, in pixels.
    pub width: Option<i64>,
    /// The image height, in pixels.
    pub height: Option<i64>,
    /// The file name you provided.
    pub filename: Option<String>,
}

/// The copies of an image found on the web.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ImagePlagiarismMatches {
    /// The matching images, full matches first. Each image URL appears once. Empty when no
    /// copy was found.
    #[serde(default, deserialize_with = "de::vec_skip_nulls")]
    pub internet: Vec<ImageMatch>,
    /// The number of matches.
    pub score: Option<ImagePlagiarismScore>,
}

/// A matching image on the web.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct ImageMatch {
    /// The URL of the matching image.
    pub url: Option<String>,
    /// `0` for a full match (an exact or near-exact copy), `1` for a partial match (cropped,
    /// resized, recolored or otherwise modified).
    pub match_type: Option<i32>,
    /// The web pages that show the image. Empty when the image was not found on a page.
    #[serde(default, deserialize_with = "de::vec_skip_nulls")]
    pub web_pages: Vec<WebPage>,
}

impl ImageMatch {
    /// The kind of match, or `None` for a value this version of the SDK does not know.
    pub fn kind(&self) -> Option<ImageMatchType> {
        self.match_type
            .and_then(|value| ImageMatchType::from_value(i64::from(value)))
    }
}

int_enum! {
    /// The kind of an [`ImageMatch`].
    ImageMatchType {
        /// An exact or near-exact copy of the image.
        Full = 0,
        /// A cropped, resized, recolored or otherwise modified copy.
        Partial = 1,
    }
}

/// A web page that shows a matching image.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct WebPage {
    /// The URL of the page.
    pub url: Option<String>,
}

/// The number of matches of an image.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct ImagePlagiarismScore {
    /// The number of matching images.
    pub total_matches: Option<i64>,
    /// The number of full matches.
    pub full_matches: Option<i64>,
    /// The number of partial matches.
    pub partial_matches: Option<i64>,
}
