//! Blocks onto the flow with Word's keep rules: page break before, keep with next, keep lines
//! together, widow and orphan control (never one line of a paragraph alone at either side of a
//! break), and table header rows repeated at the head of each continuation.

use super::Flow;
use crate::context::Ctx;
use crate::fragment::Block;
use crate::ir::Pt100;
use crate::LayoutError;

fn strips_height(block: &Block, count: usize) -> Pt100 {
    block.strips.iter().take(count).map(|s| s.height).sum()
}

/// The height that must fit together from the first of `blocks`: zero when it is not kept with
/// the next; else the block itself, and so on down the chain, ending with the opening lines of
/// the first block that is not.
pub fn chain_height<'b>(blocks: impl IntoIterator<Item = &'b Block>) -> Pt100 {
    let mut blocks = blocks.into_iter().peekable();
    if !blocks.peek().is_some_and(|b| b.keep_with_next) {
        return 0;
    }
    let mut height = 0;
    for block in blocks {
        if block.keep_with_next {
            height += block.height();
        } else {
            let opening = if block.widow_control { 2 } else { 1 };
            return height + block.space_before + strips_height(block, opening);
        }
    }
    height
}

/// Where a paragraph must break, given how many of its lines fit: `None` when it fits whole.
fn split_point(block: &Block, room: Pt100) -> Option<usize> {
    let mut used = 0;
    let mut fit = 0;
    for strip in &block.strips {
        if used + strip.height > room {
            break;
        }
        used += strip.height;
        fit += 1;
    }
    let len = block.strips.len();
    if fit >= len {
        return None;
    }
    if !block.widow_control || len < 2 {
        return Some(fit);
    }
    // An orphan (one line left behind) or a widow (one line carried over) moves a line.
    let fit = if len - fit == 1 {
        fit - usize::from(fit > 0)
    } else {
        fit
    };
    Some(if fit == 1 { 0 } else { fit })
}

fn place_block(flow: &mut Flow, ctx: &mut Ctx<'_, '_>, block: Block) -> Result<(), LayoutError> {
    if block.page_break_before && (flow.column_started() || flow.column > 0) {
        flow.new_page(ctx)?;
    }
    if flow.column_started() {
        flow.advance(block.space_before);
    }
    let whole = strips_height(&block, block.strips.len());
    let mut forced = None;
    if block.keep_together {
        if whole > flow.room() && whole <= flow.column_height() && flow.column_started() {
            flow.next_column(ctx)?;
        }
    } else if block.widow_control {
        forced = split_point(&block, flow.room());
        if forced == Some(0) && whole <= flow.column_height() && flow.column_started() {
            flow.next_column(ctx)?;
            forced = split_point(&block, flow.room());
        }
    }
    let head: Vec<_> = block
        .strips
        .iter()
        .take(block.repeat_head)
        .cloned()
        .collect();
    for (k, strip) in block.strips.into_iter().enumerate() {
        let must_break = forced == Some(k) && k > 0;
        let wont_fit = flow.y + strip.height > flow.bottom() && flow.column_started();
        if must_break || (wont_fit && k >= head.len() && !head.is_empty()) {
            flow.next_column(ctx)?;
            if k >= head.len() {
                for repeated in &head {
                    flow.place(ctx, repeated.clone())?;
                }
            }
        }
        flow.place(ctx, strip)?;
    }
    flow.advance(block.space_after);
    Ok(())
}

/// Sets one block, moving to the next column first when the chain it heads (its
/// [`chain_height`]) does not fit in what is left of this one.
pub fn place_chained(
    flow: &mut Flow,
    ctx: &mut Ctx<'_, '_>,
    block: Block,
    chain: Pt100,
) -> Result<(), LayoutError> {
    let starting_room = flow.room()
        - if flow.column_started() {
            block.space_before
        } else {
            0
        };
    if chain > starting_room && chain <= flow.column_height() && flow.column_started() {
        flow.next_column(ctx)?;
    }
    place_block(flow, ctx, block)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fragment::Fragment;

    fn lines(n: usize) -> Block {
        Block {
            space_before: 0,
            space_after: 0,
            strips: (0..n).map(|_| Fragment::empty(100)).collect(),
            keep_together: false,
            keep_with_next: false,
            page_break_before: false,
            widow_control: true,
            repeat_head: 0,
        }
    }

    #[test]
    fn a_single_line_is_never_left_behind_or_carried_over() {
        assert_eq!(split_point(&lines(5), 500), None, "fits whole");
        assert_eq!(split_point(&lines(5), 100), Some(0), "orphan: move it all");
        assert_eq!(split_point(&lines(5), 400), Some(3), "widow: carry two");
        assert_eq!(split_point(&lines(5), 300), Some(3), "a clean split stays");
        assert_eq!(
            split_point(&lines(2), 100),
            Some(0),
            "two lines stay together"
        );
    }

    #[test]
    fn without_widow_control_any_split_is_taken() {
        let mut block = lines(5);
        block.widow_control = false;
        assert_eq!(split_point(&block, 100), Some(1));
        assert_eq!(split_point(&block, 400), Some(4));
    }

    #[test]
    fn a_heading_chain_needs_its_next_paragraphs_opening_lines() {
        let mut heading = lines(1);
        heading.keep_with_next = true;
        let blocks = vec![heading, lines(5)];
        assert_eq!(chain_height(&blocks), 300);
        assert_eq!(chain_height(&blocks[1..]), 0, "not kept: no chain");
    }
}
