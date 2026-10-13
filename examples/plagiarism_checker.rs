//! Plagiarism Checker: submit a text file in sandbox mode, with AI Text Detection turned on.
//! The result arrives at your status webhook; see the `webhook` example to parse it.
//!
//! ```sh
//! COPYLEAKS_EMAIL=you@example.com COPYLEAKS_API_KEY=your-api-key \
//! WEBHOOK_URL='https://your.server/webhook/{STATUS}' cargo run --example plagiarism_checker
//! ```

use copyleaks::plagiarism::{AiGeneratedText, FileSubmission, ScanProperties};
use copyleaks::Client;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let email = std::env::var("COPYLEAKS_EMAIL")?;
    let key = std::env::var("COPYLEAKS_API_KEY")?;
    let webhook = std::env::var("WEBHOOK_URL")?;

    let client = Client::new()?;
    let token = client.login(&email, &key).await?;

    let properties = ScanProperties::new(webhook)
        .sandbox(true)
        .ai_generated_text(AiGeneratedText::default().detect(true));
    let submission = FileSubmission::from_bytes(
        "Hello world, this is a test of the Copyleaks Plagiarism Checker.",
        "hello.txt",
        properties,
    );
    client
        .submit_file(&token, "example-plagiarism-1", &submission)
        .await?;

    println!("Scan submitted. Copyleaks calls your status webhook when it completes.");
    Ok(())
}
