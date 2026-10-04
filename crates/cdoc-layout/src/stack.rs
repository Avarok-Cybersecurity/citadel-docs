//! Blocks stacked in a box with no page breaks: a table cell, a header, a footer, a footnote.
//! The result is one strip the flow (or an enclosing table) places whole.

use crate::context::Ctx;
use crate::fragment::{Block, Fragment};
use crate::ir::Pt100;
use crate::labels::Counters;
use crate::LayoutError;

/// One model block as a flow block.
pub fn block(
    ctx: &mut Ctx<'_, '_>,
    block: &cdoc_model::Block,
    width: Pt100,
    counters: &mut Counters<'_>,
) -> Result<Block, LayoutError> {
    match block {
        cdoc_model::Block::Paragraph(p) => crate::paragraph::paragraph(ctx, p, width, counters),
        cdoc_model::Block::Table(t) => crate::table::table(ctx, t, width, counters),
    }
}

/// Flow blocks stacked top to bottom into one strip.
#[must_use]
pub fn stack_blocks(blocks: Vec<Block>) -> Fragment {
    let mut out = Fragment::empty(0);
    let mut y = 0;
    for (i, block) in blocks.into_iter().enumerate() {
        if i > 0 {
            y += block.space_before;
        }
        for strip in block.strips {
            let h = strip.height;
            out.absorb(strip, 0, y);
            y += h;
        }
        y += block.space_after;
    }
    out.height = y;
    out
}

/// Model blocks laid out and stacked into one strip `width` wide.
pub fn stack(
    ctx: &mut Ctx<'_, '_>,
    blocks: &[cdoc_model::Block],
    width: Pt100,
    counters: &mut Counters<'_>,
) -> Result<Fragment, LayoutError> {
    let laid = blocks
        .iter()
        .map(|b| block(ctx, b, width, counters))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(stack_blocks(laid))
}
