//! Page setup: a page-setup patch on every section the selection touches.

use super::{done, Ctx, Outcome};
use crate::error::EditError;
use crate::outline::Owner;
use cdoc_crdt::{atom_at, Atom};
use cdoc_model::PageSetupPatch;

/// The section the gap `gap` is in.
fn section_of(ctx: &Ctx<'_, '_>, gap: usize) -> Result<usize, EditError> {
    let node = ctx
        .tx
        .doc
        .blocks_before(gap)
        .checked_sub(1)
        .ok_or(EditError::UnknownPosition)?;
    match ctx.outline.nodes.get(node).map(|n| n.owner) {
        Some(Owner::Top { section, .. } | Owner::Section(section) | Owner::Furniture(section)) => {
            Ok(section)
        }
        Some(Owner::Stray) => Err(EditError::Irregular),
        Some(Owner::Footnote) | None => {
            Err(EditError::NotApplicable("the position is not in a section"))
        }
    }
}

/// Applies `patch` to the page setup of each section from the anchor's to the head's.
pub(crate) fn set_page(
    ctx: &mut Ctx<'_, '_>,
    patch: &PageSetupPatch,
) -> Result<Outcome, EditError> {
    if patch.is_none() {
        return Err(EditError::InvalidArgument("the patch sets nothing"));
    }
    ctx.require_regular()?;
    let range = ctx.selection.range(ctx.tx.doc)?;
    let (first, last) = (section_of(ctx, range.start)?, section_of(ctx, range.end)?);
    for section in first..=last {
        let id = ctx.outline.nodes[ctx.outline.sections[section].0].id;
        let at = ctx
            .tx
            .doc
            .locate(id)
            .ok_or(EditError::UnknownPosition)?
            .index;
        let Some(Atom::Section(mut props)) = atom_at(ctx.tx.doc, at)? else {
            return Err(EditError::UnknownPosition);
        };
        props.page = patch.apply(&props.page);
        if !props.page.problems(section).is_empty() {
            return Err(EditError::InvalidArgument("the page setup is not valid"));
        }
        ctx.tx.set_block(at, &Atom::Section(props))?;
    }
    Ok(done(ctx.selection))
}
