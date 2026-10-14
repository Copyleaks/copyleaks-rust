//! AI Video Detection: find AI-generated content in the audio and visual tracks of a video.
//!
//! The detection is asynchronous: [`Client::submit_ai_video_detection`] returns once Copyleaks
//! accepts the video, and Copyleaks then posts an [`AiVideoDetectionResponse`] to your webhook.
//! Parse it with [`webhooks::parse`](crate::webhooks::parse).
//!
//! [`Client::submit_ai_video_detection`]: crate::Client::submit_ai_video_detection
//!
//! ```no_run
//! # async fn run(client: copyleaks::Client, token: copyleaks::AuthToken) -> Result<(), copyleaks::Error> {
//! use copyleaks::ai_video::{AiVideoDetectionSubmission, AI_VIDEO_1_PRO};
//!
//! let submission = AiVideoDetectionSubmission::new(
//!     "https://example.com/my-video.mp4",
//!     "my-video.mp4",
//!     AI_VIDEO_1_PRO,
//!     "https://your.server/webhook/ai-video",
//! )
//! .sandbox(true);
//! client.submit_ai_video_detection(&token, "my-video-1", &submission).await?;
//! # Ok(())
//! # }
//! ```

use serde::{Deserialize, Serialize};

use crate::ai_image::ImageMetadata;
use crate::common::{Headers, ResultWebhook};
use crate::{de, validate, Result};

/// Metadata read from the video file, when available. The same fields as for images.
pub type VideoMetadata = ImageMetadata;

/// The AI Video 1 Pro model.
pub const AI_VIDEO_1_PRO: &str = "ai-video-1-pro";

/// The file extensions the API accepts.
const EXTENSIONS: [&str; 11] = [
    ".mp4", ".avi", ".mov", ".mkv", ".webm", ".flv", ".wmv", ".mpg", ".m4v", ".3gp", ".mxf",
];

/// A video to check, for
/// [`Client::submit_ai_video_detection`](crate::Client::submit_ai_video_detection). Copyleaks
/// downloads the video from its URL.
///
/// The video must be 2 seconds to 1 hour long, at most 512 MiB, at least 360×360 px and at
/// least 16 frames per second. A video that breaks these rules is reported to the webhook as an
/// error.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct AiVideoDetectionSubmission {
    /// The public URL of the video.
    pub url: String,
    /// Headers Copyleaks sends when it fetches the video from `url`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<Headers>,
    /// The HTTP method Copyleaks uses to fetch the video: `GET`, `POST` or `PUT`.
    /// Server default: `GET`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verb: Option<String>,
    /// The file name with its extension: `.mp4`, `.avi`, `.mov`, `.mkv`, `.webm`, `.flv`,
    /// `.wmv`, `.mpg`, `.m4v`, `.3gp` or `.mxf`. From 4 to 254 characters.
    pub filename: String,
    /// The detection model, for example [`AI_VIDEO_1_PRO`].
    pub model: String,
    /// Where Copyleaks posts the result.
    pub webhooks: ResultWebhook,
    /// Sandbox mode: free mock results, for integration testing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<bool>,
    /// A custom string returned with the result. At most 511 characters.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub developer_payload: Option<String>,
}

impl AiVideoDetectionSubmission {
    /// Creates a submission of the video at `url`, with the file name, the model and the URL
    /// that receives the result.
    pub fn new(
        url: impl Into<String>,
        filename: impl Into<String>,
        model: impl Into<String>,
        webhook_url: impl Into<String>,
    ) -> Self {
        Self {
            url: url.into(),
            filename: filename.into(),
            model: model.into(),
            webhooks: ResultWebhook::new(webhook_url),
            ..Self::default()
        }
    }

    /// Replaces the webhook, for example to add headers.
    #[must_use]
    pub fn webhooks(mut self, webhooks: ResultWebhook) -> Self {
        self.webhooks = webhooks;
        self
    }

