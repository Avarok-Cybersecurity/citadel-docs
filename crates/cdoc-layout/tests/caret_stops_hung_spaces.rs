//! A space that ends a paragraph or stands before a forced break is laid out and given a caret
//! stop, so a typed space moves the caret; nothing is drawn for it.

mod support;

use cdoc_layout::caret_stops;
use cdoc_model::{Inline, InlineContent};

#[test]
fn spaces_ending_a_paragraph_hang_past_its_last_word_and_the_caret_stands_after_each() {
    let laid = support::laid_out(&support::doc_with(vec![support::para("ab  ")]));
    let stops = &caret_stops(&laid.pages[0]).expect("runs")[0];
    assert_eq!(
        stops.iter().map(|s| s.gap).collect::<Vec<u32>>(),
        [0, 1, 2, 3, 4],
        "each space is a place the caret stands"
    );
    let xs: Vec<_> = stops.iter().map(|s| s.x).collect();
    assert!(
        xs.windows(2).all(|w| w[0] < w[1]),
        "each stop is past the one before: {xs:?}"
    );
    let bare = support::laid_out(&support::doc_with(vec![support::para("ab")]));
    assert_eq!(
        laid.pages[0].draws, bare.pages[0].draws,
        "nothing is drawn for the hung spaces"
    );
}

#[test]
fn spaces_before_a_line_break_hang_too() {
    let doc = support::doc_with(vec![support::para_with(
        None,
        vec![
            support::text("ab "),
            Inline::plain(InlineContent::Break {
                kind: cdoc_model::BreakKind::Line,
            }),
            support::text("c"),
        ],
    )]);
    let laid = support::laid_out(&doc);
    let first = &caret_stops(&laid.pages[0]).expect("runs")[0];
    let at = |gap: u32| first.iter().find(|s| s.gap == gap).map(|s| s.x);
    assert!(at(3) > at(2), "the caret stands past the space: {first:?}");
}
