//! Stories: a click in a header, footer or footnote lands in it; the commands that enter and
//! leave them; typing and undo inside them; and a selection that never spans two.

mod common;

use cdoc_crdt::Furniture;
use cdoc_edit::{Command, EditError, Editor, Selection, Story};
use cdoc_model::{FootnoteId, HeaderFooterKind};
use common::{caret, check, editor, gap, noted, paragraphs, type_chars};

const KIND: HeaderFooterKind = HeaderFooterKind::Default;

fn footnote_text(e: &Editor<'_>, id: FootnoteId) -> String {
    let note = e
        .model()
        .footnotes
        .iter()
        .find(|f| f.id == id)
        .expect("a body");
    format!("{:?}", note.blocks)
}

#[test]
fn a_click_on_a_header_footer_or_footnote_line_lands_in_that_story() {
    // The kitchen sink's default header and footer, shown on every page.
    let mut doc = cdoc_model::test_support::fixtures::kitchen_sink();
    doc.settings.even_and_odd_headers = false;
    for s in &mut doc.sections {
        s.properties.title_page = false;
    }
    let mut e = editor(&doc, 1);
    let mut seen = Vec::new();
    for page in 0..e.page_count() {
        for (_, rect) in e.stops(page) {
            let hit = e
                .hit_test(page, rect.x, rect.y + rect.height / 2)
                .expect("hits");
            e.apply(Command::SetSelection(Selection::caret(hit)))
                .expect("selects");
            let story = e.story().expect("a story");
            if !seen.contains(&story) {
                seen.push(story);
            }
        }
    }
    assert!(seen.contains(&Story::Body));
    assert!(
        seen.iter().any(|s| matches!(s, Story::Header { .. })),
        "{seen:?}"
    );
    assert!(
        seen.iter().any(|s| matches!(s, Story::Footer { .. })),
        "{seen:?}"
    );
    assert!(
        seen.iter().any(|s| matches!(s, Story::Footnote(_))),
        "{seen:?}"
    );
}

#[test]
fn typing_in_a_footnote_and_returning_leaves_the_body_as_it_was() {
    let mut e = noted();
    let body = e.model().sections.clone();
    let reference = e.selection();
    let start = gap(&e, 0, 0);
    caret(&mut e, start);
    e.apply(Command::GoToFootnote(FootnoteId(7)))
        .expect("enters");
    assert_eq!(e.story().unwrap(), Story::Footnote(FootnoteId(7)));
    type_chars(&mut e, "a note");
    e.apply(Command::SplitParagraph).expect("splits");
    type_chars(&mut e, "more");
    e.apply(Command::ReturnToBody).expect("returns");
    assert_eq!(e.story().unwrap(), Story::Body);
    assert_eq!(e.selection(), reference, "back just after the reference");
    assert_eq!(e.model().sections, body, "the body is unchanged");
    assert!(footnote_text(&e, FootnoteId(7)).contains("a note"));
    check(&e);
    e.undo().unwrap().expect("an undo step");
    assert!(!footnote_text(&e, FootnoteId(7)).contains("more"));
    assert_eq!(e.model().sections, body);
    check(&e);
}

#[test]
fn backspace_and_delete_stop_at_a_footnotes_edges() {
    let mut e = noted();
    let body = e.model().sections.clone();
    e.apply(Command::GoToFootnote(FootnoteId(7)))
        .expect("enters");
    type_chars(&mut e, "x");
    for _ in 0..3 {
        let _ = e.apply(Command::DeleteBackward(cdoc_edit::DeleteUnit::Char));
    }
    for _ in 0..3 {
        let _ = e.apply(Command::DeleteForward(cdoc_edit::DeleteUnit::Char));
    }
    assert_eq!(e.story().unwrap(), Story::Footnote(FootnoteId(7)));
    assert_eq!(e.model().sections, body);
    assert_eq!(e.model().footnotes.len(), 1);
    check(&e);
}

#[test]
fn a_header_edit_shows_on_every_page_of_its_section() {
    let texts: Vec<String> = (0..120).map(|k| format!("line {k}")).collect();
    let texts: Vec<&str> = texts.iter().map(String::as_str).collect();
    let mut e = editor(&paragraphs(&texts), 1);
    assert!(e.page_count() >= 3);
    let (a, b) = (gap(&e, 40, 2), gap(&e, 41, 3));
    common::select(&mut e, a, b);
    let before = e.selection();
    e.apply(Command::EditHeaderFooter {
        furniture: Furniture::Header,
        section: 0,
        kind: KIND,
    })
    .expect("adds a header");
    assert_eq!(
        e.story().unwrap(),
        Story::Header {
            section: 0,
            kind: KIND
        }
    );
    type_chars(&mut e, "Quarterly");
    for (k, page) in e.layout().pages.iter().enumerate() {
        assert!(
            page.runs().any(|r| r.text.contains("Quarterly")),
            "page {k} shows the header"
        );
    }
    check(&e);
    e.apply(Command::ReturnToBody).expect("returns");
    assert_eq!(e.selection(), before, "back to the body selection");
    e.apply(Command::EditHeaderFooter {
        furniture: Furniture::Header,
        section: 0,
        kind: KIND,
    })
    .expect("enters the existing header");
    let headers = e.model().sections[0].properties.headers.clone();
    assert!(headers.first.is_none() && headers.even.is_none());
    assert!(format!("{:?}", headers.default).contains("Quarterly"));
}

#[test]
fn a_header_kind_the_section_never_shows_is_refused() {
    let mut e = editor(&paragraphs(&["body"]), 1);
    for kind in [HeaderFooterKind::First, HeaderFooterKind::Even] {
        let refused = e.apply(Command::EditHeaderFooter {
            furniture: Furniture::Footer,
            section: 0,
            kind,
        });
        assert!(
            matches!(refused, Err(EditError::NotApplicable(_))),
            "{kind:?}"
        );
    }
    let missing = e.apply(Command::EditHeaderFooter {
        furniture: Furniture::Footer,
        section: 1,
        kind: KIND,
    });
    assert!(matches!(missing, Err(EditError::InvalidArgument(_))));
    let none = e.apply(Command::GoToFootnote(FootnoteId(3)));
    assert!(matches!(none, Err(EditError::NotApplicable(_))));
}

#[test]
fn undoing_a_new_footer_removes_it_and_returns_to_the_body() {
    let mut e = editor(&paragraphs(&["body"]), 1);
    let before = e.selection();
    e.apply(Command::EditHeaderFooter {
        furniture: Furniture::Footer,
        section: 0,
        kind: KIND,
    })
    .expect("adds a footer");
    assert!(e.model().sections[0].properties.footers.default.is_some());
    e.undo().unwrap().expect("an undo step");
    assert!(e.model().sections[0].properties.footers.default.is_none());
    assert_eq!(e.selection(), before);
    check(&e);
}
