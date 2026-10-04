//! Normal form: adjacent text inlines with identical properties and marks are one inline.
//! `ProseMirror` merges such text nodes on every transaction, so a document that has been
//! through the editor is already normal; a document assembled by an importer may not be.
//! The mapping to and from `ProseMirror` is lossless on any document, normal or not; this exists
//! so two documents that render identically compare equal.

use crate::block::Block;
use crate::inline::{Inline, InlineContent};
use crate::paragraph::Paragraph;
use crate::table::Table;
use crate::Document;

impl Document {
    /// Merge adjacent identical text inlines everywhere. Idempotent.
    pub fn normalize(&mut self) {
        for section in &mut self.sections {
            normalize_blocks(&mut section.blocks);
            for set in [
                &mut section.properties.headers,
                &mut section.properties.footers,
            ] {
                for hf in [&mut set.default, &mut set.first, &mut set.even]
                    .into_iter()
                    .flatten()
                {
                    normalize_blocks(&mut hf.blocks);
                }
            }
        }
        for footnote in &mut self.footnotes {
            normalize_blocks(&mut footnote.blocks);
        }
    }

    /// Whether [`normalize`](Self::normalize) would change nothing.
    #[must_use]
    pub fn is_normalized(&self) -> bool {
        self.paragraphs().all(|p| {
            p.inlines
                .windows(2)
                .all(|pair| !mergeable(&pair[0], &pair[1]))
        })
    }
}

fn normalize_blocks(blocks: &mut [Block]) {
    for block in blocks {
        match block {
            Block::Paragraph(p) => normalize_paragraph(p),
            Block::Table(t) => normalize_table(t),
        }
    }
}

fn normalize_table(table: &mut Table) {
    for row in &mut table.rows {
        for cell in &mut row.cells {
            normalize_blocks(&mut cell.blocks);
        }
    }
}

fn normalize_paragraph(paragraph: &mut Paragraph) {
    let mut merged: Vec<Inline> = Vec::with_capacity(paragraph.inlines.len());
    for inline in paragraph.inlines.drain(..) {
        match merged.last_mut() {
            Some(last) if mergeable(last, &inline) => {
                if let (InlineContent::Text(a), InlineContent::Text(b)) =
                    (&mut last.content, &inline.content)
                {
                    a.text.push_str(&b.text);
                }
            }
            _ => merged.push(inline),
        }
    }
    paragraph.inlines = merged;
}

fn mergeable(a: &Inline, b: &Inline) -> bool {
    match (&a.content, &b.content) {
        (InlineContent::Text(ra), InlineContent::Text(rb)) => {
            ra.properties == rb.properties && a.marks == b.marks
        }
        _ => false,
    }
}
