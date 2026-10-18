//! Typing: text with Word's mark inheritance, Enter, inline atoms, replace, and the selection
//! commands.

use super::{delete, done, Ctx, Group, Outcome};
use crate::error::EditError;
use crate::position::{caret_gap, Position, Selection};
use crate::restore::set_exact;
use crate::text::{is_word_char, paragraph};
use cdoc_crdt::{atom_at, slice, Atom, AtomKind, Doc, Piece, Unit};
use cdoc_model::{InlineContent, ParagraphProperties, RunProperties};

/// The character properties of the text unit at `index`, if it is text.
pub(crate) fn props_at(doc: &Doc, index: usize) -> Result<Option<RunProperties>, EditError> {
    if !matches!(doc.unit_at(index), Some(Unit::Char(_))) {
        return Ok(None);
    }
    Ok(slice(doc, index..index + 1)?
        .into_iter()
        .find_map(|p| match p {
            Piece::Inline(i) => match i.content {
                InlineContent::Text(run) => Some(run.properties),
                _ => None,
            },
            Piece::Block(_) => None,
        }))
}

/// What text typed at `gap` is formatted as: the pending formatting if a format was chosen at
/// this caret, else the character before it in the paragraph, else the one after it (Word at a
/// paragraph start).
pub(crate) fn inherited(
    doc: &Doc,
    pending: Option<&RunProperties>,
    gap: usize,
    para_start: usize,
) -> Result<RunProperties, EditError> {
    inherited_by(pending, gap, para_start, |i| props_at(doc, i))
}

/// [`inherited`], reading the character properties of unit `i` with `props_at`.
pub(crate) fn inherited_by(
    pending: Option<&RunProperties>,
    gap: usize,
    para_start: usize,
    props_at: impl Fn(usize) -> Result<Option<RunProperties>, EditError>,
) -> Result<RunProperties, EditError> {
    if let Some(p) = pending {
        return Ok(p.clone());
    }
    let before = if gap > para_start {
        props_at(gap - 1)?
    } else {
        None
    };
    Ok(match before {
        Some(p) => p,
        None => props_at(gap)?.unwrap_or_else(RunProperties::none),
    })
}

/// Splits the paragraph at `gap` (copying its properties); the gap after the new start atom.
fn split_at(ctx: &mut Ctx<'_, '_>, gap: usize) -> Result<usize, EditError> {
    let (_, para) = paragraph(ctx.tx.doc, &ctx.outline, gap)?;
    let Some(Atom::Paragraph(props)) = atom_at(ctx.tx.doc, para.start - 1)? else {
        return Err(EditError::NotApplicable(
            "the position is not in a paragraph",
        ));
    };
    let props = if gap == para.end {
        let next = props
            .style
            .as_ref()
            .and_then(|s| ctx.model.styles.get(s))
            .and_then(|s| s.next.clone());
        ParagraphProperties {
            style: next.or(props.style.clone()),
            ..props
        }
    } else {
        props
    };
    ctx.tx.insert_atom(gap, &Atom::Paragraph(props))?;
    Ok(gap + 1)
}

/// Inserts `text` at `gap` formatted as `props`; returns the gap after it.
fn put(
    ctx: &mut Ctx<'_, '_>,
    mut gap: usize,
    text: &str,
    props: &RunProperties,
) -> Result<usize, EditError> {
    for (k, line) in text.split('\n').enumerate() {
        if k > 0 {
            gap = split_at(ctx, gap)?;
            ctx.refresh();
        }
        for (j, chunk) in line.split('\t').enumerate() {
            if j > 0 {
                ctx.tx.insert_atom(gap, &Atom::Inline(InlineContent::Tab))?;
                gap += 1;
            }
            if chunk.is_empty() {
                continue;
            }
            ctx.tx.insert_text(gap, chunk)?;
            let end = gap + chunk.chars().count();
            set_exact(&mut ctx.tx, gap..end, Some(props), None, false)?;
            gap = end;
        }
    }
    Ok(gap)
}

