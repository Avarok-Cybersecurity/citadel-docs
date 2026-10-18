//! Footnote bodies are never deleted by undo. Undoing a footnote's insertion removes its
//! reference and leaves the body unreferenced, as deleting the reference does; redo restores
//! the reference to the same body. Were the body deleted, a peer concurrently restoring the
//! reference (undoing their own deletion of it) would leave a reference with no body, and a
//! body restored by copy on both peers would exist twice.

use crate::error::EditError;
use crate::outline::Outline;
use cdoc_crdt::{AtomKind, Doc, Unit};
use std::ops::Range;

/// `range` without the footnote bodies that start inside it, as ascending ranges.
pub(crate) fn outside_bodies(
    doc: &Doc,
    range: Range<usize>,
) -> Result<Vec<Range<usize>>, EditError> {
    let starts = doc
        .units_from(range.start)
        .take(range.len())
        .any(|(_, u)| u == Unit::Atom(AtomKind::Footnote));
    if !starts {
        return Ok(vec![range]);
    }
    let outline = Outline::build(&doc.structure());
    let mut out = Vec::new();
    let mut from = range.start;
    for node in outline
        .nodes
        .iter()
        .filter(|n| n.kind == AtomKind::Footnote)
    {
        let at = doc.locate(node.id).ok_or(EditError::UnknownPosition)?;
        if !at.visible || at.index < from || at.index >= range.end {
            continue;
        }
        let end = node.end.ok_or(EditError::Irregular)?;
        let last = doc
            .locate(outline.nodes[end].id)
            .ok_or(EditError::UnknownPosition)?;
        if from < at.index {
            out.push(from..at.index);
        }
        from = (last.index + 1).min(range.end).max(at.index + 1);
    }
    if from < range.end {
        out.push(from..range.end);
    }
    Ok(out)
}
