//! Matching a rule's input, backtrack and lookahead sequences against the run.

use super::skip::Walker;
use super::Ctx;

/// Longest sequence a rule may match.
pub(crate) const MAX_CONTEXT: usize = 64;

/// Where a rule's input glyphs were found.
pub(crate) struct Matched {
    pub(crate) positions: [usize; MAX_CONTEXT],
    /// How many input glyphs (the first included).
    pub(crate) count: usize,
    /// One past the last input glyph.
    pub(crate) end: usize,
    /// Components of the matched glyphs, ligatures counting all of theirs.
    pub(crate) components: u8,
}

/// Matches `len` glyphs after the current one. Ligature components must belong together:
/// glyphs from different ligatures, or a mark of one ligature against another, do not match.
pub(crate) fn match_input<F: Fn(u16, u16) -> bool>(
    ctx: &Ctx<'_, '_>,
    len: u16,
    matcher: &F,
) -> Option<Matched> {
    let count = usize::from(len) + 1;
    if count > MAX_CONTEXT {
        return None;
    }
    let buf = &ctx.buf;
    let mut walker = Walker::new(ctx, buf.idx, false);
    let first = *buf.cur(0);
    let (first_id, first_comp) = (first.lig_id(), first.lig_comp());
    let mut out = Matched {
        positions: [0; MAX_CONTEXT],
        count,
        end: 0,
        components: 0,
    };
    let mut ligbase: Option<bool> = None;
    for i in 1..count {
        if !walker.next(ctx, Some(matcher)) {
            return None;
        }
        out.positions[i] = walker.idx;
        let this = buf.info[walker.idx];
        let (this_id, this_comp) = (this.lig_id(), this.lig_comp());
        if first_id != 0 && first_comp != 0 {
            if first_id != this_id || first_comp != this_comp {
                let may_skip =
                    *ligbase.get_or_insert_with(|| ligature_base_skippable(ctx, &walker, first_id));
                if !may_skip {
                    return None;
                }
            }
        } else if this_id != 0 && this_comp != 0 && this_id != first_id {
            return None;
        }
        out.components = out.components.wrapping_add(this.lig_num_comps());
    }
    out.end = walker.idx + 1;
    out.components = out.components.wrapping_add(first.lig_num_comps());
    out.positions[0] = buf.idx;
    Some(out)
}

/// Whether the ligature whose component the first glyph is would itself be skipped by this
/// lookup's flags (then its components may mix with other glyphs).
fn ligature_base_skippable(ctx: &Ctx<'_, '_>, walker: &Walker, lig_id: u8) -> bool {
    let buf = &ctx.buf;
    let mut j = buf.out_len;
    while j > 0 && buf.out_info(j - 1).lig_id() == lig_id {
        j -= 1;
        if buf.out_info(j).lig_comp() == 0 {
            return walker.skips_outright(ctx, buf.out_info(j));
        }
    }
    false
}

/// Matches `len` glyphs before the current one, in the output. The index the backtrack starts
/// at, on success.
pub(crate) fn match_backtrack<F: Fn(u16, u16) -> bool>(
    ctx: &Ctx<'_, '_>,
    len: u16,
    matcher: &F,
) -> Option<usize> {
    let mut walker = Walker::new(ctx, ctx.buf.backtrack_len(), true);
    for _ in 0..len {
        if !walker.prev(ctx, Some(matcher)) {
            return None;
        }
    }
    Some(walker.idx)
}

/// Matches `len` glyphs from `start` (one past the input). One past the lookahead, on success.
pub(crate) fn match_lookahead<F: Fn(u16, u16) -> bool>(
    ctx: &Ctx<'_, '_>,
    len: u16,
    matcher: &F,
    start: usize,
) -> Option<usize> {
    let mut walker = Walker::new(ctx, start - 1, true);
    for _ in 0..len {
        if !walker.next(ctx, Some(matcher)) {
            return None;
        }
    }
    Some(walker.idx + 1)
}
