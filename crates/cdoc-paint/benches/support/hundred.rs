//! The benchmarks' 100-page document: a fixed pseudo-random sequence over a small vocabulary,
//! laid out with Knuth-Plass until it reaches 100 pages (102). Shared by the criterion bench and
//! the `size_breakdown` example, so both measure the same list.

use cdoc_layout::{layout, BreakerChoice, Layout, LayoutOptions};
use cdoc_model::{Block, Document, Inline, Paragraph, RunProperties};

const WORDS: [&str; 16] = [
    "the",
    "display",
    "list",
    "is",
    "one",
    "description",
    "of",
    "every",
    "page",
    "painted",
    "by",
    "canvas",
    "preview",
    "and",
    "file",
    "alike",
];

pub fn document(paragraphs: usize) -> Document {
    let mut doc = Document::blank();
    let template = match &doc.sections[0].blocks[0] {
        Block::Paragraph(p) => p.properties.clone(),
        Block::Table(_) => unreachable!("a blank document starts with a paragraph"),
    };
    let mut state: u64 = 0x2545_F491_4F6C_DD1D;
    let mut word = || {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        WORDS[usize::try_from(state >> 60).unwrap_or(0)]
    };
    doc.sections[0].blocks = (0..paragraphs)
        .map(|_| {
            let text: Vec<&str> = (0..60).map(|_| word()).collect();
            Block::Paragraph(Paragraph {
                properties: template.clone(),
                inlines: vec![Inline::text(text.join(" "), RunProperties::none())],
            })
        })
        .collect();
    doc
}

pub fn hundred_pages(fonts: &cdoc_layout::FontLibrary) -> Layout {
    let mut paragraphs = 400;
    loop {
        let laid = layout(
            &document(paragraphs),
            fonts,
            &LayoutOptions::word_like(BreakerChoice::KnuthPlass(cdoc_layout::Params::word_like())),
        )
        .unwrap();
        if laid.pages.len() >= 100 {
            return laid;
        }
        paragraphs += 40;
    }
}
