//! Formatting as mark ranges: Peritext behaviour at range edges and under concurrency.

mod common;

use cdoc_crdt::{from_model, to_model, ClientId, Doc, Mark, RunKey};
use cdoc_model::{
    Block, CommentId, Document, Inline, InlineContent, Link, Paragraph, RunProperties,
};
use common::ROOM;

/// A blank document whose one paragraph holds `text`; the text starts at unit 2 (after the
/// section and paragraph atoms).
fn doc_with(text: &str, client: u64) -> Doc {
    let mut d = Document::blank();
    let Block::Paragraph(p) = &mut d.sections[0].blocks[0] else {
        unreachable!()
    };
    p.inlines.push(Inline::text(text, RunProperties::none()));
    from_model(&d, ClientId(client)).unwrap()
}

fn replica(of: &Doc, client: u64) -> Doc {
    Doc::decode_snapshot(&of.encode_snapshot(), ClientId(client)).unwrap()
}

fn sync(a: &mut Doc, b: &mut Doc) {
    let to_b = a.encode_diff(&b.state_vector());
    let to_a = b.encode_diff(&a.state_vector());
    b.apply_update(&to_b, ROOM).unwrap();
    a.apply_update(&to_a, ROOM).unwrap();
}

/// `(text, bold, link?, comments)` per inline of the first paragraph.
fn runs(d: &Doc) -> Vec<(String, Option<bool>, bool, Vec<String>)> {
    let m = to_model(d).unwrap();
    let Block::Paragraph(Paragraph { inlines, .. }) = &m.sections[0].blocks[0] else {
        unreachable!()
    };
    inlines
        .iter()
        .map(|i| {
            let InlineContent::Text(t) = &i.content else {
                unreachable!()
            };
            let comments = i
                .marks
                .comments
                .iter()
                .map(|c| c.as_str().to_owned())
                .collect();
            (
                t.text.clone(),
                t.properties.bold,
                i.marks.link.is_some(),
                comments,
            )
        })
        .collect()
}

fn bold(on: bool) -> Mark {
    Mark::Run(RunProperties {
        bold: Some(on),
        ..RunProperties::none()
    })
}

fn link() -> Mark {
    Mark::Link(Some(Link {
        href: "https://x.test".to_owned(),
        title: None,
    }))
}

fn r(
    text: &str,
    b: Option<bool>,
    l: bool,
    c: &[&str],
) -> (String, Option<bool>, bool, Vec<String>) {
    (
        text.to_owned(),
        b,
        l,
        c.iter().map(|s| (*s).to_owned()).collect(),
    )
}

#[test]
fn bold_grows_when_typing_at_its_end_but_not_at_its_start() {
    let mut d = doc_with("abc", 1);
    d.format(3..4, &bold(true)).unwrap();
    d.insert_text(4, "X").unwrap();
    d.insert_text(3, "Y").unwrap();
    assert_eq!(
        runs(&d),
        vec![
            r("aY", None, false, &[]),
            r("bX", Some(true), false, &[]),
            r("c", None, false, &[])
        ]
    );
}

#[test]
fn a_link_does_not_grow_at_either_end() {
    let mut d = doc_with("abc", 1);
    d.format(3..4, &link()).unwrap();
    d.insert_text(4, "X").unwrap();
    d.insert_text(3, "Y").unwrap();
    assert_eq!(
        runs(&d),
        vec![
            r("aY", None, false, &[]),
            r("b", None, true, &[]),
            r("Xc", None, false, &[])
        ]
    );
}

#[test]
fn text_inserted_concurrently_inside_a_bolded_range_is_bold() {
    let mut a = doc_with("hello world", 1);
    let mut b = replica(&a, 2);
    a.format(2..13, &bold(true)).unwrap();
    b.insert_text(7, " big").unwrap();
    sync(&mut a, &mut b);
    assert_eq!(runs(&a), vec![r("hello big world", Some(true), false, &[])]);
    assert_eq!(runs(&a), runs(&b));
}

#[test]
fn concurrent_bold_and_unbold_resolve_by_timestamp_then_client() {
    let mut a = doc_with("abcd", 1);
    let mut b = replica(&a, 2);
    a.format(2..6, &bold(true)).unwrap();
    b.format(3..5, &bold(false)).unwrap();
    sync(&mut a, &mut b);
    // Equal Lamport timestamps: client 2's operation wins where they overlap.
    let expected = vec![
        r("a", Some(true), false, &[]),
        r("bc", Some(false), false, &[]),
        r("d", Some(true), false, &[]),
    ];
    assert_eq!(runs(&a), expected);
    assert_eq!(runs(&b), expected);
    // A later edit that saw both wins outright.
    a.format(2..6, &Mark::ClearRun(RunKey::Bold)).unwrap();
    sync(&mut a, &mut b);
    assert_eq!(runs(&b), vec![r("abcd", None, false, &[])]);
}

#[test]
fn comments_keep_their_anchoring_order_and_can_be_removed() {
    let mut d = doc_with("abcdef", 1);
    let anchor = |id: &str, on| Mark::Comment {
        id: CommentId::new(id),
        anchored: on,
    };
    d.format(2..6, &anchor("c2", true)).unwrap();
    d.format(4..8, &anchor("c1", true)).unwrap();
    assert_eq!(
        runs(&d),
        vec![
            r("ab", None, false, &["c2"]),
            r("cd", None, false, &["c2", "c1"]),
            r("ef", None, false, &["c1"])
        ]
    );
    d.format(2..8, &anchor("c2", false)).unwrap();
    assert_eq!(
        runs(&d),
        vec![r("ab", None, false, &[]), r("cdef", None, false, &["c1"])]
    );
}

#[test]
fn a_range_over_deleted_text_still_formats_its_survivors() {
    let mut a = doc_with("abcdef", 1);
    let mut b = replica(&a, 2);
    a.delete(3, 2).unwrap();
    b.format(2..8, &bold(true)).unwrap();
    sync(&mut a, &mut b);
    assert_eq!(runs(&a), vec![r("adef", Some(true), false, &[])]);
    assert_eq!(runs(&a), runs(&b));
}

#[test]
fn formatting_needs_a_non_empty_range_and_a_value() {
    let mut d = doc_with("abc", 1);
    assert!(d.format(3..3, &bold(true)).is_err());
    assert!(d.format(3..9, &bold(true)).is_err());
    assert!(d.format(2..3, &Mark::Run(RunProperties::none())).is_err());
}