/// Types `text` over the selection.
pub(crate) fn insert_text(ctx: &mut Ctx<'_, '_>, text: &str) -> Result<Outcome, EditError> {
    if text.is_empty() {
        return Err(EditError::InvalidArgument("empty text"));
    }
    let gap = delete::selection(ctx)?;
    ctx.refresh();
    let (_, para) = paragraph(ctx.tx.doc, &ctx.outline, gap)?;
    let props = inherited(ctx.tx.doc, ctx.pending.as_ref(), gap, para.start)?;
    let end = put(ctx, gap, text, &props)?;
    let word = text.chars().all(is_word_char);
    Ok(Outcome {
        selection: ctx.caret_at(end)?,
        pending: None,
        group: Group::Typing { word },
    })
}

/// Enter. On an empty list item it ends the list instead, as Word does.
pub(crate) fn split(ctx: &mut Ctx<'_, '_>) -> Result<Outcome, EditError> {
    let gap = delete::selection(ctx)?;
    ctx.refresh();
    let (_, para) = paragraph(ctx.tx.doc, &ctx.outline, gap)?;
    if para.is_empty() {
        if let Some(Atom::Paragraph(p)) = atom_at(ctx.tx.doc, para.start - 1)? {
            if p.numbering.is_some() {
                let cleared = ParagraphProperties {
                    numbering: None,
                    ..p
                };
                ctx.tx
                    .set_block(para.start - 1, &Atom::Paragraph(cleared))?;
                return Ok(done(ctx.caret_at(gap)?));
            }
        }
    }
    let after = split_at(ctx, gap)?;
    Ok(done(ctx.caret_at(after)?))
}

/// An inline atom (break, tab, image, ...) over the selection.
pub(crate) fn inline(ctx: &mut Ctx<'_, '_>, content: InlineContent) -> Result<Outcome, EditError> {
    let gap = delete::selection(ctx)?;
    ctx.refresh();
    paragraph(ctx.tx.doc, &ctx.outline, gap)?;
    ctx.tx.insert_atom(gap, &Atom::Inline(content))?;
    Ok(done(ctx.caret_at(gap + 1)?))
}

/// Replaces `from..to` with `text`, formatted as the first replaced character.
pub(crate) fn replace(
    ctx: &mut Ctx<'_, '_>,
    from: Position,
    to: Position,
    text: &str,
) -> Result<Outcome, EditError> {
    let (a, b) = (from.index(ctx.tx.doc)?, to.index(ctx.tx.doc)?);
    let (a, b) = (a.min(b), a.max(b));
    let (_, para) = paragraph(ctx.tx.doc, &ctx.outline, a)?;
    let props = match props_at(ctx.tx.doc, a)? {
        Some(p) if a < b => p,
        _ => inherited(ctx.tx.doc, ctx.pending.as_ref(), a, para.start)?,
    };
    delete::range(ctx, &(a..b))?;
    ctx.refresh();
    let end = if text.is_empty() {
        a
    } else {
        put(ctx, a, text, &props)?
    };
    Ok(done(ctx.caret_at(end)?))
}

/// Selects the body: from the first caret gap to the last one before the footnotes.
pub(crate) fn select_all(ctx: &mut Ctx<'_, '_>) -> Result<Outcome, EditError> {
    let doc = &*ctx.tx.doc;
    let body_end = ctx
        .outline
        .nodes
        .iter()
        .find(|n| n.kind == AtomKind::Footnote)
        .and_then(|n| doc.locate(n.id))
        .map_or(doc.len(), |l| l.index);
    let first = caret_gap(doc, 0)?;
    let last = (1..=body_end)
        .rev()
        .find(|&g| crate::position::is_caret_gap(doc, g))
        .unwrap_or(first);
    let at = |g| Position::at(doc, g).ok_or(EditError::UnknownPosition);
    Ok(done(Selection {
        anchor: at(first)?,
        head: at(last)?,
    }))
}

/// Sets the selection, each end moved to the nearest caret gap.
pub(crate) fn set_selection(ctx: &mut Ctx<'_, '_>, s: Selection) -> Result<Outcome, EditError> {
    let anchor = ctx.caret_at(s.anchor.index(ctx.tx.doc)?)?.head;
    let head = ctx.caret_at(s.head.index(ctx.tx.doc)?)?.head;
    Ok(done(Selection { anchor, head }))
}
