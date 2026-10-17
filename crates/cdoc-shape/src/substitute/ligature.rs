//! Ligature substitution: several glyphs become one, and the marks between them remember which
//! component they sat on, so mark-to-ligature positioning can put them back there.

use crate::apply::matching::{match_input, Matched};
use crate::apply::Ctx;
use crate::face::gdef::LIGATURE;
use crate::unicode::Category;
use ttf_parser::gsub::Ligature;

pub(super) fn apply(ctx: &mut Ctx<'_, '_>, lig: &Ligature<'_>) -> bool {
    if lig.components.is_empty() {
        super::replace(ctx, lig.glyph.0);
        return true;
    }
    let components = lig.components;
    let m = |g: u16, i: u16| components.get(i).is_some_and(|c| c.0 == g);
    match match_input(ctx, components.len(), &m) {
        Some(matched) => {
            ligate(ctx, &matched, lig.glyph.0);
            true
        }
        None => false,
    }
}

fn ligate(ctx: &mut Ctx<'_, '_>, matched: &Matched, glyph: u16) {
    let Matched {
        positions,
        count,
        end,
        components,
    } = *matched;
    let buf = &mut *ctx.buf;
    buf.merge_clusters(buf.idx, end);
    let mut base_ligature = buf.info[positions[0]].is_base();
    let mut mark_ligature = buf.info[positions[0]].is_mark();
    for &p in &positions[1..count] {
        if !buf.info[p].is_mark() {
            base_ligature = false;
            mark_ligature = false;
        }
    }
    let is_ligature = !base_ligature && !mark_ligature;
    let class = if is_ligature { LIGATURE } else { 0 };
    let lig_id = if is_ligature {
        buf.allocate_lig_id()
    } else {
        0
    };
    let first = buf.cur_mut(0);
    let mut last_lig_id = first.lig_id();
    let mut last_num_comps = first.lig_num_comps();
    let mut comps_so_far = last_num_comps;
    if is_ligature {
        first.set_lig_for_ligature(lig_id, components);
        if first.props.category() == Category::NonspacingMark {
            first.props.set_category(Category::Other);
        }
    }
    super::set_class(ctx, glyph, class, true, false);
    ctx.buf.replace_glyph(glyph);
    let buf = &mut *ctx.buf;
    for &p in &positions[1..count] {
        while buf.idx < p && !buf.failed {
            if is_ligature {
                let cur = buf.cur_mut(0);
                let this = match cur.lig_comp() {
                    0 => last_num_comps,
                    c => c,
                };
                let comp = comps_so_far
                    .wrapping_sub(last_num_comps)
                    .wrapping_add(this.min(last_num_comps));
                cur.set_lig_for_mark(lig_id, comp);
            }
            buf.next_glyph();
        }
        let cur = buf.cur(0);
        last_lig_id = cur.lig_id();
        last_num_comps = cur.lig_num_comps();
        comps_so_far = comps_so_far.wrapping_add(last_num_comps);
        buf.idx += 1;
    }
    if !mark_ligature && last_lig_id != 0 {
        for i in buf.idx..buf.len {
            let info = &mut buf.info[i];
            if info.lig_id() != last_lig_id || info.lig_comp() == 0 {
                break;
            }
            let comp = comps_so_far
                .wrapping_sub(last_num_comps)
                .wrapping_add(info.lig_comp().min(last_num_comps));
            info.set_lig_for_mark(lig_id, comp);
        }
    }
}
