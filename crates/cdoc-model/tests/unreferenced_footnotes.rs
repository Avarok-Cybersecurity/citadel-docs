//! Which footnotes a file should not carry: those no reference names, counting references
//! inside footnotes only when that footnote is itself referenced.

use cdoc_model::test_support::fixtures::kitchen_sink;
use cdoc_model::{
    Block, Footnote, FootnoteId, Inline, InlineContent, Paragraph, ParagraphProperties,
};

fn note(id: u32, refers_to: Option<u32>) -> Footnote {
    let inlines = refers_to
        .map(|r| {
            vec![Inline::plain(InlineContent::FootnoteRef {
                id: FootnoteId(r),
            })]
        })
        .unwrap_or_default();
    Footnote {
        id: FootnoteId(id),
        blocks: vec![Block::Paragraph(Paragraph {
            properties: ParagraphProperties::none(),
            inlines,
        })],
    }
}

#[test]
fn a_referenced_footnote_stays_and_an_orphan_chain_goes() {
    let mut doc = kitchen_sink();
    assert_eq!(doc.unreferenced_footnotes(), []);
    let referenced: Vec<FootnoteId> = doc.footnotes.iter().map(|f| f.id).collect();
    doc.footnotes.push(note(50, Some(51)));
    doc.footnotes.push(note(51, None));
    assert_eq!(
        doc.unreferenced_footnotes(),
        [FootnoteId(50), FootnoteId(51)]
    );
    assert_eq!(
        doc.prune_unreferenced_footnotes(),
        [FootnoteId(50), FootnoteId(51)]
    );
    assert_eq!(
        doc.footnotes.iter().map(|f| f.id).collect::<Vec<_>>(),
        referenced
    );
    assert!(doc.validate().is_ok());
}

#[test]
fn a_footnote_named_from_a_referenced_footnote_stays() {
    let mut doc = kitchen_sink();
    let first = doc.footnotes[0].id;
    let Block::Paragraph(p) = &mut doc.footnotes[0].blocks[0] else {
        panic!("a paragraph")
    };
    p.inlines.push(Inline::plain(InlineContent::FootnoteRef {
        id: FootnoteId(60),
    }));
    doc.footnotes.push(note(60, None));
    assert_eq!(doc.unreferenced_footnotes(), []);
    assert!(doc.footnotes.iter().any(|f| f.id == first));
}
