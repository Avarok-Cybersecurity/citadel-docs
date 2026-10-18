//! A block inserted into a section that had none: the relayout lays it on that section's
//! page, as a fresh layout does, not on the next section's.

mod support;

use cdoc_layout::{BlockEdit, Dirty, LayoutSession};
use cdoc_model::{Document, Length};

fn two_sections(first_blocks: usize) -> Document {
    let mut doc = support::doc_with(Vec::new());
    let mut second = doc.sections[0].clone();
    doc.sections[0].properties.page.size.width = Length(30000);
    doc.sections[0].properties.page.size.height = Length(30000);
    doc.sections[0].blocks = (0..first_blocks).map(|_| support::para("first")).collect();
    second.properties.page.size.width = Length(30000);
    second.properties.page.size.height = Length(60000);
    second.blocks = vec![support::para("second")];
    doc.sections.push(second);
    doc
}

#[test]
fn a_block_inserted_into_an_empty_section_is_laid_on_its_own_page() {
    let fonts = support::fonts();
    let options = support::options();
    let mut session = LayoutSession::new(&fonts, &options);
    let doc = two_sections(0);
    session.relayout(&doc, &Dirty::all()).expect("lays out");
    let edited = two_sections(1);
    let dirty = Dirty::edits(vec![BlockEdit::inserted(0, 0, 1)]);
    let got = session.relayout(&edited, &dirty).expect("relays out");
    let want = support::laid_out(&edited);
    let sizes = |l: &cdoc_layout::Layout| {
        l.pages
            .iter()
            .map(|p| (p.width, p.height))
            .collect::<Vec<_>>()
    };
    assert_eq!(sizes(got.layout()), sizes(&want));
    assert_eq!(got.layout(), &want);
}

/// Three sections, the middle one empty: its sheet is taken over by the third.
fn around_an_empty_section(first: &str) -> Document {
    let mut doc = support::doc_with(vec![support::para(first)]);
    let base = doc.sections[0].clone();
    let sized = |height: i32, blocks| {
        let mut s = base.clone();
        s.properties.page.size.width = Length(30000);
        s.properties.page.size.height = Length(height);
        s.blocks = blocks;
        s
    };
    doc.sections = vec![
        sized(30000, vec![support::para(first)]),
        sized(40000, Vec::new()),
        sized(60000, vec![support::para("third")]),
    ];
    doc
}

/// An edit that moves the end of the first section but not its pages is caught up at the
/// empty section's end; the sheet spliced there is the one the third section took over.
#[test]
fn a_relayout_caught_up_at_an_empty_section_keeps_the_page_the_next_section_took() {
    let fonts = support::fonts();
    let options = support::options();
    let mut session = LayoutSession::new(&fonts, &options);
    session
        .relayout(&around_an_empty_section("first"), &Dirty::all())
        .expect("lays out");
    let edited = around_an_empty_section(&"first ".repeat(12));
    let dirty = Dirty::edits(vec![BlockEdit::changed(0, 0)]);
    let got = session.relayout(&edited, &dirty).expect("relays out");
    let want = support::laid_out(&edited);
    let sizes = |l: &cdoc_layout::Layout| {
        l.pages
            .iter()
            .map(|p| (p.width, p.height))
            .collect::<Vec<_>>()
    };
    assert_eq!(sizes(got.layout()), sizes(&want));
    assert_eq!(got.layout(), &want);
}
