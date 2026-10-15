//! Webhook payload parsing, with the examples from the API docs, and the AI alert extraction of
//! the completed webhook.

// `Pdf::version` is deprecated by the API, and is still serialized.
#![allow(deprecated)]

mod common;

use common::fixture;
use copyleaks::grammar::ScoreWeights;
use copyleaks::plagiarism::{
    AiGeneratedText, AiGeneratedTextExplain, AiSourceMatch, Assignment, Author, CopyleaksDb,
    Course, CrossLanguages, CustomMetadata, DomainsMode, Exclude, ExcludeCode, Filters, Indexing,
    IndexingRepository, Institution, InternalAiSourceMatch, MaskingPolicy, Overview, Pdf,
    PdfColors, PdfVersion, References, ScanMethodAlgorithm, ScanProperties, Scanning,
    ScanningExclude, ScanningInclude, SearchRepository, SensitiveDataProtection, StartResponse,
    SubmitAction, SubmitWebhooks, WritingFeedback,
};
use copyleaks::webhooks::{
    self, alert_codes, Alert, CompletedWebhook, CreditsCheckedWebhook, ErrorWebhook,
    ExportCompletedWebhook, IndexedWebhook, NewResultWebhook,
};
use copyleaks::Error;
use serde_json::{json, Value};

fn parse<T: serde::de::DeserializeOwned>(name: &str) -> T {
    let body = serde_json::to_vec(&fixture(name)).expect("fixture bytes");
    webhooks::parse(&body).expect("webhook parses")
}

fn alert(code: &str, additional_data: Option<&str>) -> Alert {
    let mut alert = Alert::default();
    alert.code = Some(code.to_owned());
    alert.additional_data = additional_data.map(str::to_owned);
    alert
}

fn ai_alert(additional_data: Option<&str>) -> Alert {
    alert(alert_codes::SUSPECTED_AI_TEXT, additional_data)
}

#[test]
fn completed_webhook() {
    let completed: CompletedWebhook = parse("completed_webhook.json");

    assert_eq!(completed.status, Some(0));
    assert_eq!(
        completed.developer_payload.as_deref(),
        Some("Custom developer payload")
    );
    let document = completed
        .scanned_document
        .as_ref()
        .expect("scanned document");
    assert_eq!(document.credits, Some(0));
    let metadata = document.metadata.as_ref().expect("metadata");
    assert_eq!(metadata.final_url.as_deref(), Some("string"));
    assert_eq!(metadata.last_modification_date.as_deref(), Some("string"));

    let results = completed.results.as_ref().expect("results");
    assert_eq!(results.internet.len(), 1);
    assert_eq!(results.internet[0].url.as_deref(), Some("string"));
    assert_eq!(results.internet[0].matched_words, Some(0));
    assert_eq!(results.database[0].scan_id.as_deref(), Some("string"));
    assert_eq!(results.batch.len(), 1);
    let repository = &results.repositories[0];
    assert_eq!(repository.repository_id.as_deref(), Some("string"));
    assert_eq!(
        repository
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.submitted_by.as_deref()),
        Some("string")
    );
    let score = results.score.expect("score");
    assert_eq!(score.aggregated_score, Some(0.0));
    assert_eq!(score.identical_words, Some(0));

    let alerts = &completed
        .notifications
        .as_ref()
        .expect("notifications")
        .alerts;
    assert_eq!(alerts.len(), 1);
    assert_eq!(alerts[0].category, Some(2));
    assert_eq!(alerts[0].severity, Some(4));
    assert_eq!(
        alerts[0].help_link.as_deref(),
        Some("https://docs.copyleaks.com")
    );
}

#[test]
fn completed_webhook_ai_detection_result() {
    let completed: CompletedWebhook = parse("completed_webhook.json");

    let alert = completed.ai_detection_alert().expect("AI alert");
    assert_eq!(alert.code.as_deref(), Some("suspected-ai-text"));

    let result = completed
        .ai_detection_result()
        .expect("valid additional data")
        .expect("AI result");
    assert_eq!(result.model_version.as_deref(), Some("v11.0"));
    assert_eq!(result.summary.and_then(|summary| summary.ai), Some(1.0));
    assert_eq!(result.results[0].classification, Some(2));
    assert_eq!(
        result
            .scanned_document
            .and_then(|document| document.total_words),
        Some(73)
    );
}

