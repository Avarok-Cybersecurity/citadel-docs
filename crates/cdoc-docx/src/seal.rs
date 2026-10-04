//! The empty paragraph OOXML needs after a table. A cell, header, footer, footnote or section
//! whose content ends in a table must still end in a paragraph, so the writer appends an empty
//! one; the reader removes exactly one. Both sides share this rule so they cannot disagree.

use cdoc_model::Block;

/// A paragraph with no properties and no content: what a bare `<w:p/>` reads as.
#[must_use]
pub fn is_bare(block: &Block) -> bool {
    matches!(block, Block::Paragraph(p) if p.properties.is_none() && p.inlines.is_empty())
}

/// Whether the content ends in a table followed by nothing but bare paragraphs (possibly
/// none). Such content is written with one more bare paragraph, so a model that really ends
/// `[table, bare paragraph]` comes back with its paragraph.
#[must_use]
pub fn is_sealed(blocks: &[Block]) -> bool {
    blocks
        .iter()
        .rposition(|b| matches!(b, Block::Table(_)))
        .is_some_and(|at| blocks[at + 1..].iter().all(is_bare))
}

/// Remove the paragraph the writer added, if there is one.
pub fn unseal(blocks: &mut Vec<Block>) {
    if is_sealed(blocks) && blocks.last().is_some_and(is_bare) {
        blocks.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cdoc_model::{Inline, Paragraph, ParagraphProperties, RunProperties};

    fn bare() -> Block {
        Block::Paragraph(Paragraph {
            properties: ParagraphProperties::none(),
            inlines: Vec::new(),
        })
    }

    fn text() -> Block {
        Block::Paragraph(Paragraph {
            properties: ParagraphProperties::none(),
            inlines: vec![Inline::text("x", RunProperties::none())],
        })
    }

    fn table() -> Block {
        let doc = cdoc_model::test_support::fixtures::kitchen_sink();
        let found = doc.sections[0]
            .blocks
            .iter()
            .find_map(|b| match b {
                Block::Table(t) => Some(t.clone()),
                Block::Paragraph(_) => None,
            })
            .expect("the kitchen sink has a table");
        Block::Table(found)
    }

    #[test]
    fn content_ending_in_a_table_is_sealed_whether_or_not_bare_paragraphs_follow() {
        assert!(is_sealed(&[text(), table()]));
        assert!(is_sealed(&[table(), bare()]));
        assert!(is_sealed(&[table(), bare(), bare()]));
        assert!(!is_sealed(&[table(), text()]));
        assert!(!is_sealed(&[text(), bare()]));
        assert!(!is_sealed(&[]));
    }

    #[test]
    fn unsealing_removes_exactly_the_paragraph_the_writer_added() {
        let mut ends_in_table = vec![table(), bare()];
        unseal(&mut ends_in_table);
        assert_eq!(ends_in_table, [table()]);
        let mut real_paragraph_after_table = vec![table(), bare(), bare()];
        unseal(&mut real_paragraph_after_table);
        assert_eq!(real_paragraph_after_table, [table(), bare()]);
        let mut nothing_to_remove = vec![text(), bare()];
        unseal(&mut nothing_to_remove);
        assert_eq!(nothing_to_remove, [text(), bare()]);
        let mut bare_table_end = vec![table()];
        unseal(&mut bare_table_end);
        assert_eq!(bare_table_end, [table()]);
    }
}
