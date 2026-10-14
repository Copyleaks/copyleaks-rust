//! Image Plagiarism Detector in sandbox mode: search the web for copies of an image.
//!
//! ```sh
//! COPYLEAKS_EMAIL=you@example.com COPYLEAKS_API_KEY=your-api-key \
//!     cargo run --example image_plagiarism -- path/to/photo.jpg
//! ```

use copyleaks::image_plagiarism::ImagePlagiarismRequest;
use copyleaks::Client;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let email = std::env::var("COPYLEAKS_EMAIL")?;
    let key = std::env::var("COPYLEAKS_API_KEY")?;
    let path = std::env::args()
        .nth(1)
        .ok_or("pass the path of a JPEG, PNG, GIF, BMP, WebP or ICO image, under 20 MB")?;

    let client = Client::new()?;
    let token = client.login(&email, &key).await?;

    let image = std::fs::read(&path)?;
    let filename = std::path::Path::new(&path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("image.jpg")
        .to_owned();
    let request = ImagePlagiarismRequest::new(image, filename).sandbox(true);
    let response = client
        .image_plagiarism_detection(&token, "example-image-plagiarism-1", &request)
        .await?;

    if let Some(matches) = &response.matches {
        if let Some(score) = matches.score {
            println!(
                "{:?} matches: {:?} full, {:?} partial",
                score.total_matches, score.full_matches, score.partial_matches
            );
        }
        for found in &matches.internet {
            println!("{:?} ({:?})", found.url, found.kind());
            for page in &found.web_pages {
                println!("  on {:?}", page.url);
            }
        }
    }
    Ok(())
}
