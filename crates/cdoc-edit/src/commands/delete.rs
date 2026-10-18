//! Deleting: a selected range (structure-aware), Backspace and Delete.
//!
//! A range deletion removes every unit in the range except structure it cannot remove whole:
//! paragraphs of the start paragraph's container merge into it, tables of that container lying
//! entirely inside go, and everything else structural (cells, rows, tables cut by the range,
//! section and footnote starts) stays, with only its text removed. So a section, cell or
//! footnote is never left without a block.

use super::{done, Ctx, DeleteUnit, Outcome};
use crate::error::EditError;
use crate::position::{Direction, Position};
use crate::text::{paragraph, step_cluster, word_boundary};
use cdoc_crdt::{Atom, AtomKind};
use std::ops::Range;

/// The unit ranges to delete for the gaps `range`, descending.
fn plan(ctx: &Ctx<'_, '_>, range: &Range<usize>) -> Result<Vec<Range<usize>>, EditError> {
    let doc = &*ctx.tx.doc;
    let outline = &*ctx.outline;
    let (start_node, _) = paragraph(doc, outline, range.start)?;
    let container = outline.nodes[start_node].parent;
    let (first, last) = (doc.blocks_before(range.start), doc.blocks_before(range.end));
    let mut keep: Vec<usize> = Vec::new();
    let mut blocked = !outline.regular;
    let mut n = first;
    while n < last {
        let node = outline.nodes[n];
        let index = doc.locate(node.id).ok_or(EditError::UnknownPosition)?.index;
        let mine = node.parent == container && !blocked;
        match node.kind {
            AtomKind::Paragraph if mine => n += 1,
            AtomKind::Table if mine && node.end.is_some_and(|e| e < last) => {
                n = outline.after_subtree(n);
            }
            _ => {
                keep.push(index);
                blocked = true;
                n += 1;
            }
        }
    }
    let mut out = Vec::new();
    let mut end = range.end;
    for &k in keep.iter().rev() {
        if k + 1 < end {
            out.push(k + 1..end);
        }
        end = k;
    }
    if range.start < end {
        out.push(range.start..end);
    }
    Ok(out)
}

/// Deletes the gaps `range` (start..end of a selection); the caret ends at `range.start`.
pub(crate) fn range(ctx: &mut Ctx<'_, '_>, range: &Range<usize>) -> Result<(), EditError> {
    if range.is_empty() {
        return Ok(());
    }
    for r in plan(ctx, range)? {
        ctx.tx.delete(r)?;
    }
    Ok(())
}

/// Deletes the selection if there is one; the caret gap the edit continues from.
pub(crate) fn selection(ctx: &mut Ctx<'_, '_>) -> Result<usize, EditError> {
    let r = ctx.selection.range(ctx.tx.doc)?;
    range(ctx, &r)?;
    Ok(r.start)
}

fn caret_after_unit(ctx: &Ctx<'_, '_>, index: usize) -> Result<Position, EditError> {
    ctx.tx
        .doc
        .id_at(index)
        .map(Position::after)
        .ok_or(EditError::UnknownPosition)
}

/// Joins paragraph `node` into the one before it, or removes the paragraph before it when that
/// one is empty (so the remaining paragraph keeps its own formatting, as Word does).
fn join(ctx: &mut Ctx<'_, '_>, node: usize) -> Result<Option<Outcome>, EditError> {
    let prev = node.checked_sub(1).map(|p| ctx.outline.nodes[p]);
    let this = ctx.outline.nodes[node];
    let Some(prev) = prev.filter(|p| p.kind == AtomKind::Paragraph && p.parent == this.parent)
    else {
        return Ok(None);
    };
    let doc = &*ctx.tx.doc;
    let at = doc.locate(this.id).ok_or(EditError::UnknownPosition)?.index;
    let p = doc.locate(prev.id).ok_or(EditError::UnknownPosition)?.index;
    // Read from the replica: the outline is rebuilt on structural edits only, so typing into an
    // empty paragraph would leave a cached "empty" behind.
    if at > p + 1 {
        let caret = caret_after_unit(ctx, at - 1)?;
        ctx.tx.delete(at..at + 1)?;
        return Ok(Some(done(crate::position::Selection::caret(caret))));
    }
    ctx.tx.delete(p..p + 1)?;
    Ok(Some(done(crate::position::Selection::caret(
        Position::after(this.id),
    ))))
}

/// Backspace.
pub(crate) fn backward(ctx: &mut Ctx<'_, '_>, unit: DeleteUnit) -> Result<Outcome, EditError> {
    if !ctx.selection.is_collapsed(ctx.tx.doc)? {
        let g = selection(ctx)?;
        return Ok(done(ctx.caret_at(g)?));
    }
    let gap = ctx.selection.head.index(ctx.tx.doc)?;
    let (node, para) = paragraph(ctx.tx.doc, &ctx.outline, gap)?;
    if gap > para.start {
        let from = match unit {
            DeleteUnit::Char => step_cluster(ctx.tx.doc, gap, Direction::Backward)?.max(para.start),
            DeleteUnit::Word => word_boundary(ctx.tx.doc, gap, &para, Direction::Backward),
        };
        ctx.tx.delete(from..gap)?;
        return Ok(done(ctx.caret_at(from)?));
    }
    let at = para.start - 1;
    if let Some(Atom::Paragraph(p)) = cdoc_crdt::atom_at(ctx.tx.doc, at)? {
        if p.numbering.is_some() {
            let cleared = cdoc_model::ParagraphProperties {
                numbering: None,
                ..p
            };
            ctx.tx.set_block(at, &Atom::Paragraph(cleared))?;
            return Ok(done(ctx.selection));
        }
    }
    Ok(join(ctx, node)?.unwrap_or_else(|| done(ctx.selection)))
}

/// Delete.
pub(crate) fn forward(ctx: &mut Ctx<'_, '_>, unit: DeleteUnit) -> Result<Outcome, EditError> {
    if !ctx.selection.is_collapsed(ctx.tx.doc)? {
        let g = selection(ctx)?;
        return Ok(done(ctx.caret_at(g)?));
    }
    let gap = ctx.selection.head.index(ctx.tx.doc)?;
    let (node, para) = paragraph(ctx.tx.doc, &ctx.outline, gap)?;
    if gap < para.end {
        let to = match unit {
            DeleteUnit::Char => step_cluster(ctx.tx.doc, gap, Direction::Forward)?.min(para.end),
            DeleteUnit::Word => word_boundary(ctx.tx.doc, gap, &para, Direction::Forward),
        };
        ctx.tx.delete(gap..to.max(gap + 1))?;
        return Ok(done(ctx.selection));
    }
    let next = node + 1;
    let joinable = ctx.outline.nodes.get(next).is_some_and(|n| {
        n.kind == AtomKind::Paragraph && n.parent == ctx.outline.nodes[node].parent
    });
    if !joinable {
        return Ok(done(ctx.selection));
    }
    let keep = ctx.selection;
    let joined = join(ctx, next)?;
    Ok(match joined {
        Some(o) if para.start < para.end => Outcome {
            selection: keep,
            ..o
        },
        Some(o) => o,
        None => done(keep),
    })
}
