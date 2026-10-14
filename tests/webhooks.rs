//! Webhook payload parsing, with the examples from the API docs, and the AI alert extraction of
//! the completed webhook.

mod common;

use common::fixture;
use copyleaks::plagiarism::{
    AiGeneratedText, AiSourceMatch, Author, CopyleaksDb, CrossLanguages, CustomMetadata,
    DomainsMode, Exclude, ExcludeCode, Filters, Indexing, IndexingRepository, MaskingPolicy, Pdf,
    PdfColors, PdfVersion, ScanMethodAlgorithm, ScanProperties, Scanning, ScanningExclude,
    SearchRepository, StartResponse, SubmitAction, SubmitWebhooks,
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

fn ai_alert(additional_data: Option<&str>) -> Alert {
    Alert {
        code: Some(alert_codes::SUSPECTED_AI_TEXT.to_owned()),
        additional_data: additional_data.map(str::to_owned),
        ..Alert::default()
    }
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
    let other = Alert {
        code: Some("suspected-white-text".to_owned()),
        additional_data: Some("{\"summary\": {\"ai\": 1}}".to_owned()),
        ..Alert::default()
    };
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
                .exclude(ScanningExclude::default().id_pattern("abc*"))
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
        .indexing(Indexing::default().repositories(vec![
            IndexingRepository::new("repo-1").masking_policy(MaskingPolicy::MaskOtherUsersFiles),
        ]))
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
                        .related_meaning("#FDD0A3"),
                ),
        )
        .display_language("es")
        .sensitivity_level(3)
        .cheat_detection(true)
        .scan_method_algorithm(ScanMethodAlgorithm::MaximumResults)
        .ai_generated_text(AiGeneratedText::default().detect(true))
        .ai_source_match(AiSourceMatch::default().enable(true))
        .custom_metadata(vec![CustomMetadata::new("course", "cs-101")]);

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
            "exclude": { "idPattern": "abc*" },
            "repositories": [
                { "id": "repo-1", "includeMySubmissions": true, "includeOthersSubmissions": false }
            ],
            "copyleaksDb": { "includeMySubmissions": true, "includeOthersSubmissions": true },
            "crossLanguages": { "languages": [{ "code": "fr" }, { "code": "es" }] }
        },
        "indexing": { "repositories": [{ "id": "repo-1", "maskingPolicy": 1 }] },
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
                "relatedMeaning": "#FDD0A3"
            }
        },
        "displayLanguage": "es",
        "sensitivityLevel": 3,
        "cheatDetection": true,
        "scanMethodAlgorithm": 1,
        "aiGeneratedText": { "detect": true },
        "aiSourceMatch": { "enable": true },
        "customMetadata": [{ "key": "course", "value": "cs-101" }]
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
