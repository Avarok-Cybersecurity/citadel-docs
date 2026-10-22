//! For tests here and in `cdoc-wasm`: a corpus of chat documents shaped as the old editor
//! saved them, the text a reader sees in a chat document and in a model document (the oracle
//! the mapper is checked against, written without it), and proptest strategies of
//! `StarterKit` trees.

pub mod strategies;

use cdoc_model::{Block, BreakKind, Document, InlineContent};
use serde_json::Value;

macro_rules! corpus {
    ($($name:literal),* $(,)?) => {
        [$(($name, include_str!(concat!("../../tests/corpus/", $name, ".json")))),*]
    };
}

/// The corpus, `(name, json)`, in a fixed order.
pub const CORPUS: [(&str, &str); 10] = corpus![
    "empty-doc",
    "empty-fragment",
    "headings",
    "long-paragraph",
    "meeting-notes",
    "mixed-marks",
    "nested-lists",
    "quote-code-rule",
    "unicode",
    "unknown-node",
];

/// The text of a chat document as a reader sees it: every `text` node's text in document
/// order, a hard break as a newline. Malformed nodes count for nothing.
#[must_use]
pub fn input_text(value: &Value) -> String {
    let mut out = String::new();
    collect(value, &mut out);
    out
}

fn collect(value: &Value, out: &mut String) {
    let Some(obj) = value.as_object() else {
        return;
    };
    match obj.get("type").and_then(Value::as_str) {
        Some("text") => out.push_str(obj.get("text").and_then(Value::as_str).unwrap_or("")),
        Some("hardBreak") => out.push('\n'),
        _ => {}
    }
    if let Some(children) = obj.get("content").and_then(Value::as_array) {
        for child in children {
            collect(child, out);
        }
    }
}

/// The text of a model document's body: every text run in order, a line break as a newline.
#[must_use]
pub fn model_text(document: &Document) -> String {
    let mut out = String::new();
    for section in &document.sections {
        for block in &section.blocks {
            let Block::Paragraph(paragraph) = block else {
                continue;
            };
            for inline in &paragraph.inlines {
                match &inline.content {
                    InlineContent::Text(run) => out.push_str(&run.text),
                    InlineContent::Break {
                        kind: BreakKind::Line,
                    } => out.push('\n'),
                    _ => {}
                }
            }
        }
    }
    out
}
