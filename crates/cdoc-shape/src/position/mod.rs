//! GPOS subtables: single (1) and pair (2) adjustment, mark to base (4), mark to ligature (5),
//! mark to mark (6), context (7) and chained context (8). Extension subtables (9) are unwrapped
//! by `ttf-parser`. Device tables are not applied: they only adjust for a pixel size or a
//! variation instance, and shaping here is size-independent on static fonts.

pub(crate) mod kern;
mod marks;

pub(crate) use marks::propagate_attachments;

use crate::apply::context::{apply_chain_context, apply_context};
use crate::apply::skip::{Walker, ANY};
use crate::apply::Ctx;
use crate::face::layout::{ClassPairs, PosSubtable};
use ttf_parser::gpos::{PairAdjustment, PositioningSubtable, SingleAdjustment, ValueRecord};
use ttf_parser::GlyphId;

pub(crate) fn apply(ctx: &mut Ctx<'_, '_>, subtable: &PosSubtable<'_>) -> bool {
    match subtable {
        PosSubtable::ClassPairs(pairs) => class_pairs(ctx, pairs),
        PosSubtable::Other(sub) => other(ctx, sub),
    }
}

fn other(ctx: &mut Ctx<'_, '_>, subtable: &PositioningSubtable<'_>) -> bool {
    let glyph = GlyphId(ctx.buf.cur(0).glyph);
    match subtable {
        PositioningSubtable::Single(s) => {
            let record = match *s {
                SingleAdjustment::Format1 { coverage, value } => coverage.get(glyph).map(|_| value),
                SingleAdjustment::Format2 { coverage, values } => {
                    coverage.get(glyph).and_then(|i| values.get(i))
                }
            };
            record.is_some_and(|r| {
                let idx = ctx.buf.idx;
                adjust(ctx, idx, &r);
                ctx.buf.idx += 1;
                true
            })
        }
        PositioningSubtable::Pair(PairAdjustment::Format1 { coverage, sets }) => {
            let Some(index) = coverage.get(glyph) else {
                return false;
            };
            pair(ctx, |second| {
                sets.get(index).and_then(|set| set.get(GlyphId(second)))
            })
        }
        PositioningSubtable::MarkToBase(m) => marks::to_base(ctx, m),
        PositioningSubtable::MarkToLigature(m) => marks::to_ligature(ctx, m),
        PositioningSubtable::MarkToMark(m) => marks::to_mark(ctx, m),
        PositioningSubtable::Context(c) => apply_context(ctx, c),
        PositioningSubtable::ChainContext(c) => apply_chain_context(ctx, c),
        PositioningSubtable::Pair(PairAdjustment::Format2 { .. })
        | PositioningSubtable::Cursive(_) => false,
    }
}

fn class_pairs(ctx: &mut Ctx<'_, '_>, pairs: &ClassPairs<'_>) -> bool {
    let first = ctx.buf.cur(0).glyph;
    if !pairs.first.contains(first) {
        return false;
    }
    let PairAdjustment::Format2 { matrix, .. } = pairs.table else {
        return false;
    };
    let class1 = pairs.class1.get(usize::from(first)).copied().unwrap_or(0);
    pair(ctx, |second| {
        let class2 = pairs.class2.get(usize::from(second)).copied().unwrap_or(0);
        matrix.get((class1, class2))
    })
}

/// Finds the second glyph of a pair and applies both value records. A pair whose second
/// record moves the second glyph consumes it too.
fn pair<'r>(
    ctx: &mut Ctx<'_, '_>,
    records: impl Fn(u16) -> Option<(ValueRecord<'r>, ValueRecord<'r>)>,
) -> bool {
    let mut walker = Walker::new(ctx, ctx.buf.idx, false);
    if !walker.next(ctx, ANY) {
        return false;
    }
    let second = walker.idx;
    let Some((first_record, second_record)) = records(ctx.buf.info[second].glyph) else {
        return false;
    };
    let idx = ctx.buf.idx;
    adjust(ctx, idx, &first_record);
    adjust(ctx, second, &second_record);
    ctx.buf.idx = if is_empty(&second_record) {
        second
    } else {
        second + 1
    };
    true
}

fn is_empty(r: &ValueRecord<'_>) -> bool {
    r.x_placement == 0
        && r.y_placement == 0
        && r.x_advance == 0
        && r.y_advance == 0
        && r.x_placement_device.is_none()
        && r.y_placement_device.is_none()
        && r.x_advance_device.is_none()
        && r.y_advance_device.is_none()
}

/// Horizontal runs take the x advance; the y advance is for vertical text.
fn adjust(ctx: &mut Ctx<'_, '_>, at: usize, r: &ValueRecord<'_>) {
    let pos = &mut ctx.buf.pos[at];
    pos.x_offset += i32::from(r.x_placement);
    pos.y_offset += i32::from(r.y_placement);
    pos.x_advance += i32::from(r.x_advance);
}
