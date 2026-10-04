//! The facade behaves on the native target exactly as the WASM build will: same functions,
//! same strings.

use cdoc_model::test_support::fixtures::kitchen_sink;
use cdoc_wasm::facade;

fn kitchen_json() -> String {
    serde_json::to_string(&kitchen_sink()).unwrap()
}

#[test]
fn model_pm_model_through_the_facade_is_identity() {
    let pm = facade::model_to_pm(&kitchen_json()).unwrap();
    facade::check_pm(&pm).unwrap();
    let back = facade::pm_to_model(&pm).unwrap();
    assert_eq!(
        serde_json::from_str::<cdoc_model::Document>(&back).unwrap(),
        kitchen_sink()
    );
}

#[test]
fn blank_documents_are_valid_and_agree() {
    assert_eq!(
        facade::validate_model(&facade::blank_model()).unwrap(),
        "[]"
    );
    assert_eq!(
        facade::model_to_pm(&facade::blank_model()).unwrap(),
        facade::blank_pm()
    );
    facade::check_pm(&facade::blank_pm()).unwrap();
}

#[test]
fn validation_reports_messages_as_a_json_array() {
    let mut doc = kitchen_sink();
    doc.sections.clear();
    let out = facade::validate_model(&serde_json::to_string(&doc).unwrap()).unwrap();
    let messages: Vec<String> = serde_json::from_str(&out).unwrap();
    assert_eq!(messages, ["the document has no sections"]);
}

#[test]
fn bad_inputs_are_errors_not_panics() {
    assert!(facade::model_to_pm("{")
        .unwrap_err()
        .starts_with("not a model document"));
    assert!(facade::pm_to_model("[]")
        .unwrap_err()
        .contains("not a ProseMirror node"));
    assert!(facade::check_pm(r#"{"type":"paragraph"}"#)
        .unwrap_err()
        .contains("top node must be \"doc\""));
    assert!(facade::validate_model("null").is_err());
}

#[test]
fn schema_and_word_count_pass_through() {
    assert_eq!(facade::schema(), cdoc_pm::schema::schema_json());
    assert_eq!(facade::word_count(&kitchen_json()).unwrap(), 16);
}
