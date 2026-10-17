//! Mark attachment: a mark's anchor is put on its base's (or ligature component's, or mark's)
//! anchor. The offset is recorded against the base and resolved once all positioning is done,
//! because later lookups may still move the base.

use crate::apply::skip::{Step, Walker, ANY};
use crate::apply::{Ctx, IGNORE_MARKS};
use crate::buffer::{Buffer, GlyphPos, ATTACH_CURSIVE, ATTACH_MARK};
use ttf_parser::gpos::{
    AnchorMatrix, MarkArray, MarkToBaseAdjustment, MarkToLigatureAdjustment, MarkToMarkAdjustment,
};
use ttf_parser::GlyphId;

const IGNORE_FLAGS: u32 = 0x000E;

/// Places the current mark on `anchors[row][class]` of the glyph at `base`.
fn attach(
    ctx: &mut Ctx<'_, '_>,
    marks: MarkArray<'_>,
    anchors: AnchorMatrix<'_>,
    mark: u16,
    row: u16,
    base: usize,
) -> bool {
    let Some((class, mark_anchor)) = marks.get(mark) else {
        return false;
    };
    let Some(base_anchor) = anchors.get(row, class) else {
        return false;
    };
    let idx = ctx.buf.idx;
    let pos = &mut ctx.buf.pos[idx];
    pos.x_offset = i32::from(base_anchor.x) - i32::from(mark_anchor.x);
    pos.y_offset = i32::from(base_anchor.y) - i32::from(mark_anchor.y);
    pos.attach_type = ATTACH_MARK;
    pos.attach_chain =
        i16::try_from(isize::try_from(base).unwrap_or(0) - isize::try_from(idx).unwrap_or(0))
            .unwrap_or(0);
    ctx.buf.has_attachment = true;
    ctx.buf.idx += 1;
    true
}

/// Only the first glyph of a multiple substitution's sequence takes marks, unless the
/// sequence itself had a mark in it.
fn accepts(buf: &Buffer, i: usize) -> bool {
    let info = &buf.info[i];
    !info.multiplied()
        || info.lig_comp() == 0
        || i == 0
        || buf.info[i - 1].is_mark()
        || !buf.info[i - 1].multiplied()
        || info.lig_id() != buf.info[i - 1].lig_id()
        || info.lig_comp() != buf.info[i - 1].lig_comp().wrapping_add(1)
}

/// The nearest preceding non-mark, cached across the marks that follow one base. `covered`
/// lets a mark-to-base lookup take a later sequence glyph its coverage names.
fn find_base(
    ctx: &mut Ctx<'_, '_>,
    covered: impl Fn(u16) -> bool,
    check_accept: bool,
) -> Option<usize> {
    if ctx.last_base_until > ctx.buf.idx {
        ctx.last_base_until = 0;
        ctx.last_base = None;
    }
    let walker = Walker::new(ctx, 0, false).with_props(IGNORE_MARKS);
    let mut j = ctx.buf.idx;
    while j > ctx.last_base_until {
        let mut step = walker.step(ctx, &ctx.buf.info[j - 1], ANY);
        if step == Step::Match
            && check_accept
            && !accepts(ctx.buf, j - 1)
            && !covered(ctx.buf.info[j - 1].glyph)
        {
            step = Step::Skip;
        }
        if step == Step::Match {
            ctx.last_base = Some(j - 1);
            break;
        }
        j -= 1;
    }
    ctx.last_base_until = ctx.buf.idx;
    ctx.last_base
}

pub(super) fn to_base(ctx: &mut Ctx<'_, '_>, m: &MarkToBaseAdjustment<'_>) -> bool {
    let Some(mark) = m.mark_coverage.get(GlyphId(ctx.buf.cur(0).glyph)) else {
        return false;
    };
    let Some(base) = find_base(ctx, |g| m.base_coverage.contains(GlyphId(g)), true) else {
        return false;
    };
    let Some(row) = m.base_coverage.get(GlyphId(ctx.buf.info[base].glyph)) else {
        return false;
    };
    attach(ctx, m.marks, m.anchors, mark, row, base)
}

pub(super) fn to_ligature(ctx: &mut Ctx<'_, '_>, m: &MarkToLigatureAdjustment<'_>) -> bool {
    let Some(mark) = m.mark_coverage.get(GlyphId(ctx.buf.cur(0).glyph)) else {
        return false;
    };
    let Some(lig) = find_base(ctx, |_| false, false) else {
        return false;
    };
    let Some(index) = m.ligature_coverage.get(GlyphId(ctx.buf.info[lig].glyph)) else {
        return false;
    };
    let Some(anchors) = m.ligature_array.get(index) else {
        return false;
    };
    let components = anchors.rows;
    if components == 0 {
        return false;
    }
    let (lig_id, cur) = (ctx.buf.info[lig].lig_id(), *ctx.buf.cur(0));
    let mark_comp = u16::from(cur.lig_comp());
    let same = lig_id != 0 && lig_id == cur.lig_id() && mark_comp > 0;
    let row = if same {
        mark_comp.min(components)
    } else {
        components
    } - 1;
    attach(ctx, m.marks, anchors, mark, row, lig)
}

pub(super) fn to_mark(ctx: &mut Ctx<'_, '_>, m: &MarkToMarkAdjustment<'_>) -> bool {
    let Some(mark1) = m.mark1_coverage.get(GlyphId(ctx.buf.cur(0).glyph)) else {
        return false;
    };
    let props = ctx.lookup_props & !IGNORE_FLAGS;
    let mut walker = Walker::new(ctx, ctx.buf.idx, false).with_props(props);
    if !walker.prev(ctx, ANY) {
        return false;
    }
    let j = walker.idx;
    let (cur, prev) = (*ctx.buf.cur(0), ctx.buf.info[j]);
    if !prev.is_mark() {
        return false;
    }
    let (id1, id2, comp1, comp2) = (cur.lig_id(), prev.lig_id(), cur.lig_comp(), prev.lig_comp());
    let same = if id1 == id2 {
        id1 == 0 || comp1 == comp2
    } else {
        (id1 > 0 && comp1 == 0) || (id2 > 0 && comp2 == 0)
    };
    if !same {
        return false;
    }
    let Some(mark2) = m.mark2_coverage.get(GlyphId(prev.glyph)) else {
        return false;
    };
    attach(ctx, m.marks, m.mark2_matrix, mark1, mark2, j)
}

fn propagate(pos: &mut [GlyphPos], i: usize) {
    let (chain, kind) = (pos[i].attach_chain, pos[i].attach_type);
    if chain == 0 {
        return;
    }
    pos[i].attach_chain = 0;
    let Some(j) = i
        .checked_add_signed(isize::from(chain))
        .filter(|&j| j < pos.len())
    else {
        return;
    };
    propagate(pos, j);
    match kind {
        ATTACH_MARK if j < i => {
            pos[i].x_offset += pos[j].x_offset;
            pos[i].y_offset += pos[j].y_offset;
            for k in j..i {
                pos[i].x_offset -= pos[k].x_advance;
                pos[i].y_offset -= pos[k].y_advance;
            }
        }
        ATTACH_CURSIVE => pos[i].y_offset += pos[j].y_offset,
        _ => {}
    }
}

/// Resolves every attachment against its base's final position (left to right).
pub(crate) fn propagate_attachments(buf: &mut Buffer) {
    if buf.has_attachment {
        let len = buf.len;
        for i in 0..len {
            propagate(&mut buf.pos[..len], i);
        }
    }
}
