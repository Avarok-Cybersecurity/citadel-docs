//! Footnotes at the foot of the page that references them.

mod support;

use cdoc_model::builtin::ids;
use cdoc_model::{Footnote, FootnoteId, Inline, InlineContent};
use support::{para_with, text};

#[test]
fn many_footnotes_push_their_references_and_never_overlap_the_body() {
    let mut blocks = Vec::new();
    let mut notes = Vec::new();
    for i in 1..=40 {
        blocks.push(para_with(
            None,
            vec![
                text(&format!("ref-{i}")),
                Inline::plain(InlineContent::FootnoteRef { id: FootnoteId(i) }),
            ],
        ));
        notes.push(Footnote {
            id: FootnoteId(i),
            blocks: vec![para_with(
                Some(ids::footnote_text()),
                vec![text(&format!("note-{i}"))],
            )],
        });
    }
    let mut doc = support::doc_with(blocks);
    doc.footnotes = notes;
    let layout = support::laid_out(&doc);
    assert!(layout.pages.len() >= 2);
    for i in 1..=40 {
        let exact = |needle: String| {
            layout
                .pages
                .iter()
                .position(|p| p.runs().any(|r| r.text == needle))
                .expect("placed")
        };
        assert_eq!(
            exact(format!("ref-{i}")),
            exact(format!("note-{i}")),
            "note {i} is on its reference's page"
        );
    }
    for page in &layout.pages {
        let margin = page.height - 7200;
        assert!(
            page.runs().all(|r| r.y <= margin),
            "a note ran into the bottom margin"
        );
        let lowest_body = page
            .runs()
            .filter(|r| r.text.starts_with("ref-"))
            .map(|r| r.y)
            .max();
        let highest_note = page
            .runs()
            .filter(|r| r.text.starts_with("note-"))
            .map(|r| r.y)
            .min();
        if let (Some(body), Some(note)) = (lowest_body, highest_note) {
            assert!(body < note - 1000, "body at {body}, notes from {note}");
        }
    }
}
