//! Documents other programs wrote (see `fixtures/LICENSES.md`): each reads into a valid model,
//! the content arrives where it belongs, and every report entry is of a kind this reader is
//! expected to report: an unmapped attribute or element, an unread part, or one of a fixed
//! set of sentences.

mod common;

use cdoc_model::{Document, InlineContent, Report, RevisionKind};

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!(
        "{}/tests/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("fixture")
}

const SENTENCES: [&str; 11] = [
    "a list item's further paragraphs read as items of their own",
    "a comment on a point kept without an anchor",
    "a list label's character style",
    "a list level beyond the ninth or out of sequence",
    "a soft page break (ignored)",
    "a style of family graphic",
    "a table column without a width; the table's width shared equally",
    "a tracked formatting change; the formatting is read as current",
    "an image anchored to the char read as an inline image",
    "application settings not read",
    "part not read",
];

fn expected(what: &str) -> bool {
    what.starts_with("style:") || what.starts_with("office:") || SENTENCES.contains(&what)
}

fn read(name: &str) -> (Document, Report, usize) {
    let (doc, report, media) = common::read_foreign(&fixture(name));
    assert_eq!(doc.validate(), Ok(()), "{name}");
    let unexpected: Vec<&str> = report
        .losses
        .iter()
        .filter_map(|l| l.what())
        .filter(|w| !expected(w))
        .collect();
    assert!(unexpected.is_empty(), "{name}: {unexpected:?}");
    (doc, report, media.ids().len())
}

fn text(doc: &Document) -> String {
    doc.inlines()
        .filter_map(|i| match &i.content {
            InlineContent::Text(t) => Some(t.text.as_str()),
            _ => None,
        })
        .collect()
}

#[test]
fn tracked_changes_arrive_as_insertions_and_deletions() {
    let (doc, report, _) = read("changeTracked.odt");
    let kinds: Vec<RevisionKind> = doc
        .inlines()
        .filter_map(|i| i.marks.revision.as_ref().map(|r| r.kind))
        .collect();
    assert!(kinds.contains(&RevisionKind::Deletion) && kinds.contains(&RevisionKind::Insertion));
    assert!(text(&doc).starts_with("Ich bin eine odt-Datei"));
    assert!(report.has(|_| true, |w| w.starts_with("a tracked formatting change")));
}

#[test]
fn bullet_lists_arrive_as_numbered_paragraphs() {
    let (doc, report, _) = read("bullet_list.odt");
    let numbered: Vec<_> = doc
        .paragraphs()
        .filter_map(|p| p.properties.numbering)
        .collect();
    assert_eq!(numbered.len(), 4);
    assert!(numbered
        .iter()
        .all(|n| n.level == 0 && n.num_id == numbered[0].num_id));
    assert_eq!(doc.numbering.instances.len(), 1);
    assert!(report.has(
        |_| true,
        |w| w.starts_with("a list item's further paragraphs")
    ));
}

#[test]
fn tables_arrive_with_their_cells() {
    let (doc, _, _) = read("TestTextTable.odt");
    let tables: Vec<_> = doc.blocks().flat_map(|(_, b)| b.tables()).collect();
    assert!(tables.len() >= 2);
    assert!(tables
        .iter()
        .all(|t| !t.rows.is_empty() && !t.column_widths.is_empty()));
    assert!(text(&doc).contains("cell1"));
}

#[test]
fn headers_footers_footnotes_comments_and_images_arrive() {
    let (doc, _, images) = read("HeaderFooter.odt");
    let props = &doc.sections[0].properties;
    assert!(props.headers.default.is_some() && props.footers.default.is_some());
    assert_eq!(doc.footnotes.len(), 1);
    assert_eq!(doc.comments.len(), 6);
    assert_eq!(images, 1);
    assert!(doc.inlines().any(|i| i.marks.link.is_some()));
}

#[test]
fn annotations_arrive_as_comments() {
    let (doc, _, _) = read("_annotation.odt");
    assert_eq!(doc.comments.len(), 1);
    assert!(text(&doc).contains("Before annotation"));
}
