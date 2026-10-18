use super::*;
use cdoc_pkg::xml::{parse, Whitespace};

/// What a reader makes of encoded text: serialise into a paragraph, parse, collapse.
fn round_trip(text: &str) -> (String, u32) {
    let mut children = Vec::new();
    let altered = Space::start().encode(text, &mut children);
    let mut p = Node::new("text:p");
    p.children = children;
    let (bytes, _) = cdoc_pkg::xml::serialize(&p);
    let back = parse(
        "t",
        &bytes,
        Whitespace::Keep,
        &cdoc_pkg::ReadLimits::generous(),
    )
    .expect("well-formed");
    let mut space = Space::start();
    let mut out = String::new();
    for child in &back.children {
        match child {
            Child::Text(t) => out.push_str(&space.collapse(t)),
            Child::Node(n) if n.name == "text:s" => {
                out.push_str(&spaces(n));
                space.white();
            }
            Child::Node(n) if n.name == "text:tab" => {
                out.push('\t');
                space.white();
            }
            Child::Node(n) => {
                assert_eq!(n.name, "text:line-break");
                out.push('\n');
                space.white();
            }
        }
    }
    (out, altered)
}

#[test]
fn every_space_survives_collapsing() {
    for text in [
        "a b",
        "  lead",
        "trail ",
        "a  b",
        "a   b ",
        " ",
        "   ",
        "x \u{e9}  \u{1F600} y",
        "a b c",
    ] {
        assert_eq!(round_trip(text), (text.to_owned(), 0), "{text:?}");
    }
}

#[test]
fn tabs_and_line_feeds_become_elements_and_are_counted() {
    assert_eq!(round_trip("a\tb\nc"), ("a\tb\nc".to_owned(), 2));
    assert_eq!(round_trip("a\r\nb"), ("a\nb".to_owned(), 2));
}

#[test]
fn literal_white_space_collapses_as_odf_says() {
    let mut s = Space::start();
    assert_eq!(s.collapse("  a \n\t b  "), "a b ");
    assert_eq!(
        s.collapse(" c"),
        "c",
        "a space after a space is dropped across chunks"
    );
    s.visible();
    assert_eq!(s.collapse(" d"), " d");
}

#[test]
fn space_counts_are_bounded() {
    let n = Node::new("text:s").attr("text:c", "999999999");
    assert_eq!(spaces(&n).len(), 10_000);
    assert_eq!(spaces(&Node::new("text:s")), " ");
}
