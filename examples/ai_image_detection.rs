//! AI Image Detection in sandbox mode, with a multipart upload and a heatmap mask.
//!
//! ```sh
//! COPYLEAKS_EMAIL=you@example.com COPYLEAKS_API_KEY=your-api-key \
//!     cargo run --example ai_image_detection -- path/to/image.png
//! ```

use copyleaks::ai_image::{AiImageDetectionRequest, MaskType, AI_IMAGE_1_ULTRA};
use copyleaks::Client;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let email = std::env::var("COPYLEAKS_EMAIL")?;
    let key = std::env::var("COPYLEAKS_API_KEY")?;
    let path = std::env::args().nth(1).ok_or(
        "pass the path of a PNG, JPEG, BMP, WebP, HEIC or AVIF image, at least 512x512 px",
    )?;

    let client = Client::new()?;
    let token = client.login(&email, &key).await?;

    let image = std::fs::read(&path)?;
    let filename = std::path::Path::new(&path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("image.png")
        .to_owned();
    let request = AiImageDetectionRequest::from_bytes(image, filename, AI_IMAGE_1_ULTRA)
        .mask_type(MaskType::Heatmap)
        .sandbox(true);
    // The image is uploaded as multipart/form-data, without base64.
    let response = client
        .ai_image_detection_multipart(&token, "example-ai-image-1", &request)
        .await?;

    if let Some(summary) = response.summary {
        println!("AI: {:?}, human: {:?}", summary.ai, summary.human);
    }
    println!("flagged as AI-generated: {:?}", response.is_ai_detected);
    if let Some(mask) = &response.result {
        println!(
            "{} AI regions, confidence bands {:?}",
            mask.starts.len(),
            mask.labels
        );
    }
    Ok(())
}
