//! Grammar Checker in sandbox mode.
//!
//! ```sh
//! COPYLEAKS_EMAIL=you@example.com COPYLEAKS_API_KEY=your-api-key cargo run --example grammar_checker
//! ```

use copyleaks::grammar::{GrammarCheckRequest, ScoreWeights};
use copyleaks::Client;

const TEXT: &str = "Lions are the only cat that live in groups, called pride. A prides typically consists of a few adult males, several feales, and their offspring.";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let email = std::env::var("COPYLEAKS_EMAIL")?;
    let key = std::env::var("COPYLEAKS_API_KEY")?;

    let client = Client::new()?;
    let token = client.login(&email, &key).await?;

    let weights = ScoreWeights::default()
        .grammar_score_weight(0.2)
        .mechanics_score_weight(0.3)
        .sentence_structure_score_weight(0.5)
        .word_choice_score_weight(0.4);
    let request = GrammarCheckRequest::new(TEXT)
        .sandbox(true)
        .language("en")
        .score(weights);
    let response = client
        .grammar_check(&token, "example-grammar-1", &request)
        .await?;

    let overall = response
        .score
        .as_ref()
        .and_then(|score| score.corrections.as_ref())
        .and_then(|corrections| corrections.overall_score);
    println!("overall score: {overall:?}");

    // Correction ids, with their names from the public correction types list.
    let types = client.grammar_correction_types("en").await?;
    if let Some(chars) = response
        .corrections
        .as_ref()
        .and_then(|corrections| corrections.text.as_ref())
        .and_then(|text| text.chars.as_ref())
    {
        for (((kind, start), length), replacement) in chars
            .types
            .iter()
            .zip(&chars.starts)
            .zip(&chars.lengths)
            .zip(&chars.operation_texts)
        {
            let name = types
                .correction_types
                .iter()
                .find(|correction| correction.id == Some(*kind))
                .and_then(|correction| correction.title.as_deref())
                .unwrap_or("unknown");
            println!("{name}: replace {length} characters at {start} with {replacement:?}");
        }
    }
    Ok(())
}
