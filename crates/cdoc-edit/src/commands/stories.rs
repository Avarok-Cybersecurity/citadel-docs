//! Entering and leaving the stories that are not the body: a section's headers and footers,
//! and the footnote bodies (`story.rs`). The model has no "link to previous": each section
//! owns its headers and footers, a section without one shows none, and editing one edits that
//! section's definition, which every page of the section shows.

use super::{done, Ctx, Outcome};
use crate::error::EditError;
use crate::position::{Direction, Selection};
use crate::story::{Stories, Story};
use cdoc_crdt::{Atom, AtomKind, Furniture, Unit};
use cdoc_model::{builtin::ids, FootnoteId, HeaderFooterKind, InlineContent, ParagraphProperties};

fn stories<'d>(ctx: &'d Ctx<'_, '_>) -> Result<Stories<'d>, EditError> {
    Stories::new(ctx.tx.doc, &ctx.outline)
}

/// The caret at the start of `story`, when it exists.
fn enter(ctx: &Ctx<'_, '_>, story: Story) -> Result<Option<Outcome>, EditError> {
    match stories(ctx)?.bounds(story) {
        Some((first, _)) => Ok(Some(done(ctx.caret_at(first)?))),
        None => Ok(None),
    }
}

/// The caret in a section's header or footer of `kind`, adding an empty one when the section
/// has none. Refused for a kind the section never shows (a first-page header without a
/// distinct title page, an even-page one without distinct odd and even pages).
pub(crate) fn header_footer(
    ctx: &mut Ctx<'_, '_>,
    furniture: Furniture,
    section: u32,
    kind: HeaderFooterKind,
) -> Result<Outcome, EditError> {
    let s = usize::try_from(section).map_err(|_| EditError::InvalidArgument("no such section"))?;
    let props = &ctx
        .model
        .sections
        .get(s)
        .ok_or(EditError::InvalidArgument("no such section"))?
        .properties;
    let shown = match kind {
        HeaderFooterKind::Default => true,
        HeaderFooterKind::First => props.title_page,
        HeaderFooterKind::Even => ctx.model.settings.even_and_odd_headers,
    };
    if !shown {
        return Err(EditError::NotApplicable(
            "the section shows no header or footer of that kind",
        ));
    }
    let story = match furniture {
        Furniture::Header => Story::Header { section, kind },
        Furniture::Footer => Story::Footer { section, kind },
    };
    if let Some(outcome) = enter(ctx, story)? {
        return Ok(outcome);
    }
    ctx.require_regular()?;
    let (node, _) = ctx.outline.sections.get(s).ok_or(EditError::Irregular)?;
    let start = ctx
        .tx
        .doc
        .locate(ctx.outline.nodes[*node].id)
        .ok_or(EditError::UnknownPosition)?
        .index;
    let style = match furniture {
        Furniture::Header => ids::header(),
        Furniture::Footer => ids::footer(),
    };
    let style = ctx.model.styles.get(&style).map(|s| s.id.clone());
    let atoms = [
        Atom::HeaderFooter { furniture, kind },
        Atom::Paragraph(ParagraphProperties {
            style,
            ..ParagraphProperties::none()
        }),
        Atom::End,
    ];
    for (k, atom) in atoms.iter().enumerate() {
        ctx.tx.insert_atom(start + 1 + k, atom)?;
    }
    ctx.refresh();
    enter(ctx, story)?.ok_or(EditError::UnknownPosition)
}

/// The caret at the start of footnote `id`'s body.
pub(crate) fn footnote(ctx: &Ctx<'_, '_>, id: FootnoteId) -> Result<Outcome, EditError> {
    enter(ctx, Story::Footnote(id))?.ok_or(EditError::NotApplicable("no such footnote"))
}

/// The gap after footnote `id`'s reference, if the body names one.
fn reference(ctx: &Ctx<'_, '_>, id: FootnoteId) -> Result<Option<usize>, EditError> {
    let doc = &*ctx.tx.doc;
    for (i, (_, unit)) in doc.units_from(0).enumerate() {
        if unit != Unit::Atom(AtomKind::Inline) {
            continue;
        }
        if let Some(Atom::Inline(InlineContent::FootnoteRef { id: at })) =
            cdoc_crdt::atom_at(doc, i)?
        {
            if at == id {
                return Ok(Some(i + 1));
            }
        }
    }
    Ok(None)
}

/// Back to the body: from a footnote to just after its reference; otherwise to the body
/// selection held before the story was entered, or the body's nearest gap.
pub(crate) fn body(ctx: &Ctx<'_, '_>, saved: Option<Selection>) -> Result<Outcome, EditError> {
    let all = stories(ctx)?;
    let head = ctx.selection.head.index(ctx.tx.doc)?;
    let story = all.at(head);
    if story == Story::Body {
        return Ok(done(ctx.selection));
    }
    if let Story::Footnote(id) = story {
        if let Some(gap) = reference(ctx, id)? {
            return Ok(done(ctx.caret_at(gap)?));
        }
    }
    if let Some(s) = saved {
        let (a, h) = (s.anchor.index(ctx.tx.doc)?, s.head.index(ctx.tx.doc)?);
        if all.at(a) == Story::Body && all.at(h) == Story::Body {
            return Ok(done(s));
        }
    }
    let dir = match story {
        Story::Footnote(_) => Direction::Backward,
        _ => Direction::Forward,
    };
    let gap = all
        .clamp(Story::Body, head, dir)
        .ok_or(EditError::UnknownPosition)?;
    Ok(done(ctx.caret_at(gap)?))
}
