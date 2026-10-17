//! The long document: edits in the middle and at the top relayout to exactly a fresh layout,
//! repainting only the pages they must.

mod long;
mod support;

use cdoc_layout::{BlockEdit, Dirty, Layout, LayoutOptions, LayoutSession};

#[test]
fn the_long_document_is_about_a_hundred_pages() {
    let pages = support::laid_out(&long::hundred_pages()).pages.len();
    assert!((95..=130).contains(&pages), "{pages} pages");
}

/// Lays `doc` out, applies `typed` at `index` and returns (before, after, changed pages).
fn edit(index: usize, typed: &str) -> (Layout, Layout, Vec<usize>) {
    edit_in(&long::hundred_pages(), index, typed)
}

fn edit_in(doc: &cdoc_model::Document, index: usize, typed: &str) -> (Layout, Layout, Vec<usize>) {
    let fonts = support::fonts();
    let options = LayoutOptions::word_like();
    let mut session = LayoutSession::new(&fonts, &options);
    let before = session
        .relayout(doc, &Dirty::all())
        .expect("lays out")
        .layout()
        .clone();
    let edited = long::typed(doc, index, typed);
    let dirty = Dirty::edits(vec![BlockEdit::changed(0, index)]);
    let relayout = session.relayout(&edited, &dirty).expect("relays out");
    assert_eq!(
        relayout.layout(),
        &support::laid_out(&edited),
        "a fresh layout"
    );
    (
        before,
        relayout.layout().clone(),
        relayout.changed_pages().to_vec(),
    )
}

#[test]
fn a_character_typed_in_the_middle_repaints_one_page() {
    let index = long::middle(&long::hundred_pages());
    let (before, after, changed) = edit(index, "x");
    let [page] = changed[..] else {
        panic!("one page repainted, got {changed:?}");
    };
    assert!(
        page > 30 && page < after.pages.len() - 30,
        "page {page} is mid-document"
    );
    for (i, (b, a)) in before.pages.iter().zip(&after.pages).enumerate() {
        assert_eq!(b == a, i != page, "page {i}");
    }
}

#[test]
fn a_character_typed_at_the_top_repaints_the_first_page() {
    let (_, _, changed) = edit(0, "x");
    assert_eq!(changed, [0]);
}

#[test]
fn lines_added_at_the_top_reflow_and_every_moved_page_is_repainted() {
    let (before, after, changed) = edit(9, &" and more".repeat(60));
    let moved: Vec<usize> = (0..after.pages.len())
        .filter(|i| before.pages.get(*i) != Some(&after.pages[*i]))
        .collect();
    assert_ne!(moved.len(), 0, "the edit moved something");
    assert!(
        moved.iter().all(|i| changed.contains(i)),
        "moved {moved:?}, reported {changed:?}"
    );
}

#[test]
fn in_plain_prose_a_line_added_at_the_top_moves_every_page() {
    let doc = long::plain_prose();
    let (before, after, changed) = edit_in(&doc, 0, &" and more".repeat(12));
    assert!(before.pages.len() >= 95, "{} pages", before.pages.len());
    let moved: Vec<usize> = (0..after.pages.len())
        .filter(|i| before.pages.get(*i) != Some(&after.pages[*i]))
        .collect();
    // The prose repeats, so a page may come out as it was; the second section starts afresh.
    assert!(
        moved.len() + 3 >= after.pages.len(),
        "{} of {} moved",
        moved.len(),
        after.pages.len()
    );
    assert!(
        moved.iter().all(|i| changed.contains(i)),
        "moved {moved:?}, reported {changed:?}"
    );
}