#[test]
fn completed_webhook_without_ai_alert() {
    let completed: CompletedWebhook = webhooks::parse(br#"{"status": 0}"#).expect("parses");
    assert!(completed.ai_detection_alert().is_none());
    assert!(completed.ai_detection_result().expect("no error").is_none());

    let other_alert = json!({
        "notifications": { "alerts": [null, { "code": "suspected-white-text", "additionalData": "{}" }] }
    });
    let completed: CompletedWebhook = serde_json::from_value(other_alert).expect("parses");
    assert_eq!(
        completed
            .notifications
            .as_ref()
            .expect("notifications")
            .alerts
            .len(),
        1,
        "null alerts are skipped"
    );
    assert!(completed.ai_detection_alert().is_none());
    assert!(completed.ai_detection_result().expect("no error").is_none());
}

#[test]
fn first_ai_alert_wins() {
    let body = json!({
        "notifications": { "alerts": [
            { "code": "suspected-white-text" },
            { "code": "suspected-ai-text", "additionalData": "{\"summary\": {\"ai\": 0.25}}" },
            { "code": "suspected-ai-text", "additionalData": "{\"summary\": {\"ai\": 0.75}}" }
        ] }
    });
    let completed: CompletedWebhook = serde_json::from_value(body).expect("parses");
    let result = completed
        .ai_detection_result()
        .expect("valid")
        .expect("result");
    assert_eq!(result.summary.and_then(|summary| summary.ai), Some(0.25));
}

#[test]
fn alert_additional_data_edge_cases() {
    // Trailing NUL and ASCII whitespace are removed before parsing.
    let padded = ai_alert(Some("{\"summary\": {\"ai\": 0.5}}\0\0\r\n\t \u{b}\u{c}"));
    let result = padded
        .ai_detection_result()
        .expect("valid")
        .expect("result");
    assert_eq!(result.summary.and_then(|summary| summary.ai), Some(0.5));
    assert!(padded
        .additional_data
        .as_deref()
        .expect("kept")
        .ends_with('\u{c}'));

    // Missing, empty, or only padding: no result.
    for data in [None, Some(""), Some("\0 \n")] {
        assert!(
            ai_alert(data)
                .ai_detection_result()
                .expect("no error")
                .is_none(),
            "{data:?}"
        );
    }

    // Valid JSON that is not an object: no result.
    for data in ["null", "[1, 2]", "\"text\"", "5"] {
        assert!(
            ai_alert(Some(data))
                .ai_detection_result()
                .expect("no error")
                .is_none(),
            "{data}"
        );
    }

    // Malformed JSON, or a field with a type the result cannot accept: a JSON error.
    for data in ["{not json", "{\"summary\": 5}", "{\"results\": \"x\"}"] {
        let error = ai_alert(Some(data)).ai_detection_result().expect_err(data);
        assert!(matches!(error, Error::Json(_)), "{data}: {error:?}");
    }

    // Another alert code is never decoded, even with valid AI data.
    let other = alert("suspected-white-text", Some("{\"summary\": {\"ai\": 1}}"));
    assert!(other.ai_detection_result().expect("no error").is_none());
}

#[test]
fn sandbox_ai_alert_with_pascal_case_keys() {
    // Sandbox scans send `Starts`/`Lengths` and `Human`/`Ai`, and a numeric model version.
    let data = json!({
        "modelVersion": 11,
        "results": [{
            "classification": 2,
            "matches": [{ "text": { "chars": { "Starts": [0], "Lengths": [10] }, "words": null } }]
        }],
        "summary": { "Human": 0.1, "Ai": 0.9 },
        "scanType": "sandbox"
    });
    let alert = ai_alert(Some(&data.to_string()));
    let result = alert.ai_detection_result().expect("valid").expect("result");
    assert_eq!(result.model_version.as_deref(), Some("11"));
    let summary = result.summary.expect("summary");
    assert_eq!((summary.human, summary.ai), (Some(0.1), Some(0.9)));
    let chars = result.results[0].matches[0]
        .text
        .as_ref()
        .and_then(|text| text.chars.as_ref())
        .expect("chars");
    assert_eq!(chars.starts, [0]);
    assert_eq!(chars.lengths, [10]);
    assert_eq!(result.scan_type.as_deref(), Some("sandbox"));
}

#[test]
fn null_lists_are_empty() {
    let data = json!({ "results": null, "summary": { "ai": 1 }, "explain": { "patterns": {
        "statistics": { "aiCount": null, "humanCount": null, "proportion": null, "source": null },
        "text": { "chars": { "starts": null, "lengths": null } }
    } } });
    let result = ai_alert(Some(&data.to_string()))
        .ai_detection_result()
        .expect("valid")
        .expect("result");
    assert!(result.results.is_empty());
    let patterns = result
        .explain
        .and_then(|explain| explain.patterns)
        .expect("patterns");
    assert!(patterns.statistics.expect("statistics").ai_count.is_empty());
    assert!(patterns
        .text
        .and_then(|text| text.chars)
        .expect("chars")
        .starts
        .is_empty());
}

#[test]
fn error_webhook() {
    let error: ErrorWebhook = parse("error_webhook.json");
    assert_eq!(error.status, Some(1));
    assert_eq!(
        error.developer_payload.as_deref(),
        Some("Custom developer payload")
    );
    let details = error.error.as_ref().expect("error");
    assert_eq!(details.code, Some(1));
    assert_eq!(details.error_type.as_deref(), Some("invalid_request_error"));
    assert_eq!(details.id.as_deref(), Some("missing_parameter"));
    assert!(details
        .message
        .as_deref()
        .expect("message")
        .starts_with("Bad request"));
}

#[test]
fn credits_checked_webhook() {
    let checked: CreditsCheckedWebhook = parse("credits_checked_webhook.json");
    assert_eq!(checked.status, Some(2));
    assert_eq!(checked.credits, Some(1));
    assert_eq!(
        checked.developer_payload.as_deref(),
        Some("Custom developer payload")
    );
    assert!(checked.scanned_document.is_none());
}

#[test]
fn indexed_webhook() {
    let indexed: IndexedWebhook = parse("indexed_webhook.json");
    assert_eq!(indexed.status, Some(3));
    assert_eq!(
        indexed.developer_payload.as_deref(),
        Some("Custom developer payload")
    );
}

#[test]
fn export_completed_webhook() {
    let export: ExportCompletedWebhook = parse("export_completed_webhook.json");
    assert!(export.completed);
    assert_eq!(
        export.developer_payload.as_deref(),
        Some("This is my payload")
    );
    assert_eq!(export.tasks.len(), 3);
    assert!(export.tasks.iter().all(|task| task.is_healthy));
    assert_eq!(export.tasks[1].http_status_code, Some(200));
    assert_eq!(
        export.tasks[1].endpoint.as_deref(),
        Some("https://yourserver.com/export/export-id/pdf-report")
    );
}

#[test]
fn new_result_webhook_docs_example() {
    // The docs show `score` as a number.
    let new_result: NewResultWebhook = parse("new_result_webhook.json");
    assert_eq!(new_result.developer_payload.as_deref(), Some("string"));
    assert_eq!(
        new_result.score.and_then(|score| score.aggregated_score),
        Some(0.0)
    );
    assert_eq!(new_result.internet.len(), 1);
    assert_eq!(new_result.database.len(), 1);
    assert_eq!(new_result.batch.len(), 1);
    assert_eq!(
        new_result.repositories[0].repository_id.as_deref(),
        Some("string")
    );
}

#[test]
fn new_result_webhook_server_score_object() {
    // The server sends `score` as an object.
    let body = json!({ "score": { "aggregatedScore": 12.5 }, "internet": [] });
    let new_result: NewResultWebhook = serde_json::from_value(body).expect("parses");
    assert_eq!(
        new_result.score.and_then(|score| score.aggregated_score),
        Some(12.5)
    );
    assert!(new_result.internet.is_empty());
}

#[test]
fn malformed_webhook_is_a_json_error() {
    let error = webhooks::parse::<CompletedWebhook>(b"not json").expect_err("bad body");
    assert!(matches!(error, Error::Json(_)), "{error:?}");
}

#[test]
fn start_response_accepts_strings_and_objects() {
    // The docs show plain ids; the server sends `{ "id": ... }` objects, and may leave nulls.
    let docs: StartResponse =
        serde_json::from_value(json!({ "success": ["scan-1"], "failed": [] })).expect("docs");
    assert_eq!(docs.success, ["scan-1"]);
    let server: StartResponse = serde_json::from_value(
        json!({ "success": [{ "id": "scan-1" }, null], "failed": [null, { "id": "scan-2" }] }),
    )
    .expect("server");
    assert_eq!(server.success, ["scan-1"]);
    assert_eq!(server.failed, ["scan-2"]);
}

#[test]
fn scan_properties_use_the_api_field_names() {
    let properties = ScanProperties::new("https://example.com/webhook/{STATUS}")
        .webhooks(
            SubmitWebhooks::new("https://example.com/webhook/{STATUS}")
                .new_result("https://example.com/webhook/new-result")
                .status_headers(vec![("key".to_owned(), "value".to_owned())])
                .new_result_headers(vec![("key2".to_owned(), "value2".to_owned())]),
        )
        .action(SubmitAction::CheckCredits)
        .include_html(true)
        .developer_payload("payload")
        .sandbox(true)
        .expiration(480)
        .author(Author::new("author-1"))
        .filters(
            Filters::default()
                .identical_enabled(true)
                .minor_changes_enabled(true)
                .related_meaning_enabled(false)
                .min_copied_words(5)
                .safe_search(true)
                .domains(vec!["example.com".to_owned()])
                .domains_mode(DomainsMode::Include)
                .allow_same_domain(false),
        )
        .scanning(
            Scanning::default()
                .internet(true)
                .exclude(
                    ScanningExclude::default()
                        .id_pattern("abc*")
                        .backlinks_domains(vec!["spam.example".to_owned()])
                        .text(vec!["lorem ipsum".to_owned()]),
                )
                .include(ScanningInclude::default().id_pattern("class-*"))
                .repositories(vec![SearchRepository::new("repo-1")
                    .include_my_submissions(true)
                    .include_others_submissions(false)])
                .copyleaks_db(
                    CopyleaksDb::default()
                        .include_my_submissions(true)
                        .include_others_submissions(true),
                )
                .cross_languages(CrossLanguages::new(["fr", "es"])),
        )
        .indexing(
            Indexing::default()
                .repositories(vec![IndexingRepository::new("repo-1")
                    .masking_policy(MaskingPolicy::MaskOtherUsersFiles)])
                .copyleaks_db(false),
        )
        .exclude(
            Exclude::default()
                .quotes(true)
                .citations(true)
                .references(true)
                .table_of_contents(true)
                .titles(true)
                .html_template(true)
                .document_template_ids(vec!["template-1".to_owned()])
                .code(ExcludeCode::default().comments(true)),
        )
        .pdf(
            Pdf::default()
                .create(true)
                .title("Report")
                .large_logo("bG9nbw==")
                .rtl(false)
                .version(PdfVersion::V2)
                .report_version("v3")
                .colors(
                    PdfColors::default()
                        .main_strip("#000000")
                        .titles("#040F21")
                        .identical("#FE5758")
                        .minor_changes("#FE8888")
                        .related_meaning("#FDD0A3")
                        .ai_detection("#FE5758")
                        .writing_feedback("#FF9A02"),
                ),
        )
        .display_language("es")
        .sensitivity_level(3)
        .cheat_detection(true)
        .scan_method_algorithm(ScanMethodAlgorithm::MaximumResults)
        .ai_generated_text(
            AiGeneratedText::default()
                .detect(true)
                .sensitivity(1)
                .explain(AiGeneratedTextExplain::default().enable(false)),
        )
        .ai_source_match(AiSourceMatch::default().enable(true))
        .custom_metadata(vec![CustomMetadata::new("course", "cs-101")])
        .scan_time_zone("Europe/Paris")
        .course(Course::new("course-1"))
        .assignment(Assignment::new("assignment-1"))
        .institution(Institution::new("institution-1"))
        .sensitive_data_protection(
            SensitiveDataProtection::default()
                .drivers_license(true)
                .credentials(true)
                .passport(true)
                .network(true)
                .url(true)
                .email_address(true)
                .credit_card(true)
                .phone_number(true),
        )
        .writing_feedback(
            WritingFeedback::default().enable(true).score(
                ScoreWeights::default()
                    .grammar_score_weight(0.1)
                    .mechanics_score_weight(0.2)
                    .sentence_structure_score_weight(0.3)
                    .word_choice_score_weight(0.4),
            ),
        )
        .overview(
            Overview::default()
                .enable(true)
                .ignore_ai_detection(true)
                .ignore_plagiarism_detection(false)
                .ignore_writing_feedback(true)
                .ignore_author_data(false),
        )
        .internal_ai_source_match(InternalAiSourceMatch::default().enable(true))
        .references(References::default().validate(true));

    let expected: Value = json!({
        "webhooks": {
            "status": "https://example.com/webhook/{STATUS}",
            "newResult": "https://example.com/webhook/new-result",
            "statusHeaders": [["key", "value"]],
            "newResultHeaders": [["key2", "value2"]]
        },
        "action": 1,
        "includeHtml": true,
        "developerPayload": "payload",
        "sandbox": true,
        "expiration": 480,
        "author": { "id": "author-1" },
        "filters": {
            "identicalEnabled": true,
            "minorChangesEnabled": true,
            "relatedMeaningEnabled": false,
            "minCopiedWords": 5,
            "safeSearch": true,
            "domains": ["example.com"],
            "domainsMode": 0,
            "allowSameDomain": false
        },
        "scanning": {
            "internet": true,
            "exclude": {
                "idPattern": "abc*",
                "backlinksDomains": ["spam.example"],
                "text": ["lorem ipsum"]
            },
            "include": { "idPattern": "class-*" },
            "repositories": [
                { "id": "repo-1", "includeMySubmissions": true, "includeOthersSubmissions": false }
            ],
            "copyleaksDb": { "includeMySubmissions": true, "includeOthersSubmissions": true },
            "crossLanguages": { "languages": [{ "code": "fr" }, { "code": "es" }] }
        },
        "indexing": {
            "repositories": [{ "id": "repo-1", "maskingPolicy": 1 }],
            "copyleaksDb": false
        },
        "exclude": {
            "quotes": true,
            "citations": true,
            "references": true,
            "tableOfContents": true,
            "titles": true,
            "htmlTemplate": true,
            "documentTemplateIds": ["template-1"],
            "code": { "comments": true }
        },
        "pdf": {
            "create": true,
            "title": "Report",
            "largeLogo": "bG9nbw==",
            "rtl": false,
            "version": 2,
            "reportVersion": "v3",
            "colors": {
                "mainStrip": "#000000",
                "titles": "#040F21",
                "identical": "#FE5758",
                "minorChanges": "#FE8888",
                "relatedMeaning": "#FDD0A3",
                "aiDetection": "#FE5758",
                "writingFeedback": "#FF9A02"
            }
        },
        "displayLanguage": "es",
        "sensitivityLevel": 3,
        "cheatDetection": true,
        "scanMethodAlgorithm": 1,
        "aiGeneratedText": { "detect": true, "sensitivity": 1, "explain": { "enable": false } },
        "aiSourceMatch": { "enable": true },
        "customMetadata": [{ "key": "course", "value": "cs-101" }],
        "scanTimeZone": "Europe/Paris",
        "course": { "id": "course-1" },
        "assignment": { "id": "assignment-1" },
        "institution": { "id": "institution-1" },
        "sensitiveDataProtection": {
            "driversLicense": true,
            "credentials": true,
            "passport": true,
            "network": true,
            "url": true,
            "emailAddress": true,
            "creditCard": true,
            "phoneNumber": true
        },
        "writingFeedback": {
            "enable": true,
            "score": {
                "grammarScoreWeight": 0.1,
                "mechanicsScoreWeight": 0.2,
                "sentenceStructureScoreWeight": 0.3,
                "wordChoiceScoreWeight": 0.4
            }
        },
        "overview": {
            "enable": true,
            "ignoreAIDetection": true,
            "ignorePlagiarismDetection": false,
            "ignoreWritingFeedback": true,
            "ignoreAuthorData": false
        },
        "internalAiSourceMatch": { "enable": true },
        "references": { "validate": true }
    });
    assert_eq!(
        serde_json::to_value(properties).expect("serialize"),
        expected
    );
}

#[test]
fn ai_source_match_display_language_and_report_version_json() {
    let webhook = "https://example.com/webhook/{STATUS}";

    // Set: the exact API field names and values.
    let properties = ScanProperties::new(webhook)
        .pdf(Pdf::default().create(true).report_version("latest"))
        .display_language("de")
        .ai_source_match(AiSourceMatch::default().enable(true));
    assert_eq!(
        serde_json::to_value(properties).expect("serialize"),
        json!({
            "webhooks": { "status": webhook },
            "pdf": { "create": true, "reportVersion": "latest" },
            "displayLanguage": "de",
            "aiSourceMatch": { "enable": true }
        })
    );

    // `enable: false` is sent, not dropped.
    assert_eq!(
        serde_json::to_value(AiSourceMatch::default().enable(false)).expect("serialize"),
        json!({ "enable": false })
    );

    // Unset: left out, so the server defaults apply.
    let properties = ScanProperties::new(webhook).pdf(Pdf::default().create(true));
    assert_eq!(
        serde_json::to_value(properties).expect("serialize"),
        json!({
            "webhooks": { "status": webhook },
            "pdf": { "create": true }
        })
    );
    assert_eq!(
        serde_json::to_value(AiSourceMatch::default()).expect("serialize"),
        json!({})
    );
}

#[test]
fn completed_webhook_new_sections() {
    let completed: CompletedWebhook = parse("completed_webhook_full.json");

    let document = completed
        .scanned_document
        .as_ref()
        .expect("scanned document");
    assert_eq!(document.expected_credits, Some(1));
    assert_eq!(document.detected_language.as_deref(), Some("en"));
    let enabled = document.enabled.expect("enabled features");
    assert_eq!(enabled.plagiarism_detection, Some(true));
    assert_eq!(enabled.pdf_report, Some(true));
    assert_eq!(enabled.ai_detection, Some(false));
    assert_eq!(enabled.internal_ai_source_match, Some(false));
    assert_eq!(enabled.overview, None, "not sent");

    let results = completed.results.as_ref().expect("results");
    assert_eq!(results.internal_ai_data.len(), 1);
    let internal = &results.internal_ai_data[0];
    assert_eq!(internal.id.as_deref(), Some("string"));
    assert_eq!(internal.matched_words, Some(0));
    assert_eq!(internal.identical_words, Some(0));
    assert_eq!(internal.similar_words, Some(0));
    assert_eq!(internal.paraphrased_words, Some(0));
    assert_eq!(internal.total_words, Some(0));
    assert_eq!(
        internal.tags[0].code.as_deref(),
        Some("internal-ai-source-match")
    );

    let summary = completed
        .references_validation
        .as_ref()
        .and_then(|validation| validation.summary)
        .expect("references summary");
    assert_eq!(summary.total, Some(2));
    assert_eq!(summary.academic, Some(1));
    assert_eq!(summary.academic_validated, Some(1));
    assert_eq!(summary.non_academic_validated, Some(0));

    let feedback = completed
        .writing_feedback
        .as_ref()
        .expect("writing feedback");
    let statistics = feedback.text_statistics.expect("text statistics");
    assert_eq!(statistics.sentence_count, Some(5));
    assert_eq!(statistics.average_word_length, Some(4.7));
    let score = feedback.score.as_ref().expect("score");
    assert_eq!(score.overall_score, Some(94));
    assert_eq!(score.grammar_corrections_score, Some(93));
    let readability = feedback.readability.as_ref().expect("readability");
    assert_eq!(
        readability.readability_level_text.as_deref(),
        Some("5th Grader")
    );

    // The AI alert of this example still decodes, including its deprecated `probability`.
    let ai = completed
        .ai_detection_result()
        .expect("valid")
        .expect("AI result");
    assert_eq!(ai.results[0].probability, Some(0.7307997032499992));
}

#[test]
fn completed_webhook_docs_example_has_the_new_sections() {
    // The scan-completed example of the API docs.
    let completed: CompletedWebhook = parse("completed_webhook.json");
    let results = completed.results.as_ref().expect("results");
    assert_eq!(results.internal_ai_data.len(), 1);
    assert_eq!(results.internal_ai_data[0].similar_words, Some(0));
    assert!(completed.references_validation.is_some());

    // Absent sections stay empty.
    let minimal: CompletedWebhook =
        webhooks::parse(br#"{"status": 0, "results": {}}"#).expect("parses");
    assert!(minimal
        .results
        .as_ref()
        .expect("results")
        .internal_ai_data
        .is_empty());
    assert!(minimal.writing_feedback.is_none());
    assert!(minimal.references_validation.is_none());
}

#[test]
fn new_result_webhook_internal_ai_data() {
    let body = json!({
        "score": { "aggregatedScore": 3.5 },
        "internalAIData": [null, { "id": "a1", "matchedWords": 12, "totalWords": 40 }]
    });
    let new_result: NewResultWebhook = serde_json::from_value(body).expect("parses");
    assert_eq!(new_result.internal_ai_data.len(), 1);
    assert_eq!(new_result.internal_ai_data[0].id.as_deref(), Some("a1"));
    assert_eq!(new_result.internal_ai_data[0].total_words, Some(40));
}

#[test]
fn ai_video_webhook() {
    use copyleaks::ai_video::AiVideoDetectionResponse;

    let video: AiVideoDetectionResponse = parse("ai_video_webhook.json");
    assert_eq!(video.model.as_deref(), Some("ai-video-1-pro"));
    let audio = video.audio_result.as_ref().expect("audio");
    assert_eq!(audio.starts, [13000, 45000, 47000]);
    assert_eq!(audio.lengths, [14000, 1000, 8700]);
    let excluded = audio.exclude.as_ref().expect("excluded audio");
    assert_eq!(excluded.starts.len(), 5);
    let visual = video.visual_result.as_ref().expect("visual");
    assert_eq!(visual.starts, [11566, 29433]);
    assert!(visual
        .exclude
        .as_ref()
        .expect("empty exclude")
        .starts
        .is_empty());
    let summary = video.summary.expect("summary");
    assert_eq!(summary.audio_ai_ratio, Some(0.4902));
    assert_eq!(summary.visual_ai_ratio, Some(0.5817));
    assert_eq!(summary.overall_ai_ratio, Some(0.7487));
    assert_eq!(video.is_ai_detected, Some(true));
    let info = video.video_info.as_ref().expect("video info");
    assert_eq!(info.duration, Some(55.7));
    let metadata = info.metadata.as_ref().expect("metadata");
    assert_eq!(metadata.app_or_device_used.as_deref(), Some("Sora"));
    let scanned = video.scanned_video.as_ref().expect("scanned video");
    assert_eq!(scanned.scan_id.as_deref(), Some("my-video-scan-1"));
    assert_eq!(scanned.actual_credits, Some(1));
    assert_eq!(
        scanned.creation_time.as_deref(),
        Some("2026-05-05T12:37:50Z")
    );
    assert!(video.error.is_none());
    assert!(video.developer_payload.is_none());
}

#[test]
fn ai_video_webhook_echoes_the_developer_payload() {
    use copyleaks::ai_video::AiVideoDetectionResponse;

    let mut body = fixture("ai_video_webhook.json");
    body["developerPayload"] = json!("order-1");
    let video: AiVideoDetectionResponse =
        webhooks::parse(&serde_json::to_vec(&body).expect("bytes")).expect("parses");
    assert_eq!(video.developer_payload.as_deref(), Some("order-1"));
    assert_eq!(video.is_ai_detected, Some(true));
    assert!(video.error.is_none());
}

/// Checks the parsed `ai_detection_error_webhook.json`.
fn assert_detection_error(error: Option<&webhooks::WebhookError>, developer_payload: Option<&str>) {
    let error = error.expect("error");
    assert_eq!(error.error_type.as_deref(), Some("invalid_request_error"));
    assert_eq!(error.id.as_deref(), Some("invalid_file"));
    assert_eq!(error.code, Some(1));
    assert_eq!(
        error.message.as_deref(),
        Some("The file could not be processed.")
    );
    assert!(error.url.is_none());
    assert!(error.details.is_empty());
    assert_eq!(developer_payload, Some("order-1"));
}

#[test]
fn ai_video_error_webhook() {
    use copyleaks::ai_video::AiVideoDetectionResponse;

    let video: AiVideoDetectionResponse = parse("ai_detection_error_webhook.json");
    assert_detection_error(video.error.as_ref(), video.developer_payload.as_deref());
    assert!(video.model.is_none());
    assert!(video.is_ai_detected.is_none());
    assert!(video.summary.is_none());
}

#[test]
fn ai_image_async_error_webhook() {
    use copyleaks::ai_image::AiImageDetectionResponse;

    let image: AiImageDetectionResponse = parse("ai_detection_error_webhook.json");
    assert_detection_error(image.error.as_ref(), image.developer_payload.as_deref());
    assert!(image.model.is_none());
    assert!(image.is_ai_detected.is_none());
    assert!(image.detected_objects.is_empty());
}

#[test]
fn ai_detection_error_with_url_and_details() {
    use copyleaks::ai_video::AiVideoDetectionResponse;

    let body = json!({
        "error": {
            "type": "invalid_request_error",
            "id": "missing_parameter",
            "code": 1,
            "message": "Bad request.",
            "url": "https://docs.copyleaks.com/using-the-apis/api-errors",
            "details": [{ "param": "url", "message": "The video could not be downloaded." }]
        }
    });
    let video: AiVideoDetectionResponse =
        webhooks::parse(&serde_json::to_vec(&body).expect("bytes")).expect("parses");
    let error = video.error.as_ref().expect("error");
    assert_eq!(
        error.url.as_deref(),
        Some("https://docs.copyleaks.com/using-the-apis/api-errors")
    );
    assert_eq!(error.details.len(), 1);
    assert_eq!(error.details[0].param.as_deref(), Some("url"));
    assert!(video.developer_payload.is_none());
}

#[test]
fn ai_image_async_webhook() {
    use copyleaks::ai_image::AiImageDetectionResponse;

    // The sandbox result of a heatmap submission with object detection.
    let body = json!({
        "model": "ai-image-1-ultra",
        "result": { "starts": [0], "lengths": [1048576], "labels": [4] },
        "summary": { "ai": 1.0, "human": 0.0 },
        "imageInfo": {
            "shape": { "width": 1024, "height": 1024 },
            "metadata": { "issuedBy": "OpenAI" }
        },
        "detectedObjects": [
            { "object": "face", "mask": { "starts": [100000], "lengths": [50000] } }
        ],
        "isAiDetected": true,
        "developerPayload": "order-1"
    });
    let response: AiImageDetectionResponse =
        webhooks::parse(&serde_json::to_vec(&body).expect("bytes")).expect("parses");
    assert_eq!(
        response
            .result
            .as_ref()
            .and_then(|mask| mask.labels.clone()),
        Some(vec![4])
    );
    assert_eq!(response.detected_objects[0].object.as_deref(), Some("face"));
    assert_eq!(response.developer_payload.as_deref(), Some("order-1"));
    assert_eq!(response.is_ai_detected, Some(true));
    assert!(response.error.is_none());
}

#[test]
fn exported_result_file() {
    use copyleaks::exports::ExportedResult;

    let result: ExportedResult = parse("exported_result.json");
    let statistics = result.statistics.expect("statistics");
    assert_eq!(statistics.identical, Some(0));
    assert_eq!(statistics.minor_changes, Some(0));
    assert_eq!(statistics.related_meaning, Some(0));
    let text = result.text.as_ref().expect("text");
    assert_eq!(text.value.as_deref(), Some("Hello world!"));
    assert_eq!(text.pages.as_ref().expect("pages").start_position, [0]);
    let identical = text
        .comparison
        .as_ref()
        .and_then(|comparison| comparison.identical.as_ref())
        .expect("identical matches");
    let source = identical
        .source
        .as_ref()
        .and_then(|source| source.chars.as_ref())
        .expect("source chars");
    assert_eq!(
        (source.starts.clone(), source.lengths.clone()),
        (vec![0], vec![1])
    );
    assert!(identical.suspected.is_some());
    let html = result.html.as_ref().expect("html");
    let related = html
        .comparison
        .as_ref()
        .and_then(|comparison| comparison.related_meaning.as_ref())
        .expect("related meaning");
    assert_eq!(related.group_id.as_deref(), Some(&[0][..]));
}

#[test]
fn crawled_version_file_with_citation_checker() {
    use copyleaks::exports::CrawledVersion;

    let crawled: CrawledVersion = parse("crawled_version.json");
    let metadata = crawled.metadata.expect("metadata");
    assert_eq!((metadata.words, metadata.excluded), (Some(30), Some(2)));
    let text = crawled.text.as_ref().expect("text");
    let excluded = text.exclude.as_ref().expect("excluded text");
    assert_eq!(excluded.reasons, [3]);
    assert_eq!(excluded.group_ids, None);
    let html = crawled.html.as_ref().expect("html");
    assert_eq!(
        html.exclude
            .as_ref()
            .expect("excluded html")
            .group_ids
            .as_deref(),
        Some(&[1][..])
    );

    let validation = crawled.references_validation.as_ref().expect("references");
    assert_eq!(validation.summary.expect("summary").total, Some(2));
    assert_eq!(validation.results.len(), 2);
    let academic = &validation.results[0];
    assert_eq!(academic.reference_type.as_deref(), Some("academic"));
    assert_eq!(
        academic.text_ranges.as_ref().expect("text ranges").starts,
        [131]
    );
    assert_eq!(
        academic
            .html_ranges
            .as_ref()
            .and_then(|ranges| ranges.group_ids.clone()),
        Some(vec![0])
    );
    let parsed = academic.parsed.as_ref().expect("parsed");
    assert_eq!(parsed.title.as_deref(), Some("Attention Is All You Need"));
    assert_eq!(parsed.authors.len(), 3);
    assert_eq!(parsed.year, Some(2017));
    let suggestion = &academic.suggestions[0];
    assert_eq!(suggestion.published_year, Some(2017));
    assert_eq!(suggestion.modified_year, Some(2023));
    let signals = suggestion.signals.expect("signals");
    assert_eq!(
        (signals.title, signals.year, signals.authors),
        (Some(true), Some(true), Some(true))
    );
    let web = &validation.results[1];
    assert_eq!(web.reference_type.as_deref(), Some("nonAcademic"));
    assert_eq!(
        web.parsed.as_ref().and_then(|parsed| parsed.url.as_deref()),
        Some("https://en.wikipedia.org/wiki/Artificial_intelligence")
    );
}

#[test]
fn reference_ranges_sent_as_an_empty_list_are_none() {
    use copyleaks::exports::ReferenceResult;

    // Plain text scans have no HTML layout: the docs show `"htmlRanges": []`.
    let result: ReferenceResult = serde_json::from_value(json!({
        "type": "academic",
        "textRanges": { "starts": [5], "lengths": [10] },
        "htmlRanges": [],
        "suggestions": null
    }))
    .expect("parses");
    assert!(result.html_ranges.is_none());
    assert_eq!(result.text_ranges.expect("text ranges").lengths, [10]);
    assert!(result.suggestions.is_empty());
}

#[test]
fn ai_overview_file() {
    use copyleaks::exports::AiOverview;

    let overview: AiOverview = parse("ai_overview.json");
    assert!(overview
        .overview
        .as_deref()
        .expect("overview")
        .starts_with("### Historical Author Data:"));
    assert_eq!(overview.model_version.as_deref(), Some("v1"));
}
