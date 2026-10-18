//! Shared by the integration tests: faces, options, fixtures and the invariant every edit
//! must keep (the model view equals `to_model` of the replica, and the document validates).

#![allow(dead_code, reason = "each test target uses a different part")]

pub mod ops;

use cdoc_crdt::{to_model, ClientId};
use cdoc_edit::{Command, Editor, Position, Selection};
use cdoc_layout::{BreakerChoice, FontLibrary, LayoutOptions, Params};
use cdoc_model::{Block, Document, Inline, Paragraph, ParagraphProperties, RunProperties};
use std::sync::OnceLock;

pub fn fonts() -> &'static FontLibrary {
    static FONTS: OnceLock<FontLibrary> = OnceLock::new();
    FONTS.get_or_init(cdoc_layout::test_support::default_fonts)
}

pub fn options() -> &'static LayoutOptions {
    static OPTIONS: OnceLock<LayoutOptions> = OnceLock::new();
    OPTIONS.get_or_init(|| LayoutOptions::word_like(BreakerChoice::KnuthPlass(Params::word_like())))
}

/// A one-section document with these paragraphs (Normal style, plain text).
pub fn paragraphs(texts: &[&str]) -> Document {
    let mut d = Document::blank();
    let Block::Paragraph(p) = d.sections[0].blocks[0].clone() else {
        unreachable!()
    };
    d.sections[0].blocks = texts
        .iter()
        .map(|t| {
            let inlines = if t.is_empty() {
                Vec::new()
            } else {
                vec![Inline::text(*t, RunProperties::none())]
            };
            Block::Paragraph(Paragraph {
                properties: p.properties.clone(),
                inlines,
            })
        })
        .collect();
    d
}

pub fn editor(doc: &Document, client: u64) -> Editor<'static> {
    Editor::from_document(doc, ClientId(client), fonts(), options()).expect("opens")
}

pub fn peer(of: &Editor<'_>, client: u64) -> Editor<'static> {
    Editor::open(&of.snapshot(), ClientId(client), fonts(), options()).expect("opens")
}

/// The model view equals the replica's model, and the document is valid.
pub fn check(e: &Editor<'_>) {
    let fresh = to_model(e.doc()).expect("converts");
    let (have, want) = (e.model(), &fresh);
    for (s, (a, b)) in have.sections.iter().zip(&want.sections).enumerate() {
        for (i, (x, y)) in a.blocks.iter().zip(&b.blocks).enumerate() {
            assert_eq!(x, y, "section {s} block {i} drifted from the replica");
        }
        assert_eq!(
            a.blocks.len(),
            b.blocks.len(),
            "section {s} block count drifted"
        );
    }
    assert!(
        have == want,
        "the model view drifted from the replica outside the bodies"
    );
    if let Err(errors) = fresh.validate() {
        panic!("invalid document: {errors:?}");
    }
}

/// The gap `index` as a position.
pub fn at(e: &Editor<'_>, index: usize) -> Position {
    Position::at(e.doc(), index).expect("a gap")
}

/// Puts the caret at gap `index`.
pub fn caret(e: &mut Editor<'_>, index: usize) {
    let p = at(e, index);
    e.apply(Command::SetSelection(Selection::caret(p)))
        .expect("selects");
}

/// Selects gaps `a..b`.
pub fn select(e: &mut Editor<'_>, a: usize, b: usize) {
    let s = Selection {
        anchor: at(e, a),
        head: at(e, b),
    };
    e.apply(Command::SetSelection(s)).expect("selects");
}

/// The text of paragraph `i` of section 0's body.
pub fn text(e: &Editor<'_>, i: usize) -> String {
    match &e.model().sections[0].blocks[i] {
        Block::Paragraph(p) => p
            .inlines
            .iter()
            .map(|x| match &x.content {
                cdoc_model::InlineContent::Text(r) => r.text.clone(),
                _ => "\u{fffc}".to_owned(),
            })
            .collect(),
        Block::Table(_) => "<table>".to_owned(),
    }
}

/// The paragraph properties of block `i` of section 0.
pub fn props(e: &Editor<'_>, i: usize) -> ParagraphProperties {
    match &e.model().sections[0].blocks[i] {
        Block::Paragraph(p) => p.properties.clone(),
        Block::Table(_) => panic!("block {i} is a table"),
    }
}

/// The gap `offset` units into body paragraph `block` of section 0.
pub fn gap(e: &Editor<'_>, block: usize, offset: usize) -> usize {
    let path = cdoc_edit::ModelPath {
        section: 0,
        block,
        cells: Vec::new(),
        offset,
    };
    e.position_at_path(&path)
        .expect("a paragraph")
        .index(e.doc())
        .expect("held")
}

/// The text runs of body paragraph `block` with their properties.
pub fn runs(e: &Editor<'_>, block: usize) -> Vec<(String, RunProperties)> {
    let Block::Paragraph(p) = &e.model().sections[0].blocks[block] else {
        panic!("block {block} is a table")
    };
    p.inlines
        .iter()
        .filter_map(|i| match &i.content {
            cdoc_model::InlineContent::Text(r) => Some((r.text.clone(), r.properties.clone())),
            _ => None,
        })
        .collect()
}

/// Bold, as direct formatting.
pub fn bold() -> RunProperties {
    RunProperties {
        bold: Some(true),
        ..RunProperties::none()
    }
}

/// Types `text` one character at a time, as a keyboard does.
pub fn type_chars(e: &mut Editor<'_>, text: &str) {
    for c in text.chars() {
        e.apply(Command::InsertText(c.to_string())).expect("types");
    }
}

/// `doc` without the footnote bodies no reference names: undoing a footnote's insertion
/// leaves its body unreferenced (and undrawn), as deleting the reference does.
#[allow(dead_code)]
pub fn referenced(doc: &Document) -> Document {
    let names: Vec<_> = doc
        .inlines()
        .filter_map(|i| match i.content {
            cdoc_model::InlineContent::FootnoteRef { id } => Some(id),
            _ => None,
        })
        .collect();
    let mut out = doc.clone();
    out.footnotes.retain(|f| names.contains(&f.id));
    out
}

/// Room for everything these tests buffer: the cap is not what they test
/// (`cdoc-crdt/tests/pending_limits.rs` does).
pub const ROOM: cdoc_crdt::PendingLimits = cdoc_crdt::PendingLimits {
    max_ops: usize::MAX,
    max_bytes: usize::MAX,
};

/// A one-paragraph body with footnote 7 after "one".
pub fn noted() -> Editor<'static> {
    let mut e = editor(&paragraphs(&["one two"]), 1);
    let g = gap(&e, 0, 3);
    caret(&mut e, g);
    e.apply(Command::InsertFootnote(Some(cdoc_model::FootnoteId(7))))
        .expect("inserts");
    e
}
