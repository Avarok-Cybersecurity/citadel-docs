//! The viewport: once the UI names the pages on screen, a result carries the selection's
//! rectangles on those and the dirty pages only, and `selection_rects_on` serves any other
//! page on request (the rects record).

mod common;

use cdoc_edit::wire::{decode_rects, encode_rects, Request};
use cdoc_edit::Command;
use common::{editor, paragraphs};
use std::collections::BTreeSet;

fn long() -> cdoc_edit::Editor<'static> {
    let line = "word ".repeat(200);
    let lines: Vec<&str> = (0..40).map(|_| line.as_str()).collect();
    let mut e = editor(&paragraphs(&lines), 1);
    assert!(e.page_count() >= 4, "{} pages", e.page_count());
    e.apply(Command::SelectAll).unwrap();
    e
}

fn pages(rects: &[(usize, cdoc_layout::Rect)]) -> BTreeSet<usize> {
    rects.iter().map(|(p, _)| *p).collect()
}

#[test]
fn without_a_viewport_every_page_has_its_rects() {
    let e = long();
    let o = e.outcome(Vec::new(), Vec::new(), Vec::new()).unwrap();
    assert_eq!(pages(&o.rects), (0..e.page_count()).collect());
}

#[test]
fn a_viewport_limits_the_rects_to_its_pages_and_the_dirty_ones() {
    let mut e = long();
    let all = e.outcome(Vec::new(), Vec::new(), Vec::new()).unwrap().rects;
    let applied = e.handle(Request::SetViewport(vec![1, 2])).unwrap();
    assert!(applied.update.is_empty(), "no document change");
    let o = e.outcome(Vec::new(), vec![3], Vec::new()).unwrap();
    assert_eq!(pages(&o.rects), BTreeSet::from([1, 2, 3]));
    let kept: Vec<_> = all.iter().filter(|(p, _)| (1..=3).contains(p)).collect();
    assert_eq!(o.rects.iter().collect::<Vec<_>>(), kept);
    assert!(
        e.undo().unwrap().is_none(),
        "setting the viewport is no undo step"
    );
}

#[test]
fn a_page_off_screen_is_served_on_request() {
    let mut e = long();
    let all = e.outcome(Vec::new(), Vec::new(), Vec::new()).unwrap().rects;
    e.handle(Request::SetViewport(vec![0])).unwrap();
    let on_two = e.selection_rects_on(2).unwrap();
    let want: Vec<_> = all
        .iter()
        .filter(|(p, _)| *p == 2)
        .map(|(_, r)| *r)
        .collect();
    assert_ne!(want.len(), 0, "the selection reaches page 2");
    assert_eq!(on_two, want);
    let bytes = encode_rects(&on_two);
    assert_eq!(decode_rects(&bytes).unwrap(), on_two);
    for n in 0..bytes.len() {
        assert!(decode_rects(&bytes[..n]).is_err(), "truncated at {n}");
    }
    let mut bad = bytes.clone();
    bad[3] = 2;
    assert!(decode_rects(&bad).is_err(), "another version");
}
