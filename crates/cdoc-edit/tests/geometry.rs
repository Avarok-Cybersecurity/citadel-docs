//! Hit-testing, caret and selection rectangles, and vertical movement with a goal x.

mod common;

use cdoc_edit::{Direction, Granularity, Selection};
use common::*;

const LONG: &str = "The quick brown fox jumps over the lazy dog while the cat watches \
    from the window and the bird sings a long song about the morning light.";

fn path(e: &cdoc_edit::Editor<'_>, p: cdoc_edit::Position) -> cdoc_edit::ModelPath {
    e.model_path(p).expect("a position").expect("in the body")
}

fn block(e: &cdoc_edit::Editor<'_>, p: cdoc_edit::Position) -> usize {
    path(e, p).block
}

fn wrapped() -> cdoc_edit::Editor<'static> {
    editor(&paragraphs(&[LONG, "short", LONG]), 1)
}

#[test]
fn every_caret_stop_hit_tests_back_to_itself() {
    for (doc, has_asides) in [
        (cdoc_model::test_support::fixtures::kitchen_sink(), true),
        (paragraphs(&[LONG, "", "two words", LONG]), false),
    ] {
        let e = editor(&doc, 1);
        let (mut seen, mut asides) = (0, 0);
        for page in 0..e.page_count() {
            for (pos, rect) in e.stops(page) {
                let hit = e
                    .hit_test(page, rect.x, rect.y + rect.height / 2)
                    .expect("hits");
                let index = |p: cdoc_edit::Position| p.index(e.doc()).expect("a gap");
                assert_eq!(index(hit), index(pos), "hit {pos:?} on page {page}");
                // A header repeats on many pages; its caret is drawn on the first of them.
                let (p, r) = e.caret_rect(pos).expect("a caret");
                if e.model_path(pos).expect("a position").is_some() {
                    assert_eq!((p, r), (page, rect), "the caret of a body stop is the stop");
                } else {
                    asides += 1;
                    assert!(p <= page);
                }
                seen += 1;
            }
        }
        assert!(seen > 100, "only {seen} stops");
        assert_eq!(
            asides > 0,
            has_asides,
            "headers, footers and footnotes are hit-testable"
        );
    }
}

#[test]
fn a_click_left_of_a_line_lands_at_its_start_and_right_of_it_at_its_end() {
    let e = wrapped();
    let stops = e.stops(0);
    let (first, rect) = stops[0];
    let hit = e.hit_test(0, 0, rect.y + rect.height / 2).expect("hits");
    assert_eq!(hit, first);
    let width = e.page_size(0).expect("a page").0;
    let line: Vec<_> = stops.iter().filter(|(_, r)| r.y == rect.y).collect();
    let end = line.last().expect("a line").1;
    let hit = e
        .hit_test(0, width, rect.y + rect.height / 2)
        .expect("hits");
    assert_eq!(e.caret_rect(hit).expect("a caret").1, end);
}

#[test]
fn moving_down_through_a_short_line_keeps_the_goal_x() {
    let mut e = wrapped();
    let first: Vec<_> = e
        .stops(0)
        .into_iter()
        .filter(|(p, _)| block(&e, *p) == 0)
        .collect();
    let top = first[0].1.y;
    let line: Vec<_> = first.iter().filter(|(_, r)| r.y == top).collect();
    let (start, _) = *line[line.len() * 3 / 4];
    e.apply(cdoc_edit::Command::SetSelection(Selection::caret(start)))
        .expect("selects");
    let start_x = e.caret_rect(start).expect("a caret").1.x;
    let mut ys = Vec::new();
    loop {
        e.move_caret(Granularity::Line, Direction::Forward, false)
            .expect("moves");
        let (_, r) = e.caret_rect(e.selection().head).expect("a caret");
        ys.push(r.y);
        if block(&e, e.selection().head) == 2 {
            let gap = (r.x - start_x).abs();
            assert!(
                gap < 800,
                "the goal x drifted by {gap} after {} lines",
                ys.len()
            );
            break;
        }
        assert!(ys.len() < 10, "never reached the third paragraph");
    }
    assert!(
        ys.windows(2).all(|w| w[0] < w[1]),
        "each move goes down a line: {ys:?}"
    );
    e.move_caret(Granularity::Line, Direction::Backward, false)
        .expect("moves");
    let short = e.caret_rect(e.selection().head).expect("a caret").1;
    assert!(short.x < start_x, "the short line clamps the caret");
}

