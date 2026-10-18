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
