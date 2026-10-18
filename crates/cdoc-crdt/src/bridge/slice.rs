//! Part of the sequence as model values: the inlines (with their formatting and marks) and the
//! structural atoms of a range of visible units, in order. [`to_model`](super::to_model) is the
//! same walk over everything, folded into a document; an editor reads a paragraph, a deleted
//! range or one changed block with it, without converting the whole document.

use super::build::Builder;
use super::resolve::Resolver;
use super::{Atom, BridgeError};
use crate::doc::Doc;
use crate::item::{byte_prefix, Content};
use cdoc_model::{Block, Inline, InlineContent, TextRun};
use std::ops::Range;

/// One element of a slice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Piece {
    /// Text (adjacent characters with equal formatting and marks merged) or an inline atom.
    Inline(Inline),
    /// A structural atom with its properties as last written.
    Block(Atom),
}

pub(crate) use super::fields::atom;

/// Calls `sink` with every piece of the visible units `range` (all of them for `None`).
pub(crate) fn walk(
    doc: &Doc,
    range: Option<Range<u64>>,
    sink: &mut dyn FnMut(Piece),
) -> Result<(), BridgeError> {
    let mut positions = vec![0u64; doc.seq.item_count()];
    let mut pos = 0u64;
    for idx in doc.seq.iter() {
        positions[idx as usize] = pos;
        pos += u64::from(doc.seq.item(idx).len);
    }
    let mut marks = Resolver::new(doc, &positions);
    let (from, to) = range.map_or((0, u64::MAX), |r| (r.start, r.end));
    let mut visible = 0u64;
    for idx in doc.seq.iter() {
        if visible >= to {
            break;
        }
        let item = doc.seq.item(idx);
        let len = item.weight().len;
        let (lo, hi) = (from.max(visible), to.min(visible + len));
        visible += len;
        if lo >= hi {
            continue;
        }
        let first = positions[idx as usize] + lo - (visible - len);
        match item.content {
            Content::Gone => {}
            Content::Text { start: at, bytes } => {
                let text = &doc.text[at as usize..(at + bytes) as usize];
                let skip = byte_prefix(text, usize::try_from(lo + len - visible).unwrap_or(0));
                let mut rest = &text[skip..];
                let end = first + (hi - lo);
                let mut u = first;
                while u < end {
                    marks.seek(u);
                    let until = marks.next_change().min(end);
                    let cut = byte_prefix(rest, usize::try_from(until - u).unwrap_or(usize::MAX));
                    let (properties, inline_marks) = marks.current()?;
                    sink(Piece::Inline(Inline {
                        content: InlineContent::Text(TextRun {
                            text: rest[..cut].to_owned(),
                            properties,
                        }),
                        marks: inline_marks,
                    }));
                    rest = &rest[cut..];
                    u = until;
                }
            }
            Content::Atom { index, kind } => match atom(doc, item.id, index, kind)? {
                Atom::Inline(content) => {
                    marks.seek(first);
                    let (_, inline_marks) = marks.current()?;
                    sink(Piece::Inline(Inline {
                        content,
                        marks: inline_marks,
                    }));
                }
                structural => sink(Piece::Block(structural)),
            },
        }
    }
    Ok(())
}

/// The pieces of the visible units `range`, adjacent text with equal formatting merged.
pub fn slice(doc: &Doc, range: Range<usize>) -> Result<Vec<Piece>, BridgeError> {
    let mut out: Vec<Piece> = Vec::new();
    walk(
        doc,
        Some(range.start as u64..range.end as u64),
        &mut |piece| {
            if let (Some(Piece::Inline(last)), Piece::Inline(next)) = (out.last_mut(), &piece) {
                if let (InlineContent::Text(a), InlineContent::Text(b)) =
                    (&mut last.content, &next.content)
                {
                    if a.properties == b.properties && last.marks == next.marks {
                        a.text.push_str(&b.text);
                        return;
                    }
                }
            }
            out.push(piece);
        },
    )?;
    Ok(out)
}

/// Pieces folded into blocks by the rules [`to_model`](super::to_model) folds a document by;
/// for a range that is a run of whole blocks of one container.
#[must_use]
pub fn fold_blocks(pieces: Vec<Piece>) -> Vec<Block> {
    let mut builder = Builder::new();
    for piece in pieces {
        match piece {
            Piece::Inline(inline) => builder.inline(inline),
            Piece::Block(atom) => builder.atom(atom),
        }
    }
    builder.finish_blocks()
}

/// The atom at visible unit `index`, or `None` if that unit is a character.
pub fn atom_at(doc: &Doc, index: usize) -> Result<Option<Atom>, BridgeError> {
    let Some((idx, _)) = doc.seq.find(index as u64) else {
        return Ok(None);
    };
    let item = doc.seq.item(idx);
    match item.content {
        Content::Atom { index, kind } => Ok(Some(atom(doc, item.id, index, kind)?)),
        _ => Ok(None),
    }
}
