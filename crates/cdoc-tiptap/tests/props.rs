//! Random chat documents: the mapper never panics, never refuses a node, keeps every
//! character a reader sees, gives a valid document, reports only classified losses, and gives
//! the same result every time.

use cdoc_model::loss::labels;
use cdoc_model::Severity;
use cdoc_tiptap::test_support::strategies::{foreign_doc, starter_kit_doc};
use cdoc_tiptap::test_support::{input_text, model_text};
use cdoc_tiptap::tiptap_to_document;
use proptest::prelude::*;
use serde_json::Value;

fn check(doc: &Value) -> Result<cdoc_model::Report, TestCaseError> {
    let json = doc.to_string();
    let (document, report) = tiptap_to_document(&json).expect("a node is never refused");
    prop_assert_eq!(model_text(&document), input_text(doc));
    prop_assert!(document.validate().is_ok(), "{:?}", document.validate());
    prop_assert!(document.is_normalized());
    for loss in &report.losses {
        prop_assert_ne!(loss.kind().label, labels::UNCLASSIFIED, "{:?}", loss);
    }
    let again = tiptap_to_document(&json).expect("reads again");
    prop_assert_eq!(&again.0, &document);
    prop_assert_eq!(&again.1, &report);
    Ok(report)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn starter_kit_documents_keep_their_text_and_lose_no_content(doc in starter_kit_doc()) {
        let report = check(&doc)?;
        prop_assert_eq!(report.count_at_least(Severity::Major), 0, "{:?}", report.losses);
    }

    #[test]
    fn foreign_documents_keep_their_text_and_report_what_they_are(doc in foreign_doc()) {
        let report = check(&doc)?;
        let json = doc.to_string();
        let foreign = ["mention", "taskList"].iter().any(|k| json.contains(&format!("\"type\":\"{k}\"")));
        prop_assert_eq!(foreign, report.count_at_least(Severity::Major) > 0, "{:?}", report.losses);
    }

    #[test]
    fn any_json_value_is_read_or_refused_without_panicking(doc in any_json()) {
        let _ = tiptap_to_document(&doc.to_string());
    }
}

/// Arbitrary JSON shaped loosely like nodes: objects with and without `type`, `content`,
/// `marks`, `attrs` and `text` of every JSON kind.
fn any_json() -> BoxedStrategy<Value> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        any::<i64>().prop_map(Value::from),
        "[a-zA-Z]{0,6}".prop_map(Value::from),
        Just(Value::from("paragraph")),
        Just(Value::from("text")),
        Just(Value::from("orderedList")),
    ]
    .boxed();
    leaf.prop_recursive(5, 64, 4, |inner| {
        let keys = prop_oneof![
            Just("type"),
            Just("content"),
            Just("marks"),
            Just("attrs"),
            Just("text"),
            Just("level"),
            Just("start")
        ];
        prop_oneof![
            proptest::collection::vec(inner.clone(), 0..4).prop_map(Value::from),
            proptest::collection::vec((keys, inner), 0..5).prop_map(|pairs| {
                Value::Object(pairs.into_iter().map(|(k, v)| (k.to_owned(), v)).collect())
            }),
        ]
        .boxed()
    })
    .boxed()
}
