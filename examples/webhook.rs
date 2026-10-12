//! Parse a completed webhook and read its AI Text Detection result.
//! Reads the webhook body from a file, as your web framework would hand it to you.
//!
//! ```sh
//! cargo run --example webhook -- tests/fixtures/completed_webhook.json
//! ```

use copyleaks::webhooks::{self, CompletedWebhook};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("pass the path of a completed webhook JSON body")?;
    let body = std::fs::read(path)?;

    let completed: CompletedWebhook = webhooks::parse(&body)?;
    if let Some(score) = completed.results.as_ref().and_then(|results| results.score) {
        println!("plagiarism score: {:?}", score.aggregated_score);
    }

    // `None` when the scan has no `suspected-ai-text` alert.
    match completed.ai_detection_result() {
        Ok(Some(ai)) => {
            let summary = ai.summary.unwrap_or_default();
            println!("AI: {:?}, human: {:?}", summary.ai, summary.human);
        }
        Ok(None) => println!("no AI Text Detection result in this webhook"),
        // The alert's additionalData is not valid JSON. Still acknowledge the webhook.
        Err(error) => println!("could not read the AI alert: {error}"),
    }
    Ok(())
}
