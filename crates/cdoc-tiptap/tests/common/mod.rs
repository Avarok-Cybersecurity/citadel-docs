//! Reading a chat document in tests, and looking at what came out.

#![allow(dead_code)]

use cdoc_model::{Block, InlineContent, Paragraph, Report, Severity};
use cdoc_tiptap::tiptap_to_document;
use serde_json::{json, Value};

pub fn read(doc: &Value) -> (Vec<Paragraph>, cdoc_model::Document, Report) {
    let (document, report) = tiptap_to_document(&doc.to_string()).expect("reads");
    let paragraphs = document.sections[0]
        .blocks
        .iter()
        .map(|b| match b {
            Block::Paragraph(p) => p.clone(),
            Block::Table(_) => panic!("no tables come from a chat document"),
        })
        .collect();
    (paragraphs, document, report)
}

pub fn style(p: &Paragraph) -> &str {
    p.properties.style.as_ref().map_or("", |s| s.as_str())
}

pub fn text(p: &Paragraph) -> String {
    p.inlines
        .iter()
        .map(|i| match &i.content {
            InlineContent::Text(r) => r.text.clone(),
            InlineContent::Break { .. } => "\n".to_owned(),
            _ => String::new(),
        })
        .collect()
}

pub fn losses(report: &Report) -> Vec<(String, Severity)> {
    report
        .losses
        .iter()
        .map(|l| (l.what().unwrap_or("").to_owned(), l.severity()))
        .collect()
}

pub fn t(s: &str) -> Value {
    json!({"type": "text", "text": s})
}
