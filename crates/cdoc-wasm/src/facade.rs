//! The facade's logic, as plain string-in string-out functions so it is tested natively and the
//! wasm-bindgen layer in `lib.rs` is nothing but signatures. Every input and output is JSON
//! text: the browser already has a JSON parser, the payloads contain no `BigInt`, and a text
//! boundary keeps the WASM ABI to one type.

use cdoc_model::Document;
use cdoc_pm::schema::{check, schema as pm_schema, schema_json};
use cdoc_pm::{from_pm_json, to_pm_json, PmNode};

fn parse_document(model_json: &str) -> Result<Document, String> {
    serde_json::from_str(model_json).map_err(|e| format!("not a model document: {e}"))
}

fn to_json<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_string(value).expect("model values serialise")
}

/// Model JSON → ProseMirror JSON.
pub fn model_to_pm(model_json: &str) -> Result<String, String> {
    Ok(to_pm_json(&parse_document(model_json)?))
}

/// ProseMirror JSON → model JSON.
pub fn pm_to_model(pm_json: &str) -> Result<String, String> {
    from_pm_json(pm_json)
        .map(|doc| to_json(&doc))
        .map_err(|e| e.to_string())
}

/// Validate a model document. `Ok` holds a JSON array of error messages; empty means valid.
pub fn validate_model(model_json: &str) -> Result<String, String> {
    let doc = parse_document(model_json)?;
    let messages: Vec<String> = match doc.validate() {
        Ok(()) => Vec::new(),
        Err(errors) => errors.iter().map(ToString::to_string).collect(),
    };
    Ok(to_json(&messages))
}

/// Check a ProseMirror JSON tree against the schema; `Err` names the first violation.
pub fn check_pm(pm_json: &str) -> Result<(), String> {
    let node: PmNode =
        serde_json::from_str(pm_json).map_err(|e| format!("not a ProseMirror node: {e}"))?;
    check(&node, &pm_schema()).map_err(|e| e.to_string())
}

/// The ProseMirror schema as JSON, for the UI's schema-drift test.
#[must_use]
pub fn schema() -> String {
    schema_json()
}

/// A new, blank document as model JSON.
#[must_use]
pub fn blank_model() -> String {
    to_json(&Document::blank())
}

/// A new, blank document as ProseMirror JSON: what the editor opens on "New".
#[must_use]
pub fn blank_pm() -> String {
    to_pm_json(&Document::blank())
}

/// Words in the body of a model document.
pub fn word_count(model_json: &str) -> Result<u32, String> {
    let doc = parse_document(model_json)?;
    u32::try_from(doc.word_count()).map_err(|_| "word count overflows u32".to_owned())
}
