//! A long document: the kitchen sink's body repeated with prose, list items and its tables,
//! about a hundred pages, with its page break made a line break so that nothing stops a
//! reflow from running to the end. Every prose paragraph is numbered in its text, so no two
//! are alike and a full layout measures each one. The benchmark and the long-document tests
//! share it.

#![allow(dead_code, reason = "each target uses a different part")]

use cdoc_model::test_support::fixtures::kitchen_sink;
use cdoc_model::{Block, Document, Inline, NumId, NumberingRef, Paragraph, ParagraphProperties};
use cdoc_model::{RunProperties, StyleId};

/// How many times the body repeats.
pub const COPIES: usize = 120;

const PROSE: &str = "Citadel lays a document out the way a careful typesetter would, line by \
    line and page by page, measuring every word in the face the styles resolve and breaking \
    where the measure runs out, so that what the editor shows is what the file prints.";

fn paragraph(style: &str, numbering: Option<NumberingRef>, text: &str) -> Block {
    Block::Paragraph(Paragraph {
        properties: ParagraphProperties {
            style: Some(StyleId::new(style)),
            numbering,
            ..ParagraphProperties::none()
        },
        inlines: vec![Inline::text(text, RunProperties::none())],
    })
}

/// The long document.
#[must_use]
pub fn hundred_pages() -> Document {
    let mut doc = kitchen_sink();
    let mut body = doc.sections[0].blocks.clone();
    cdoc_model::visit_mut::paragraphs_in(&mut body, &mut |p| {
        for inline in &mut p.inlines {
            if let cdoc_model::InlineContent::Break { kind } = &mut inline.content {
                *kind = cdoc_model::BreakKind::Line;
            }
        }
    });
    let mut blocks = Vec::new();
    for copy in 0..COPIES {
        blocks.extend(body.iter().cloned());
        for item in 0..3 {
            let reference = NumberingRef {
                num_id: NumId(1),
                level: u8::try_from(item % 2).unwrap_or(0),
            };
            let text = format!("list item {copy}.{item}: {}", &PROSE[..60]);
            blocks.push(paragraph("ListParagraph", Some(reference), &text));
        }
        for k in 0..4 {
            blocks.push(paragraph("Normal", None, &format!("{copy}.{k}. {PROSE}")));
        }
    }
    doc.sections[0].blocks = blocks;
    doc
}

/// About a hundred pages of plain prose and list items, no headings: nothing absorbs a
/// reflow, so a line added at the top moves every page after it (the worst case).
#[must_use]
pub fn plain_prose() -> Document {
    let mut doc = hundred_pages();
    doc.sections[0].blocks = (0..1400)
        .map(|i| {
            let reference = NumberingRef {
                num_id: NumId(1),
                level: 0,
            };
            if i % 5 == 4 {
                paragraph(
                    "ListParagraph",
                    Some(reference),
                    &format!("{i} {}", &PROSE[..70]),
                )
            } else {
                paragraph("Normal", None, &format!("{i}. {PROSE}"))
            }
        })
        .collect();
    doc
}

/// What is typed at the end of the first paragraph of [`plain_prose`] to add lines that no
/// page's slack absorbs (a page left short by widow control takes up one line), so every
/// page after it moves: three lines and more.
#[must_use]
pub fn lines_at_the_top() -> String {
    " and more".repeat(36)
}

/// The index of a prose paragraph about halfway through the first section.
#[must_use]
pub fn middle(doc: &Document) -> usize {
    let blocks = &doc.sections[0].blocks;
    (blocks.len() / 2..blocks.len())
        .find(|i| text_of(&blocks[*i]).is_some_and(|t| t.contains(". Citadel")))
        .expect("prose in the second half")
}

fn text_of(block: &Block) -> Option<&str> {
    let Block::Paragraph(p) = block else {
        return None;
    };
    p.inlines.iter().find_map(|i| match &i.content {
        cdoc_model::InlineContent::Text(run) => Some(run.text.as_str()),
        _ => None,
    })
}

/// The document with `extra` appended to the first text run of block `index` of section 0.
#[must_use]
pub fn typed(doc: &Document, index: usize, extra: &str) -> Document {
    let mut doc = doc.clone();
    let Block::Paragraph(p) = &mut doc.sections[0].blocks[index] else {
        panic!("block {index} is not a paragraph");
    };
    let run = p
        .inlines
        .iter_mut()
        .find_map(|i| match &mut i.content {
            cdoc_model::InlineContent::Text(run) => Some(run),
            _ => None,
        })
        .expect("a text run");
    run.text.push_str(extra);
    doc
}
