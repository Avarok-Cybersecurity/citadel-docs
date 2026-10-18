//! Part of the sequence as model values: the inlines (with their formatting and marks) and the
//! structural atoms of a range of visible units, in order. [`to_model`](super::to_model) is the
//! same walk over everything, folded into a document; an editor reads a paragraph, a deleted
//! range or one changed block with it, without converting the whole document.

use super::build::Builder;
use super::resolve::Resolver;
use super::{Atom, BridgeError};
use crate::doc::Doc;
use crate::item::{byte_prefix, Content};
use cdoc_model::{Block, Inline, InlineContent, Marks, RunProperties, TextRun};
use std::ops::Range;

/// One element of a slice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Piece {
    /// Text (adjacent characters with equal formatting and marks merged) or an inline atom.
    Inline(Inline),
    /// A structural atom with its properties as last written.
    Block(Atom),
}

use super::atoms::{Atoms, Decoded};
pub(crate) use super::fields::atom;
use std::rc::Rc;

/// The formatting of the characters a walk reports, resolved as far as the visitor asks.
pub(crate) struct Formatting<'r, 'd>(&'r mut Resolver<'d>);

impl Formatting<'_, '_> {
    /// Their character properties.
    pub(crate) fn run(&mut self) -> Result<&RunProperties, BridgeError> {
        self.0.current_run()
    }

    /// Their character properties and marks.
    pub(crate) fn all(&mut self) -> Result<&(RunProperties, Marks), BridgeError> {
        self.0.current()
    }
}

/// What a walk reports, in document order.
pub(crate) trait Visit {
    /// Characters, with their formatting.
    fn text(&mut self, text: &str, formatting: Formatting<'_, '_>) -> Result<(), BridgeError>;
    /// Whether the visitor reads inline atoms: when not, they are neither decoded nor
    /// resolved, only stepped over.
    fn reads_inlines(&self) -> bool;
    /// An inline atom with its marks.
    fn inline(&mut self, inline: Inline);
    /// A structural atom; atoms with equal properties share one value.
    fn structural(&mut self, atom: &Rc<Atom>);
}

struct Pieces<'s>(&'s mut dyn FnMut(Piece));

impl Visit for Pieces<'_> {
    fn text(&mut self, text: &str, mut formatting: Formatting<'_, '_>) -> Result<(), BridgeError> {
        let (properties, marks) = formatting.all()?;
        (self.0)(Piece::Inline(Inline {
            content: InlineContent::Text(TextRun {
                text: text.to_owned(),
                properties: properties.clone(),
            }),
            marks: marks.clone(),
        }));
        Ok(())
    }

    fn reads_inlines(&self) -> bool {
        true
    }

    fn inline(&mut self, inline: Inline) {
        (self.0)(Piece::Inline(inline));
    }

    fn structural(&mut self, atom: &Rc<Atom>) {
        (self.0)(Piece::Block(Atom::clone(atom)));
    }
}

/// Calls `sink` with every piece of the visible units `range` (all of them for `None`).
pub(crate) fn walk(
    doc: &Doc,
    range: Option<Range<u64>>,
    sink: &mut dyn FnMut(Piece),
) -> Result<(), BridgeError> {
    visit(doc, range, &mut Pieces(sink))
}

/// Reports every piece of the visible units `range` (all of them for `None`) to `visitor`.
pub(crate) fn visit(
    doc: &Doc,
    range: Option<Range<u64>>,
    visitor: &mut dyn Visit,
) -> Result<(), BridgeError> {
    let mut positions = vec![0u64; doc.seq.item_count()];
    let mut pos = 0u64;
    for idx in doc.seq.iter() {
        positions[idx as usize] = pos;
        pos += u64::from(doc.seq.item(idx).len);
    }
    let mut marks = Resolver::new(doc, &positions)?;
    let mut atoms = Atoms::new(doc);
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
                    visitor.text(&rest[..cut], Formatting(&mut marks))?;
                    rest = &rest[cut..];
                    u = until;
                }
            }
            Content::Atom { kind, .. } if !kind.is_structural() && !visitor.reads_inlines() => {}
            Content::Atom { index, kind } => match atoms.get(item.id, index, kind)? {
                Decoded::Inline(content) => {
                    marks.seek(first);
                    let marks = marks.current()?.1.clone();
                    visitor.inline(Inline { content, marks });
                }
                Decoded::Structural(atom) => visitor.structural(&atom),
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
