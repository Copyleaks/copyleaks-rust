//! Grammar Checker: grammar, spelling and sentence corrections, with writing scores.

use serde::{Deserialize, Serialize};

use crate::{de, validate, Result, ScannedDocument};

/// A text to check, for [`Client::grammar_check`](crate::Client::grammar_check).
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GrammarCheckRequest {
    /// The text. From 1 to 25,000 characters.
    pub text: String,
    /// Sandbox mode: free mock results, for integration testing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<bool>,
    /// The language code of the text, for example `en`. Detected automatically when not set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// The weight of each correction category in the overall score.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<ScoreWeights>,
}

impl GrammarCheckRequest {
    /// Creates a request for this text.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            ..Self::default()
        }
    }

    pub(crate) fn validate(&self) -> Result<()> {
        validate::length(&self.text, "text", 1, 25_000)?;
        if let Some(score) = &self.score {
            score.validate()?;
        }
        Ok(())
    }
}

setters!(GrammarCheckRequest {
    sandbox: bool,
    language: String,
    score: ScoreWeights,
});

/// The weight of each correction category in the overall score, each from 0.0 to 1.0.
/// A category without a weight uses the default of 1.0.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScoreWeights {
    /// The weight of grammar corrections.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grammar_score_weight: Option<f64>,
    /// The weight of mechanics corrections.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mechanics_score_weight: Option<f64>,
    /// The weight of sentence structure corrections.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sentence_structure_score_weight: Option<f64>,
    /// The weight of word choice corrections.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub word_choice_score_weight: Option<f64>,
}

impl ScoreWeights {
    fn validate(&self) -> Result<()> {
        let weights = [
            (self.grammar_score_weight, "score.grammarScoreWeight"),
            (self.mechanics_score_weight, "score.mechanicsScoreWeight"),
            (
                self.sentence_structure_score_weight,
                "score.sentenceStructureScoreWeight",
            ),
            (self.word_choice_score_weight, "score.wordChoiceScoreWeight"),
        ];
        for (weight, name) in weights {
            if let Some(weight) = weight {
                validate::weight(weight, name)?;
            }
        }
        Ok(())
    }
}

setters!(ScoreWeights {
    grammar_score_weight: f64,
    mechanics_score_weight: f64,
    sentence_structure_score_weight: f64,
    word_choice_score_weight: f64,
});

/// The result of the Grammar Checker. Unknown fields are ignored and missing fields are `None`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GrammarCheckResponse {
    /// Writing scores, readability and text statistics.
    pub score: Option<GrammarScore>,
    /// The suggested corrections.
    pub corrections: Option<GrammarCorrections>,
    /// General information about the scanned document.
    pub scanned_document: Option<ScannedDocument>,
    /// `"sandbox"` for sandbox scans. Not sent for regular scans.
    pub scan_type: Option<String>,
}

/// Writing scores, readability and text statistics.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GrammarScore {
    /// Correction counts, scores and weights per category, and the overall score.
    pub corrections: Option<ScoreCorrections>,
    /// Readability of the text.
    pub readability: Option<Readability>,
    /// Statistics of the text.
    pub statistics: Option<TextStatistics>,
}

/// Correction counts, scores (0 to 100) and weights per category, and the overall score.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScoreCorrections {
    /// Number of grammar corrections.
    pub grammar_corrections_count: Option<i64>,
    /// Grammar score.
    pub grammar_corrections_score: Option<i64>,
    /// Weight of the grammar score in the overall score.
    pub grammar_score_weight: Option<f64>,
    /// Number of mechanics corrections.
    pub mechanics_corrections_count: Option<i64>,
    /// Mechanics score.
    pub mechanics_corrections_score: Option<i64>,
    /// Weight of the mechanics score in the overall score.
    pub mechanics_score_weight: Option<f64>,
    /// Number of sentence structure corrections.
    pub sentence_structure_corrections_count: Option<i64>,
    /// Sentence structure score.
    pub sentence_structure_corrections_score: Option<i64>,
    /// Weight of the sentence structure score in the overall score.
    pub sentence_structure_score_weight: Option<f64>,
    /// Number of word choice corrections.
    pub word_choice_corrections_count: Option<i64>,
    /// Word choice score.
    pub word_choice_corrections_score: Option<i64>,
    /// Weight of the word choice score in the overall score.
    pub word_choice_score_weight: Option<f64>,
    /// The overall writing score.
    pub overall_score: Option<i64>,
}

/// Readability of the text.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Readability {
    /// Readability score.
    pub score: Option<i64>,
    /// Readability level.
    pub readability_level: Option<i64>,
    /// Readability level as text, for example `10th to 12th Grader`.
    pub readability_level_text: Option<String>,
    /// Description of the readability level, for example `Fairly difficult to read`.
    pub readability_level_description: Option<String>,
}

/// Statistics of the text.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextStatistics {
    /// Number of sentences.
    pub sentence_count: Option<i64>,
    /// Average sentence length, in words.
    pub average_sentence_length: Option<f64>,
    /// Average word length, in characters.
    pub average_word_length: Option<f64>,
    /// Estimated reading time, in seconds.
    pub reading_time_seconds: Option<f64>,
    /// Estimated speaking time, in seconds.
    pub speaking_time_seconds: Option<f64>,
}

/// The suggested corrections.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrammarCorrections {
    /// Corrections in the plain text.
    pub text: Option<CorrectionsText>,
}

/// Corrections in the plain text.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CorrectionsText {
    /// Character positions of the corrections.
    pub chars: Option<CorrectionRanges>,
}

/// Corrections as parallel lists: correction `i` replaces `lengths[i]` characters from
/// `starts[i]` with `operation_texts[i]`, and its type is `types[i]`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CorrectionRanges {
    /// Correction type ids. See
    /// [`Client::grammar_correction_types`](crate::Client::grammar_correction_types).
    #[serde(default, deserialize_with = "de::null_as_default")]
    pub types: Vec<i32>,
    /// Start positions of the corrected segments.
    #[serde(default, deserialize_with = "de::null_as_default")]
    pub starts: Vec<i64>,
    /// Lengths of the corrected segments.
    #[serde(default, deserialize_with = "de::null_as_default")]
    pub lengths: Vec<i64>,
    /// The suggested replacement text of each segment.
    #[serde(default, deserialize_with = "de::null_as_default")]
    pub operation_texts: Vec<String>,
}

/// The correction types of the Grammar Checker, from
/// [`Client::grammar_correction_types`](crate::Client::grammar_correction_types).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CorrectionTypes {
    /// The correction types.
    #[serde(default, deserialize_with = "de::null_as_default")]
    pub correction_types: Vec<CorrectionType>,
}

/// A correction type.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CorrectionType {
    /// The id used in [`CorrectionRanges::types`].
    pub id: Option<i32>,
    /// The correction category.
    pub category: Option<i32>,
    /// A short name, for example `Subject Verb Disagreement`.
    pub title: Option<String>,
    /// What the correction type means.
    pub description: Option<String>,
    /// The message shown for a correction of this type.
    pub message: Option<String>,
}
