//! Whitespace Markdown cannot hold at a line's edges. A tab or four spaces at the start of a
//! paragraph would open an indented code block, so they become one space and the loss is
//! reported; one to three spaces are kept as `&#32;`. A tab inside a line is a literal tab,
//! never the `&#9;` entity. A trailing tab, which a reader strips, is dropped and reported.

use cdoc_md::document_to_md;
use cdoc_model::{Block, BreakKind, Document, Inline, InlineContent, RunProperties};

const LEADING: &str = "leading tab dropped: Markdown has no leading tab";
const TRAILING: &str = "trailing tab dropped: Markdown strips trailing whitespace";

fn plain(text: &str) -> Inline {
    Inline::text(text, RunProperties::none())
}

/// The Markdown for one paragraph of `text`, with how often each loss was reported.
fn write(text: &str) -> (String, u32, u32) {
    write_inlines(vec![plain(text)])
}

fn write_inlines(inlines: Vec<Inline>) -> (String, u32, u32) {
    let mut doc = Document::blank();
    let Block::Paragraph(p) = &mut doc.sections[0].blocks[0] else {
        unreachable!()
    };
    p.inlines = inlines;
    let (md, report) = document_to_md(&doc);
    let (lead, trail) = (report.count_of(LEADING), report.count_of(TRAILING));
    (md, lead, trail)
}

#[test]
fn a_leading_tab_or_four_spaces_become_one_reported_space() {
    assert_eq!(write("\tx"), (" x\n".to_owned(), 1, 0));
    assert_eq!(write("    x"), (" x\n".to_owned(), 1, 0));
    assert_eq!(write(" \t x"), (" x\n".to_owned(), 1, 0));
}

#[test]
fn up_to_three_leading_spaces_are_kept() {
    assert_eq!(write("  x"), ("&#32;&#32;x\n".to_owned(), 0, 0));
}

#[test]
fn an_inner_tab_is_literal_and_a_trailing_tab_is_reported() {
    assert_eq!(write("a\tb"), ("a\tb\n".to_owned(), 0, 0));
    assert_eq!(write("a\t"), ("a\n".to_owned(), 0, 1));
}

#[test]
fn a_line_after_a_hard_break_is_a_line_start_too() {
    let br = Inline::plain(InlineContent::Break {
        kind: BreakKind::Line,
    });
    let written = write_inlines(vec![plain("a"), br, plain("\tb")]);
    assert_eq!(written, ("a\\\n b\n".to_owned(), 1, 0));
}
