//! Character and paragraph formatting: toggles that read the effective (style-resolved)
//! value as Word does, property patches, and pending marks for a collapsed selection.

use super::MarkKind;
use super::{done, Ctx, Outcome};
use crate::error::EditError;
use crate::outline::Outline;
use crate::text::paragraph;
use cdoc_crdt::{atom_at, slice, Atom, AtomKind, Doc, Mark, Piece, RunKey};
use cdoc_model::{
    InlineContent, ParagraphProperties, RunPatch, RunProperties, Underline, UnderlineStyle,
    VerticalAlign,
};
use std::ops::Range;

/// Whether `kind` is on in resolved properties.
fn is_on(kind: MarkKind, p: &RunProperties) -> bool {
    match kind {
        MarkKind::Bold => p.bold == Some(true),
        MarkKind::Italic => p.italic == Some(true),
        MarkKind::Underline => p
            .underline
            .as_ref()
            .is_some_and(|u| u.style != UnderlineStyle::None),
        MarkKind::Strike => p.strike == Some(true),
        MarkKind::Subscript => p.vertical_align == Some(VerticalAlign::Subscript),
        MarkKind::Superscript => p.vertical_align == Some(VerticalAlign::Superscript),
    }
}

/// The property that turns `kind` on or explicitly off.
fn value(kind: MarkKind, on: bool) -> RunProperties {
    let none = RunProperties::none();
    match kind {
        MarkKind::Bold => RunProperties {
            bold: Some(on),
            ..none
        },
        MarkKind::Italic => RunProperties {
            italic: Some(on),
            ..none
        },
        MarkKind::Underline => RunProperties {
            underline: Some(Underline {
                style: if on {
                    UnderlineStyle::Single
                } else {
                    UnderlineStyle::None
                },
                color: None,
            }),
            ..none
        },
        MarkKind::Strike => RunProperties {
            strike: Some(on),
            ..none
        },
        MarkKind::Subscript | MarkKind::Superscript => {
            let align = match (on, kind) {
                (false, _) => VerticalAlign::Baseline,
                (true, MarkKind::Subscript) => VerticalAlign::Subscript,
                (true, _) => VerticalAlign::Superscript,
            };
            RunProperties {
                vertical_align: Some(align),
                ..none
            }
        }
    }
}

const fn key(kind: MarkKind) -> RunKey {
    match kind {
        MarkKind::Bold => RunKey::Bold,
        MarkKind::Italic => RunKey::Italic,
        MarkKind::Underline => RunKey::Underline,
        MarkKind::Strike => RunKey::Strike,
        MarkKind::Subscript | MarkKind::Superscript => RunKey::VerticalAlign,
    }
}

/// Text pieces of `range` with their unit ranges, paragraph properties and direct properties.
pub(crate) fn texts(
    doc: &Doc,
    outline: &Outline,
    range: Range<usize>,
) -> Result<Vec<(Range<usize>, ParagraphProperties, RunProperties)>, EditError> {
    let (_, para) = paragraph(doc, outline, range.start)?;
    let mut props = match atom_at(doc, para.start - 1)? {
        Some(Atom::Paragraph(p)) => p,
        _ => ParagraphProperties::none(),
    };
    let mut out = Vec::new();
    let mut at = range.start;
    for piece in slice(doc, range)? {
        let len = crate::tx::piece_len(&piece);
        match piece {
            Piece::Block(Atom::Paragraph(p)) => props = p,
            Piece::Inline(i) => {
                if let InlineContent::Text(run) = i.content {
                    out.push((at..at + len, props.clone(), run.properties));
                }
            }
            Piece::Block(_) => {}
        }
        at += len;
    }
    Ok(out)
}

/// What the style cascade gives a run, ignoring its direct value for `kind`.
fn inherited_on(
    ctx: &Ctx<'_, '_>,
    kind: MarkKind,
    para: &ParagraphProperties,
    run: &RunProperties,
) -> bool {
    let mut base = run.clone();
    crate::run_keys::clear_key(&mut base, key(kind));
    ctx.model
        .styles
        .resolve_run(para, &base)
        .is_ok_and(|r| is_on(kind, &r))
}

