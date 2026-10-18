//! Boundaries in the text: the paragraph a gap is in, clusters (a character with the combining
//! marks after it) and words, as Word's caret and deletion keys see them.

use crate::error::EditError;
use crate::outline::Outline;
use crate::position::{caret_gap, is_caret_gap, Direction};
use cdoc_crdt::{AtomKind, Doc, Unit};
use std::ops::Range;

/// The paragraph holding caret gap `gap`: its node and its content gaps (start..end).
pub(crate) fn paragraph(
    doc: &Doc,
    outline: &Outline,
    gap: usize,
) -> Result<(usize, Range<usize>), EditError> {
    let node = doc
        .blocks_before(gap)
        .checked_sub(1)
        .filter(|&n| {
            outline
                .nodes
                .get(n)
                .is_some_and(|n| n.kind == AtomKind::Paragraph)
        })
        .ok_or(EditError::NotApplicable(
            "the position is not in a paragraph",
        ))?;
    let start = doc
        .locate(outline.nodes[node].id)
        .ok_or(EditError::UnknownPosition)?
        .index
        + 1;
    let end = outline
        .nodes
        .get(node + 1)
        .and_then(|n| doc.locate(n.id))
        .map_or(doc.len(), |l| l.index);
    Ok((node, start..end))
}

/// Whether `c` joins the character before it into one cluster (combining marks, joiners,
/// variation selectors). A table-free approximation of grapheme clusters.
pub(crate) fn extends(c: char) -> bool {
    matches!(c as u32,
        0x0300..=0x036F | 0x1AB0..=0x1AFF | 0x1DC0..=0x1DFF | 0x20D0..=0x20FF
        | 0xFE00..=0xFE0F | 0xFE20..=0xFE2F | 0x200C | 0x200D | 0xE0100..=0xE01EF)
}

fn char_at(doc: &Doc, index: usize) -> Option<char> {
    match doc.unit_at(index)? {
        Unit::Char(c) => Some(c),
        Unit::Atom(_) => None,
    }
}

/// The next caret gap from `gap` one cluster in `dir`; `gap` itself at the document's ends.
pub(crate) fn step_cluster(doc: &Doc, gap: usize, dir: Direction) -> Result<usize, EditError> {
    match dir {
        Direction::Forward => {
            let mut g = gap + 1;
            while g < doc.len() && char_at(doc, g).is_some_and(extends) {
                g += 1;
            }
            while g <= doc.len() && !is_caret_gap(doc, g) {
                g += 1;
            }
            Ok(if g > doc.len() { gap } else { g })
        }
        Direction::Backward => {
            let mut g = gap.saturating_sub(1);
            while g > 0 && char_at(doc, g).is_some_and(extends) {
                g -= 1;
            }
            while g > 0 && !is_caret_gap(doc, g) {
                g -= 1;
            }
            if g == 0 {
                return caret_gap(doc, gap);
            }
            Ok(g)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    Word,
    Space,
    Other,
}

fn class(c: char) -> Class {
    if c.is_alphanumeric() || c == '_' || c == '\'' || c == '\u{2019}' || extends(c) {
        Class::Word
    } else if c.is_whitespace() {
        Class::Space
    } else {
        Class::Other
    }
}

/// Whether `c` continues a word for undo grouping.
pub(crate) fn is_word_char(c: char) -> bool {
    class(c) == Class::Word
}

/// The gap a word move or deletion reaches from `gap` within `para` (Word's Ctrl+arrow:
/// backward to the start of the word before; forward past the word and the spaces after it).
pub(crate) fn word_boundary(doc: &Doc, gap: usize, para: &Range<usize>, dir: Direction) -> usize {
    let unit_class = |i: usize| char_at(doc, i).map_or(Class::Other, class);
    match dir {
        Direction::Backward => {
            let mut g = gap;
            while g > para.start && unit_class(g - 1) == Class::Space {
                g -= 1;
            }
            let kind = if g > para.start {
                unit_class(g - 1)
            } else {
                return g;
            };
            while g > para.start && unit_class(g - 1) == kind {
                g -= 1;
            }
            g
        }
        Direction::Forward => {
            let mut g = gap;
            if g < para.end {
                let kind = unit_class(g);
                while g < para.end && unit_class(g) == kind && kind != Class::Space {
                    g += 1;
                }
            }
            while g < para.end && unit_class(g) == Class::Space {
                g += 1;
            }
            g
        }
    }
}

/// The word around `gap` (for a double click): the run of one class that holds it.
pub(crate) fn word_at(doc: &Doc, gap: usize, para: &Range<usize>) -> Range<usize> {
    let unit_class = |i: usize| char_at(doc, i).map_or(Class::Other, class);
    let probe = if gap < para.end {
        gap
    } else if gap > para.start {
        gap - 1
    } else {
        return gap..gap;
    };
    let kind = unit_class(probe);
    let (mut a, mut b) = (probe, probe + 1);
    while a > para.start && unit_class(a - 1) == kind {
        a -= 1;
    }
    while b < para.end && unit_class(b) == kind {
        b += 1;
    }
    a..b
}
