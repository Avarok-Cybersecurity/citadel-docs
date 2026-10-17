//! GSUB subtables: single (1), multiple (2), alternate (3, first alternate), ligature (4),
//! context (5), chained context (6) and reverse chained single (8). Extension subtables (7) are
//! unwrapped by `ttf-parser` when the lookup is read.

mod ligature;

use crate::apply::context::{apply_chain_context, apply_context};
use crate::apply::matching::{match_backtrack, match_lookahead};
use crate::apply::Ctx;
use crate::face::gdef::{BASE, LIGATED, MULTIPLIED, PRESERVE, SUBSTITUTED};
use ttf_parser::gsub::{SingleSubstitution, SubstitutionSubtable};
use ttf_parser::GlyphId;

pub(crate) fn apply(ctx: &mut Ctx<'_, '_>, subtable: &SubstitutionSubtable<'_>) -> bool {
    let glyph = GlyphId(ctx.buf.cur(0).glyph);
    match subtable {
        SubstitutionSubtable::Single(s) => {
            let to = match *s {
                SingleSubstitution::Format1 { coverage, delta } => coverage
                    .get(glyph)
                    .map(|_| GlyphId(glyph.0.wrapping_add_signed(delta))),
                SingleSubstitution::Format2 {
                    coverage,
                    substitutes,
                } => coverage.get(glyph).and_then(|i| substitutes.get(i)),
            };
            to.is_some_and(|g| {
                replace(ctx, g.0);
                true
            })
        }
        SubstitutionSubtable::Multiple(m) => {
            let seq = m.coverage.get(glyph).and_then(|i| m.sequences.get(i));
            seq.is_some_and(|seq| {
                multiple(ctx, seq.substitutes);
                true
            })
        }
        SubstitutionSubtable::Alternate(a) => {
            let first = a
                .coverage
                .get(glyph)
                .and_then(|i| a.alternate_sets.get(i))
                .and_then(|set| set.alternates.get(0));
            first.is_some_and(|g| {
                replace(ctx, g.0);
                true
            })
        }
        SubstitutionSubtable::Ligature(l) => {
            let set = l.coverage.get(glyph).and_then(|i| l.ligature_sets.get(i));
            set.is_some_and(|set| set.into_iter().any(|lig| ligature::apply(ctx, &lig)))
        }
        SubstitutionSubtable::Context(c) => apply_context(ctx, c),
        SubstitutionSubtable::ChainContext(c) => apply_chain_context(ctx, c),
        SubstitutionSubtable::ReverseChainSingle(r) => reverse_single(ctx, r),
    }
}

/// Marks the current glyph substituted and gives it `glyph`'s class (from GDEF, else `guess`).
pub(crate) fn set_class(
    ctx: &mut Ctx<'_, '_>,
    glyph: u16,
    guess: u16,
    ligature: bool,
    component: bool,
) {
    let gdef = &ctx.face.gdef;
    let cur = ctx.buf.cur_mut(0);
    let mut props = cur.gprops | SUBSTITUTED;
    if ligature {
        props = (props | LIGATED) & !MULTIPLIED;
    }
    if component {
        props |= MULTIPLIED;
    }
    cur.gprops = if gdef.has_classes() {
        (props & PRESERVE) | gdef.props(glyph)
    } else if guess != 0 {
        (props & PRESERVE) | guess
    } else {
        props
    };
}

pub(crate) fn replace(ctx: &mut Ctx<'_, '_>, glyph: u16) {
    set_class(ctx, glyph, 0, false, false);
    ctx.buf.replace_glyph(glyph);
}

fn multiple(ctx: &mut Ctx<'_, '_>, substitutes: ttf_parser::LazyArray16<'_, GlyphId>) {
    match substitutes.len() {
        0 => ctx.buf.delete_glyph(),
        1 => replace(ctx, substitutes.get(0).map_or(0, |g| g.0)),
        _ => {
            let guess = if ctx.buf.cur(0).is_ligature() {
                BASE
            } else {
                0
            };
            let lig_id = ctx.buf.cur(0).lig_id();
            for (i, g) in substitutes.into_iter().enumerate() {
                if lig_id == 0 {
                    let comp = u8::try_from(i).unwrap_or(u8::MAX);
                    ctx.buf.cur_mut(0).set_lig_for_mark(0, comp);
                }
                set_class(ctx, g.0, guess, false, true);
                ctx.buf.output_glyph(g.0);
            }
            ctx.buf.skip_glyph();
        }
    }
}

fn reverse_single(
    ctx: &mut Ctx<'_, '_>,
    r: &ttf_parser::gsub::ReverseChainSingleSubstitution<'_>,
) -> bool {
    let glyph = GlyphId(ctx.buf.cur(0).glyph);
    let Some(index) = r.coverage.get(glyph) else {
        return false;
    };
    let Some(to) = r.substitutes.get(index) else {
        return false;
    };
    if !ctx.is_outermost() {
        return false;
    }
    let back = |g: u16, i: u16| {
        r.backtrack_coverages
            .get(i)
            .is_some_and(|c| c.contains(GlyphId(g)))
    };
    let ahead = |g: u16, i: u16| {
        r.lookahead_coverages
            .get(i)
            .is_some_and(|c| c.contains(GlyphId(g)))
    };
    let start = ctx.buf.idx + 1;
    if match_backtrack(ctx, r.backtrack_coverages.len(), &back).is_some()
        && match_lookahead(ctx, r.lookahead_coverages.len(), &ahead, start).is_some()
    {
        set_class(ctx, to.0, 0, false, false);
        ctx.buf.cur_mut(0).glyph = to.0;
        return true;
    }
    false
}