/// Toggles `kind` on the selection, or in the pending marks for the next typed text.
pub(crate) fn toggle(ctx: &mut Ctx<'_, '_>, kind: MarkKind) -> Result<Outcome, EditError> {
    let range = ctx.selection.range(ctx.tx.doc)?;
    if range.is_empty() {
        let (_, para) = paragraph(ctx.tx.doc, &ctx.outline, range.start)?;
        let mut base =
            super::typing::inherited(ctx.tx.doc, ctx.pending.as_ref(), range.start, para.start)?;
        let para_props = match atom_at(ctx.tx.doc, para.start - 1)? {
            Some(Atom::Paragraph(p)) => p,
            _ => ParagraphProperties::none(),
        };
        let on = ctx
            .model
            .styles
            .resolve_run(&para_props, &base)
            .is_ok_and(|r| is_on(kind, &r));
        if inherited_on(ctx, kind, &para_props, &base) == on {
            base = base.overlay(&value(kind, !on));
        } else {
            crate::run_keys::clear_key(&mut base, key(kind));
        }
        return Ok(Outcome {
            pending: Some(base),
            ..done(ctx.selection)
        });
    }
    let pieces = texts(ctx.tx.doc, &ctx.outline, range)?;
    let all_on = pieces.iter().all(|(_, p, r)| {
        ctx.model
            .styles
            .resolve_run(p, r)
            .is_ok_and(|r| is_on(kind, &r))
    });
    for (units, para, run) in pieces.iter().rev() {
        // The style already gives the wanted value: drop the direct one instead.
        let mark = if inherited_on(ctx, kind, para, run) == all_on {
            Mark::Run(value(kind, !all_on))
        } else {
            Mark::ClearRun(key(kind))
        };
        ctx.tx.format(units.clone(), &mark, true)?;
    }
    Ok(done(ctx.selection))
}

/// Sets the character properties `patch` sets.
pub(crate) fn set_run(ctx: &mut Ctx<'_, '_>, patch: &RunPatch) -> Result<Outcome, EditError> {
    if patch.is_empty() {
        return Err(EditError::InvalidArgument("the patch sets nothing"));
    }
    let range = ctx.selection.range(ctx.tx.doc)?;
    if range.is_empty() {
        let (_, para) = paragraph(ctx.tx.doc, &ctx.outline, range.start)?;
        let base =
            super::typing::inherited(ctx.tx.doc, ctx.pending.as_ref(), range.start, para.start)?;
        return Ok(Outcome {
            pending: Some(patch.apply(&base)),
            ..done(ctx.selection)
        });
    }
    let sets = patch.sets();
    if !sets.is_none() {
        ctx.tx.format(range.clone(), &Mark::Run(sets), true)?;
    }
    for (key, _) in RunKey::ALL.iter().zip(patch.clears()).filter(|(_, c)| *c) {
        ctx.tx.format(range.clone(), &Mark::ClearRun(*key), true)?;
    }
    Ok(done(ctx.selection))
}

/// The start-atom indices of the selected paragraphs.
pub(crate) fn selected_paragraphs(
    doc: &Doc,
    outline: &Outline,
    range: &Range<usize>,
) -> Result<Vec<usize>, EditError> {
    let (first, _) = paragraph(doc, outline, range.start)?;
    let (last, _) = paragraph(doc, outline, range.end)?;
    outline.nodes[first..=last]
        .iter()
        .filter(|n| n.kind == AtomKind::Paragraph)
        .map(|n| {
            doc.locate(n.id)
                .map(|l| l.index)
                .ok_or(EditError::UnknownPosition)
        })
        .collect()
}

/// Replaces each selected paragraph's properties with `f` of them.
pub(crate) fn paragraphs(
    ctx: &mut Ctx<'_, '_>,
    f: impl Fn(&ParagraphProperties) -> ParagraphProperties,
) -> Result<Outcome, EditError> {
    let range = ctx.selection.range(ctx.tx.doc)?;
    for at in selected_paragraphs(ctx.tx.doc, &ctx.outline, &range)? {
        if let Some(Atom::Paragraph(p)) = atom_at(ctx.tx.doc, at)? {
            ctx.tx.set_block(at, &Atom::Paragraph(f(&p)))?;
        }
    }
    Ok(done(ctx.selection))
}
