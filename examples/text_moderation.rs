//! Text Moderation in sandbox mode, with every label.
//!
//! ```sh
//! COPYLEAKS_EMAIL=you@example.com COPYLEAKS_API_KEY=your-api-key cargo run --example text_moderation
//! ```

use copyleaks::moderation::{labels, TextModerationRequest, LANGUAGE_ENGLISH};
use copyleaks::Client;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let email = std::env::var("COPYLEAKS_EMAIL")?;
    let key = std::env::var("COPYLEAKS_API_KEY")?;

    let client = Client::new()?;
    let token = client.login(&email, &key).await?;

    let request = TextModerationRequest::new("This is some text to scan.", labels::ALL)
        .sandbox(true)
        .language(LANGUAGE_ENGLISH);
    let response = client
        .text_moderation(&token, "example-moderation-1", &request)
        .await?;

    if let Some(chars) = response
        .moderations
        .as_ref()
        .and_then(|moderations| moderations.text.as_ref())
        .and_then(|text| text.chars.as_ref())
    {
        for ((label, start), length) in chars.labels.iter().zip(&chars.starts).zip(&chars.lengths) {
            let id = response
                .legend
                .iter()
                .find(|entry| entry.index == Some(*label))
                .and_then(|entry| entry.id.as_deref())
                .unwrap_or("unknown");
            println!("{id}: {length} characters at {start}");
        }
    }
    Ok(())
}
