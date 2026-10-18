//! Footnotes at the foot of the column that references them: each reference reserves its
//! note's height (and, for the column's first note, the separator's), and the notes are set
//! when the column is finished.

use super::Flow;
use crate::context::Ctx;
use crate::fragment::{Fragment, Mark};
use crate::ir::{CaretStop, Draw, Pt100, Region, Units};
use crate::labels::Counters;
use crate::stack::stack;
use crate::LayoutError;
use cdoc_model::{Block, FootnoteId, Inline, InlineContent};

/// Space the separator takes above the first note.
const SEPARATOR: Pt100 = 1000;
/// The separator rule's longest length: two inches, as Word draws it.
const SEPARATOR_LENGTH: Pt100 = 14_400;

fn laid_out(flow: &mut Flow, ctx: &mut Ctx<'_, '_>, id: FootnoteId) -> Result<Pt100, LayoutError> {
    let width = flow.geometry.columns[flow.column].width;
    if let Some(strip) = flow.note_cache.get(&(id, width)) {
        return Ok(strip.height);
    }
    let Some(note) = ctx.doc.footnotes.iter().find(|n| n.id == id) else {
        return Ok(0);
    };
    let mut blocks = note.blocks.clone();
    let numbered = if let Some(Block::Paragraph(first)) = blocks.first_mut() {
        first
            .inlines
            .insert(0, Inline::plain(InlineContent::FootnoteRef { id }));
        true
    } else {
        false
    };
    let mut counters = Counters::new(&ctx.doc.numbering);
    let mut strip = stack(ctx, &blocks, width, &mut counters)?;
    strip.marks.retain(|m| !matches!(m, Mark::Footnote(_)));
    if numbered {
        unnumber(&mut strip);
    }
    let height = strip.height;
    flow.note_cache.insert((id, width), strip);
    Ok(height)
}

/// The note's own number, set before its first paragraph's text, is no unit of that paragraph:
/// its lines' gaps count from after it, and the gap before it is not stood at.
fn unnumber(strip: &mut Fragment) {
    let mut runs = strip.draws.iter_mut().filter_map(|d| match d {
        Draw::Text(run) if run.units != Units::None => Some(run),
        _ => None,
    });
    for mark in &mut strip.marks {
        let Mark::Line(line) = mark else {
            continue;
        };
        let first = line.path == [0];
        let mut kept = 0;
        let mut number_end = None;
        for run in runs.by_ref().take(line.runs as usize) {
            if first {
                run.units = run.units.without_first();
                if run.units == Units::None {
                    number_end = Some(run.x + run.glyphs.iter().map(|g| g.advance).sum::<Pt100>());
                }
            }
            kept += u32::from(run.units != Units::None);
        }
        if first {
            line.runs = kept;
            for s in &mut line.stops {
                s.gap = s.gap.saturating_sub(1);
            }
            line.stops.dedup_by(|after, before| {
                let same = after.gap == before.gap;
                if same {
                    *before = *after;
                }
                same
            });
            if let (0, true, Some(x)) = (kept, line.stops.is_empty(), number_end) {
                line.stops.push(CaretStop { gap: 0, x });
            }
        }
    }
}

/// The height `strip`'s new footnotes would take from this column.
pub fn extra_for(
    flow: &mut Flow,
    ctx: &mut Ctx<'_, '_>,
    strip: &Fragment,
) -> Result<Pt100, LayoutError> {
    let mut extra = 0;
    let mut seen: Vec<FootnoteId> = Vec::new();
    for id in strip.footnotes() {
        if flow.notes.contains(&id) || seen.contains(&id) {
            continue;
        }
        if flow.notes.is_empty() && seen.is_empty() {
            extra += SEPARATOR;
        }
        seen.push(id);
        extra += laid_out(flow, ctx, id)?;
    }
    Ok(extra)
}

/// Keeps room for a note in this column.
pub fn reserve(flow: &mut Flow, ctx: &mut Ctx<'_, '_>, id: FootnoteId) -> Result<(), LayoutError> {
    if flow.notes.contains(&id) {
        return Ok(());
    }
    if flow.notes.is_empty() {
        flow.notes_height += SEPARATOR;
    }
    flow.notes_height += laid_out(flow, ctx, id)?;
    flow.notes.push(id);
    Ok(())
}

/// Sets the column's notes at its foot and clears the reservation.
pub fn set_at_foot(flow: &mut Flow, ctx: &mut Ctx<'_, '_>) -> Result<(), LayoutError> {
    if flow.notes.is_empty() {
        return Ok(());
    }
    let column = flow.geometry.columns[flow.column];
    let mut y = flow.geometry.bottom - flow.notes_height;
    let mut draws = vec![Draw::Rule {
        from: (column.x, y + SEPARATOR / 2),
        to: (
            column.x + column.width.min(SEPARATOR_LENGTH),
            y + SEPARATOR / 2,
        ),
        width: 50,
        colour: crate::style::INK,
    }];
    y += SEPARATOR;
    for id in std::mem::take(&mut flow.notes) {
        laid_out(flow, ctx, id)?;
        let Some(strip) = flow.note_cache.get(&(id, column.width)).cloned() else {
            continue;
        };
        let h = strip.height;
        let mut holder = Fragment::empty(0);
        holder.absorb(strip, column.x, y);
        draws.extend(holder.draws);
        let sheet = flow.sheets.last_mut().expect("never empty");
        sheet
            .page
            .lines
            .extend(holder.marks.into_iter().filter_map(|m| match m {
                Mark::Line(line) => Some(line.placed(Region::Footnote(id), &[])),
                _ => None,
            }));
        y += h;
    }
    flow.notes_height = 0;
    let sheet = flow.sheets.last_mut().expect("never empty");
    sheet.page.draws.extend(draws);
    Ok(())
}
