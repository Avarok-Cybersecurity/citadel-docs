//! Contextual and chained contextual rules (GSUB 5/6, GPOS 7/8): match a sequence with its
//! surroundings, then run nested lookups at chosen positions inside it.

use super::matching::{match_backtrack, match_input, match_lookahead};
use super::nested::apply_nested;
use super::Ctx;
use ttf_parser::opentype_layout::{
    ChainedContextLookup, ClassDefinition, ContextLookup, SequenceLookupRecord,
};
use ttf_parser::{GlyphId, LazyArray16};

fn class(def: ClassDefinition<'_>) -> impl Fn(u16, u16) -> bool + '_ {
    move |glyph, value| def.get(GlyphId(glyph)) == value
}

fn glyph(glyph: u16, value: u16) -> bool {
    glyph == value
}

/// Indexes a rule's value array for the matcher of one sequence.
fn rule<'r, F: Fn(u16, u16) -> bool>(
    values: LazyArray16<'r, u16>,
    m: &'r F,
) -> impl Fn(u16, u16) -> bool + 'r {
    move |g, i| values.get(i).is_some_and(|v| m(g, v))
}

pub(crate) fn apply_context(ctx: &mut Ctx<'_, '_>, table: &ContextLookup<'_>) -> bool {
    let first = GlyphId(ctx.buf.cur(0).glyph);
    match *table {
        ContextLookup::Format1 { coverage, sets } => coverage
            .get(first)
            .and_then(|i| sets.get(i))
            .is_some_and(|set| {
                set.into_iter()
                    .any(|r| simple_rule(ctx, r.input, &glyph, r.lookups))
            }),
        ContextLookup::Format2 {
            coverage,
            classes,
            sets,
        } => {
            coverage.get(first).is_some()
                && sets.get(classes.get(first)).is_some_and(|set| {
                    set.into_iter()
                        .any(|r| simple_rule(ctx, r.input, &class(classes), r.lookups))
                })
        }
        ContextLookup::Format3 {
            coverage,
            coverages,
            lookups,
        } => {
            if coverage.get(first).is_none() {
                return false;
            }
            let m = |g: u16, i: u16| coverages.get(i).is_some_and(|c| c.contains(GlyphId(g)));
            match match_input(ctx, coverages.len(), &m) {
                Some(matched) => {
                    apply_nested(ctx, &matched, lookups);
                    true
                }
                None => false,
            }
        }
    }
}

fn simple_rule<F: Fn(u16, u16) -> bool>(
    ctx: &mut Ctx<'_, '_>,
    input: LazyArray16<'_, u16>,
    m: &F,
    lookups: LazyArray16<'_, SequenceLookupRecord>,
) -> bool {
    match match_input(ctx, input.len(), &rule(input, m)) {
        Some(matched) => {
            apply_nested(ctx, &matched, lookups);
            true
        }
        None => false,
    }
}

pub(crate) fn apply_chain_context(ctx: &mut Ctx<'_, '_>, table: &ChainedContextLookup<'_>) -> bool {
    let first = GlyphId(ctx.buf.cur(0).glyph);
    match *table {
        ChainedContextLookup::Format1 { coverage, sets } => coverage
            .get(first)
            .and_then(|i| sets.get(i))
            .is_some_and(|set| {
                set.into_iter().any(|r| {
                    let m: [&dyn Fn(u16, u16) -> bool; 3] = [
                        &rule(r.backtrack, &glyph),
                        &rule(r.input, &glyph),
                        &rule(r.lookahead, &glyph),
                    ];
                    chain_rule(
                        ctx,
                        [r.backtrack.len(), r.input.len(), r.lookahead.len()],
                        m,
                        r.lookups,
                    )
                })
            }),
        ChainedContextLookup::Format2 {
            coverage,
            backtrack_classes,
            input_classes,
            lookahead_classes,
            sets,
        } => {
            let (b, i, l) = (
                class(backtrack_classes),
                class(input_classes),
                class(lookahead_classes),
            );
            coverage.get(first).is_some()
                && sets.get(input_classes.get(first)).is_some_and(|set| {
                    set.into_iter().any(|r| {
                        let m: [&dyn Fn(u16, u16) -> bool; 3] = [
                            &rule(r.backtrack, &b),
                            &rule(r.input, &i),
                            &rule(r.lookahead, &l),
                        ];
                        chain_rule(
                            ctx,
                            [r.backtrack.len(), r.input.len(), r.lookahead.len()],
                            m,
                            r.lookups,
                        )
                    })
                })
        }
        ChainedContextLookup::Format3 {
            coverage,
            backtrack_coverages,
            input_coverages,
            lookahead_coverages,
            lookups,
        } => {
            if coverage.get(first).is_none() {
                return false;
            }
            let b = |g: u16, i: u16| {
                backtrack_coverages
                    .get(i)
                    .is_some_and(|c| c.contains(GlyphId(g)))
            };
            let i = |g: u16, x: u16| {
                input_coverages
                    .get(x)
                    .is_some_and(|c| c.contains(GlyphId(g)))
            };
            let l = |g: u16, x: u16| {
                lookahead_coverages
                    .get(x)
                    .is_some_and(|c| c.contains(GlyphId(g)))
            };
            let lens = [
                backtrack_coverages.len(),
                input_coverages.len(),
                lookahead_coverages.len(),
            ];
            chain_rule(ctx, lens, [&b, &i, &l], lookups)
        }
    }
}

/// Input first, then lookahead after it, then backtrack before it; all three must match.
fn chain_rule(
    ctx: &mut Ctx<'_, '_>,
    lens: [u16; 3],
    m: [&dyn Fn(u16, u16) -> bool; 3],
    lookups: LazyArray16<'_, SequenceLookupRecord>,
) -> bool {
    let Some(matched) = match_input(ctx, lens[1], &m[1]) else {
        return false;
    };
    if match_lookahead(ctx, lens[2], &m[2], matched.end).is_none()
        || match_backtrack(ctx, lens[0], &m[0]).is_none()
    {
        return false;
    }
    apply_nested(ctx, &matched, lookups);
    true
}
