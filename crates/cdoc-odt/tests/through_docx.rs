//! odt -> model -> docx -> model equals odt -> model on what both formats hold: the model is
//! the common ground, so a document read from ODF survives conversion to Word. Word stores
//! lengths in twentieths of a point (border spacing in points), coarser than the model's
//! hundredths: a length may move by up to one Word unit, nothing else may change.

mod common;

use cdoc_docx::{read_docx, write_docx};
use cdoc_model::test_support::fixtures::kitchen_sink;
use cdoc_model::Document;
use cdoc_odt::{MemoryMedia, ReadLimits, WriteOptions};
use serde_json::Value;

/// Keys whose numbers are lengths, with the largest move Word's rounding makes (Pt100).
fn tolerance(key: &str) -> i64 {
    match key {
        "space" => 50,
        "top" | "bottom" | "left" | "right" | "header" | "footer" | "gutter" | "width"
        | "height" | "gap" | "defaultTabStop" | "before" | "after" | "by" | "length"
        | "columnWidths" => 3,
        _ => 0,
    }
}

fn same(key: &str, a: &Value, b: &Value, path: &str, out: &mut Vec<String>) {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) if x.len() == y.len() => {
            for (k, v) in x {
                same(
                    k,
                    v,
                    y.get(k).unwrap_or(&Value::Null),
                    &format!("{path}/{k}"),
                    out,
                );
            }
        }
        (Value::Array(x), Value::Array(y)) if x.len() == y.len() => {
            for (i, (v, w)) in x.iter().zip(y).enumerate() {
                same(key, v, w, &format!("{path}/{i}"), out);
            }
        }
        (Value::Number(x), Value::Number(y)) => {
            let (x, y) = (
                x.as_i64().unwrap_or(i64::MIN),
                y.as_i64().unwrap_or(i64::MAX),
            );
            if (x - y).abs() > tolerance(key) {
                out.push(format!("{path}: {x} != {y}"));
            }
        }
        _ if a != b => out.push(format!("{path}: {a} != {b}")),
        _ => {}
    }
}

fn differences(a: &Document, b: &Document) -> Vec<String> {
    let mut out = Vec::new();
    let json = |d: &Document| serde_json::to_value(d).expect("serialises");
    same("", &json(a), &json(b), "", &mut out);
    out
}

fn through_docx(doc: &Document, media: &MemoryMedia) -> Document {
    let (bytes, _) = write_docx(doc, media, WriteOptions::document()).expect("docx");
    let mut again = MemoryMedia::with_prefix("img-");
    let mut back = read_docx(&bytes, &ReadLimits::generous(), &mut again)
        .expect("read docx")
        .document;
    for (old, new) in media.ids().iter().zip(again.ids()) {
        back.for_each_paragraph_mut(&mut |p| {
            for i in &mut p.inlines {
                if let cdoc_model::InlineContent::Image(img) = &mut i.content {
                    if img.content_id == *new {
                        img.content_id = old.clone();
                    }
                }
            }
        });
    }
    back
}

#[test]
fn the_kitchen_sink_read_from_odt_survives_docx() {
    let (bytes, _) = common::write(&kitchen_sink());
    let (from_odt, _, media) = common::read_foreign(&bytes);
    let back = through_docx(&from_odt, &media);
    assert!(
        back == from_odt,
        "{}",
        common::first_diff(&back, &from_odt).unwrap_or_default()
    );
}

#[test]
fn foreign_documents_read_from_odt_survive_docx() {
    for name in [
        "changeTracked.odt",
        "bullet_list.odt",
        "TestTextTable.odt",
        "HeaderFooter.odt",
        "_annotation.odt",
    ] {
        let bytes = std::fs::read(format!(
            "{}/tests/fixtures/{name}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .expect("fixture");
        let (from_odt, _, media) = common::read_foreign(&bytes);
        let back = through_docx(&from_odt, &media);
        let diffs = differences(&back, &from_odt);
        assert!(diffs.is_empty(), "{name}: {diffs:?}");
    }
}
