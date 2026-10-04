//! The loss report counts and merges.

use cdoc_model::Report;

#[test]
fn occurrences_are_counted_per_part_and_kind() {
    let mut r = Report::empty();
    assert!(r.is_lossless());
    r.note("word/document.xml", "w:bookmarkStart");
    r.note("word/document.xml", "w:bookmarkStart");
    r.note("word/styles.xml", "w:bookmarkStart");
    r.note_n("word/document.xml", "w:smartTag", 3);
    r.note_n("word/document.xml", "never", 0);
    assert!(!r.is_lossless());
    assert_eq!(r.losses.len(), 3, "zero occurrences make no entry");
    assert_eq!(r.count_of("w:bookmarkStart"), 3, "over every part");
    assert_eq!(r.count_of("w:smartTag"), 3);
    assert_eq!(r.count_of("never"), 0);
}

#[test]
fn absorbing_adds_counts_and_keeps_first_seen_order() {
    let mut a = Report::empty();
    a.note_n("p", "x", 2);
    let mut b = Report::empty();
    b.note("p", "y");
    b.note("p", "x");
    a.absorb(b);
    let seen: Vec<(&str, u32)> = a
        .losses
        .iter()
        .map(|l| (l.what.as_str(), l.count))
        .collect();
    assert_eq!(seen, [("x", 3), ("y", 1)]);
}

#[test]
fn it_serialises_for_the_facade() {
    let mut r = Report::empty();
    r.note("markdown", "underline");
    assert_eq!(
        serde_json::to_string(&r).unwrap(),
        r#"{"losses":[{"part":"markdown","what":"underline","count":1}]}"#
    );
}