    pub(crate) fn validate(&self) -> Result<()> {
        validate::required(&self.url, "url")?;
        validate::required(&self.model, "model")?;
        validate::length(&self.filename, "filename", 4, 254)?;
        validate::extension(&self.filename, &EXTENSIONS)?;
        if let Some(verb) = &self.verb {
            validate::verb(verb, "verb")?;
        }
        validate::required(&self.webhooks.url, "webhooks.url")?;
        if let Some(payload) = &self.developer_payload {
            validate::length(payload, "developerPayload", 0, 511)?;
        }
        Ok(())
    }
}

setters!(AiVideoDetectionSubmission {
    headers: Headers,
    verb: String,
    sandbox: bool,
    developer_payload: String,
});

/// The result of AI Video Detection, posted to the webhook of an
/// [`AiVideoDetectionSubmission`]. Positions and lengths are in milliseconds. Missing fields are
/// `None`, and a missing or `null` list is empty.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct AiVideoDetectionResponse {
    /// The version of the detection model.
    pub model: Option<String>,
    /// The AI-generated segments of the audio track.
    pub audio_result: Option<VideoTrackResult>,
    /// The AI-generated segments of the visual track.
    pub visual_result: Option<VideoTrackResult>,
    /// The AI ratios of the video.
    pub summary: Option<AiVideoSummary>,
    /// Whether the video was found to contain AI-generated content. Use it rather than reading
    /// the ratios yourself.
    pub is_ai_detected: Option<bool>,
    /// Information about the video.
    pub video_info: Option<VideoInfo>,
    /// General information about the scan.
    pub scanned_video: Option<ScannedVideo>,
    /// The developer payload you sent with the submission.
    pub developer_payload: Option<String>,
}

/// The AI-generated segments of a track, as parallel lists in milliseconds: segment `i` starts
/// at `starts[i]` and is `lengths[i]` long.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct VideoTrackResult {
    /// Start positions of the AI-generated segments.
    #[serde(default, deserialize_with = "de::null_as_default")]
    pub starts: Vec<i64>,
    /// Lengths of the AI-generated segments.
    #[serde(default, deserialize_with = "de::null_as_default")]
    pub lengths: Vec<i64>,
    /// Ranges of the track that were not scored, and are not counted in the ratios.
    pub exclude: Option<VideoRanges>,
}

/// Ranges of a track, as parallel lists in milliseconds.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct VideoRanges {
    /// Start positions of the ranges.
    #[serde(default, deserialize_with = "de::null_as_default")]
    pub starts: Vec<i64>,
    /// Lengths of the ranges.
    #[serde(default, deserialize_with = "de::null_as_default")]
    pub lengths: Vec<i64>,
}

/// The AI ratios of a video, each from 0 to 1, from the scored segments only.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AiVideoSummary {
    /// AI-generated audio duration over the audible duration.
    #[serde(rename = "audioAIRatio")]
    pub audio_ai_ratio: Option<f64>,
    /// AI-generated visual duration over the visible duration.
    #[serde(rename = "visualAIRatio")]
    pub visual_ai_ratio: Option<f64>,
    /// The combined AI ratio of the audio and visual tracks, over the whole video.
    #[serde(rename = "overallAIRatio")]
    pub overall_ai_ratio: Option<f64>,
}

/// Information about the analyzed video.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct VideoInfo {
    /// The duration of the video, in seconds.
    pub duration: Option<f64>,
    /// Metadata read from the video file, when available.
    pub metadata: Option<VideoMetadata>,
}

/// General information about a video scan.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct ScannedVideo {
    /// The scan id you provided.
    pub scan_id: Option<String>,
    /// The number of credits spent on the scan.
    pub actual_credits: Option<i64>,
    /// The number of credits the scan was expected to cost.
    pub expected_credits: Option<i64>,
    /// When the scan was created, as an ISO 8601 time (UTC).
    pub creation_time: Option<String>,
}
