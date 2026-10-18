//! Lists that restart: an ordered list right after another one counts from its own start. In
//! CommonMark adjacent items with the same delimiter are one list whatever their numbers say,
//! so the writer switches between `.` and `)` at a restart, and reading that back gives two
//! lists again; writing it a second time changes nothing.

mod common;

use cdoc_md::{document_to_md, md_to_document};
use cdoc_model::{
    AbstractNumId, AbstractNumbering, Alignment, Block, Document, Inline, Level, NumId,
    NumberFormat, NumberingInstance, NumberingRef, Paragraph, ParagraphProperties, RunProperties,
};

fn decimal() -> AbstractNumbering {
    let level = Level {
        start: 1,
        format: NumberFormat::Decimal,
        text: "%1.".to_owned(),
        alignment: Alignment::Left,
        indent: None,
        run: None,
    };
    AbstractNumbering {
        id: AbstractNumId(1),
        levels: vec![level.clone(), level],
    }
}

fn item(num: u32, level: u8, text: &str) -> Block {
    Block::Paragraph(Paragraph {
        properties: ParagraphProperties {
            numbering: Some(NumberingRef {
                num_id: NumId(num),
                level,
            }),
            ..ParagraphProperties::none()
        },
        inlines: vec![Inline::text(text, RunProperties::none())],
    })
}

fn document(blocks: Vec<Block>) -> Document {
    let mut doc = Document::blank();
    doc.numbering.definitions = vec![decimal()];
    doc.numbering.instances = (1..=3)
        .map(|id| NumberingInstance {
            id: NumId(id),
            abstract_id: AbstractNumId(1),
            overrides: vec![],
        })
        .collect();
    doc.sections[0].blocks = blocks;
    doc
}

#[test]
fn an_ordered_list_after_another_restarts_with_the_other_delimiter() {
    let doc = document(vec![
        item(1, 0, "a"),
        item(1, 0, "b"),
        item(2, 0, "c"),
        item(2, 1, "c.1"),
        item(3, 1, "c.2"),
        item(2, 0, "d"),
        item(3, 0, "e"),
    ]);
    let (md, _) = document_to_md(&doc);
    assert_eq!(md, "1. a\n2. b\n1) c\n    1. c.1\n    1) c.2\n2) d\n1. e\n");
    let (back, _) = md_to_document(&md, &common::options());
    let lists: Vec<NumId> = back
        .paragraphs()
        .filter(|p| p.properties.numbering.is_some_and(|n| n.level == 0))
        .map(|p| p.properties.numbering.expect("numbered").num_id)
        .collect();
    assert_eq!(lists.len(), 5);
    assert_ne!(lists[1], lists[2], "the restart is a list of its own");
    assert_ne!(lists[3], lists[4]);
    assert_eq!(document_to_md(&back).0, md, "second write differs");
}

#[test]
fn one_list_keeps_one_delimiter_across_its_nested_items() {
    let doc = document(vec![
        item(1, 0, "a"),
        item(1, 1, "a.1"),
        item(1, 0, "b"),
        item(1, 1, "b.1"),
    ]);
    assert_eq!(
        document_to_md(&doc).0,
        "1. a\n    1. a.1\n2. b\n    1. b.1\n"
    );
}
