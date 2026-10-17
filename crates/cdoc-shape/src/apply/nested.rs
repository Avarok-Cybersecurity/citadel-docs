//! Nested lookups of a contextual rule: each runs at a matched position, and the positions
//! after it are kept right as it grows or shrinks the run.

use super::matching::{Matched, MAX_CONTEXT};
use super::Ctx;
use ttf_parser::opentype_layout::SequenceLookupRecord;
use ttf_parser::LazyArray16;

/// Runs each record's lookup at its matched position, keeping the remaining positions right
/// as nested lookups grow or shrink the run.
pub(crate) fn apply_nested(
    ctx: &mut Ctx<'_, '_>,
    matched: &Matched,
    lookups: LazyArray16<'_, SequenceLookupRecord>,
) {
    let Matched {
        mut positions,
        mut count,
        end: match_end,
        ..
    } = *matched;
    let backtrack = ctx.buf.backtrack_len();
    let delta0 = isize_of(backtrack) - isize_of(ctx.buf.idx);
    for p in &mut positions[..count] {
        *p = usize_of(isize_of(*p) + delta0);
    }
    let mut end = isize_of(backtrack) + isize_of(match_end) - isize_of(ctx.buf.idx);
    for record in lookups {
        if ctx.buf.failed {
            break;
        }
        let idx = usize::from(record.sequence_index);
        if idx >= count {
            continue;
        }
        let orig_len = ctx.buf.backtrack_len() + ctx.buf.lookahead_len();
        if positions[idx] >= orig_len {
            continue;
        }
        if !ctx.buf.move_to(positions[idx]) || ctx.buf.max_ops <= 0 {
            break;
        }
        if !ctx.recurse(record.lookup_list_index) {
            continue;
        }
        let new_len = ctx.buf.backtrack_len() + ctx.buf.lookahead_len();
        let mut delta = isize_of(new_len) - isize_of(orig_len);
        if delta == 0 {
            continue;
        }
        end += delta;
        if end < isize_of(positions[idx]) {
            delta += isize_of(positions[idx]) - end;
            end = isize_of(positions[idx]);
        }
        let mut next = idx + 1;
        if delta > 0 {
            if usize_of(delta) + count > MAX_CONTEXT {
                break;
            }
        } else {
            delta = delta.max(isize_of(next) - isize_of(count));
            next = usize_of(isize_of(next) - delta);
        }
        let to = usize_of(isize_of(next) + delta);
        positions.copy_within(next..count, to);
        next = to;
        count = usize_of(isize_of(count) + delta);
        for j in idx + 1..next {
            positions[j] = positions[j - 1] + 1;
        }
        for p in &mut positions[next..count] {
            *p = usize_of(isize_of(*p) + delta);
        }
    }
    ctx.buf.move_to(usize_of(end.max(0)));
}

fn isize_of(v: usize) -> isize {
    isize::try_from(v).unwrap_or(isize::MAX)
}

fn usize_of(v: isize) -> usize {
    usize::try_from(v).unwrap_or(0)
}