#[test]
fn a_selection_over_a_wrapped_paragraph_has_one_rect_per_line() {
    let mut e = wrapped();
    let (s, t) = (gap(&e, 0, 0), gap(&e, 0, LONG.chars().count()));
    select(&mut e, s, t);
    let rects = e.selection_rects(e.selection()).expect("rects");
    let lines = e
        .stops(0)
        .into_iter()
        .filter(|(p, _)| block(&e, *p) == 0)
        .map(|(_, r)| r.y)
        .collect::<std::collections::BTreeSet<_>>();
    assert!(lines.len() > 1, "the paragraph wraps");
    assert_eq!(rects.len(), lines.len());
    let width = e.page_size(0).expect("a page").0;
    for (page, r) in &rects {
        assert_eq!(*page, 0);
        assert!(r.width > 0 && r.x >= 0 && r.x + r.width <= width, "{r:?}");
    }
    assert!(rects.windows(2).all(|w| w[0].1.y < w[1].1.y));
    let collapsed = Selection::caret(e.selection().head);
    assert_eq!(e.selection_rects(collapsed).expect("rects"), Vec::new());
}

#[test]
fn line_boundaries_and_words_move_the_caret() {
    let mut e = wrapped();
    let g = gap(&e, 0, 4);
    caret(&mut e, g);
    e.move_caret(Granularity::LineBoundary, Direction::Forward, false)
        .expect("moves");
    let end = e.caret_rect(e.selection().head).expect("a caret").1;
    let row: Vec<_> = e
        .stops(0)
        .into_iter()
        .filter(|(_, r)| r.y == end.y)
        .collect();
    assert_eq!(row.last().expect("a line").1, end);
    e.move_caret(Granularity::LineBoundary, Direction::Backward, true)
        .expect("moves");
    let s = e.selection();
    assert_eq!(path(&e, s.head).offset, 0);
    assert!(!s.is_collapsed(e.doc()).expect("a selection"));
    let g = gap(&e, 0, 0);
    caret(&mut e, g);
    e.move_caret(Granularity::Word, Direction::Forward, false)
        .expect("moves");
    // Word-like: a word step lands at the start of the next word.
    assert_eq!(path(&e, e.selection().head).offset, 4);
}

/// A signature field's caption ("Sign here") once matched a paragraph holding "s" and an
/// image (the capital drawn for "s", the image taking the rest as if it were a field's
/// value). The match held only while nothing followed the image, so the page kept it after
/// typing there although its layout had not changed, and a fresh replica disagreed.
#[test]
fn a_widget_caption_is_not_matched_as_text_beside_an_image() {
    use common::ops::Op;
    let mut e = editor(&cdoc_model::test_support::fixtures::kitchen_sink(), 1);
    let ops = [
        Op::Select(25459, 61971),
        Op::Cmd(6, 0, 0),
        Op::Cmd(6, 0, 0),
        Op::Select(31243, 38869),
        Op::Cmd(27, 0, 0),
        Op::Cmd(1, 50390, 27886),
    ];
    for (k, o) in ops.iter().enumerate() {
        common::ops::apply(&mut e, o, k);
        let fresh = peer(&e, 2);
        for page in 0..e.page_count() {
            assert_eq!(
                e.stops(page),
                fresh.stops(page),
                "after op {k}, page {page}"
            );
        }
    }
}

proptest::proptest! {
    #![proptest_config(proptest::prelude::ProptestConfig { cases: 48, ..Default::default() })]

    /// Line maps kept across edits (only the pages the layout reports changed are derived
    /// again) equal the maps of a replica opened fresh on the result.
    #[test]
    fn incremental_line_maps_equal_fresh_ones(
        ops in proptest::collection::vec(common::ops::op(), 1..16),
    ) {
        let mut e = editor(&cdoc_model::test_support::fixtures::kitchen_sink(), 1);
        for (k, o) in ops.iter().enumerate() {
            common::ops::apply(&mut e, o, k);
        }
        let fresh = peer(&e, 2);
        proptest::prop_assert_eq!(e.page_count(), fresh.page_count());
        for page in 0..e.page_count() {
            proptest::prop_assert_eq!(e.stops(page), fresh.stops(page), "page {}", page);
        }
    }
}
