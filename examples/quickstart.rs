//! Log in and run AI Text Detection in sandbox mode.
//!
//! ```sh
//! COPYLEAKS_EMAIL=you@example.com COPYLEAKS_API_KEY=your-api-key cargo run --example quickstart
//! ```

use copyleaks::ai_text::AiTextDetectionRequest;
use copyleaks::Client;

const TEXT: &str = "Lions are social animals, living in groups called prides, typically consisting of several females, their offspring, and a few males. Female lions are the primary hunters, working together to catch prey. Lions are known for their strength, teamwork, and complex social structures.";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let email = std::env::var("COPYLEAKS_EMAIL")?;
    let key = std::env::var("COPYLEAKS_API_KEY")?;

    let client = Client::new()?;
    let token = client.login(&email, &key).await?;

    // Sandbox mode returns mock results and uses no credits.
    let request = AiTextDetectionRequest::new(TEXT).sandbox(true);
    let response = client
        .ai_text_detection(&token, "quickstart-ai-text-1", &request)
        .await?;

    if let Some(summary) = response.summary {
        println!("AI: {:?}, human: {:?}", summary.ai, summary.human);
    }
    for section in &response.results {
        // 1 = human-written, 2 = AI-generated
        println!("section classification: {:?}", section.classification);
    }
    Ok(())
}
