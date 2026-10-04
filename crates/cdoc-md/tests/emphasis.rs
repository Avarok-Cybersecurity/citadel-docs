//! The emphasis cases that broke while the writer was being built, kept as fixed examples: the
//! property test finds them by luck, these find them every time.

mod common;

use cdoc_md::{document_to_md, md_to_document};
use cdoc_model::{Block, Document, Inline, RunProperties};

/// A run with the formatting named by `flags`: `b`old, `i`talic, `s`trike, `c`ode.
fn run(text: &str, flags: &str) -> Inline {
    let has = |c: char| flags.contains(c);
    Inline::text(
        text,
        RunProperties {
            bold: has('b').then_some(true),
            italic: has('i').then_some(true),
            strike: has('s').then_some(true),
            font: has('c').then(|| cdoc_md::CODE_FONT.to_owned()),
            ..RunProperties::none()
        },
    )
}

fn plain(text: &str) -> Inline {
    run(text, "")
}

fn paragraph(inlines: Vec<Inline>) -> Document {
    let mut doc = Document::blank();
    let Block::Paragraph(p) = &mut doc.sections[0].blocks[0] else {
        unreachable!()
    };
    p.inlines = inlines;
    doc
}

/// Write, read, and require the same meaning; returns the Markdown.
fn survives(inlines: Vec<Inline>) -> String {
    let doc = paragraph(inlines);
    let (md, _) = document_to_md(&doc);
    let (back, _) = md_to_document(&md, &common::options());
    assert_eq!(
        common::project(&back),
        common::project(&doc),
        "markdown was {md:?}"
    );
    md
}

#[test]
fn strikethrough_beside_bold_after_a_letter_is_written_as_a_tag() {
    let md = survives(vec![plain("V"), run("0,.<8", "bs")]);
    assert!(md.contains("<del>"), "{md}");
}

#[test]
fn emphasis_that_changes_without_a_gap_is_written_as_tags() {
    let md = survives(vec![run("ab", "b"), run("cd", "i")]);
    assert_eq!(md, "<strong>ab</strong><em>cd</em>\n");
    survives(vec![run("7>c", "i"), run("7&*.p", "b"), run("6)&v", "bi")]);
}

#[test]
fn spans_that_open_together_but_close_apart_are_written_as_tags() {
    let md = survives(vec![run("j7", "bi"), run("5a", "b"), run("o7", "bi")]);
    assert!(md.contains("<strong>") && md.contains("<em>"), "{md}");
}

#[test]
fn spans_that_open_and_close_together_use_plain_delimiters() {
    assert_eq!(survives(vec![run("both", "bi")]), "***both***\n");
    assert_eq!(
        survives(vec![plain("a"), run("b", "b"), plain("c")]),
        "a**b**c\n",
        "intraword emphasis is fine for the star delimiter"
    );
}

#[test]
fn adjacent_code_runs_are_one_code_span() {
    let md = survives(vec![run("0a", "c"), run("aa", "c")]);
    assert_eq!(md, "`0aaa`\n");
}

#[test]
fn a_trailing_space_is_kept_as_an_entity() {
    assert_eq!(survives(vec![plain("e ")]), "e&#32;\n");
    assert_eq!(survives(vec![plain("  ")]), "&#32;&#32;\n");
}
