//! Inline objects and annotations: images, footnotes, links and comments. Ids, authors and
//! dates come from the caller (the editor has no clock or randomness).

use super::typing::inline;
use super::{done, Created, Ctx, Outcome};
use crate::error::EditError;
use crate::fresh::Purpose;
use crate::position::Selection;
use crate::text::{paragraph, word_at};
use cdoc_crdt::{slice, Atom, DocAttr, Mark, Piece};
use cdoc_model::{
    Comment, ContentId, FootnoteId, InlineContent, InlineImage, Length, Link, ParagraphProperties,
};
use std::ops::Range;

/// An inline image at the caret.
pub(crate) fn image(
    ctx: &mut Ctx<'_, '_>,
    content_id: ContentId,
    width: Length,
    height: Length,
) -> Result<Outcome, EditError> {
    if !width.is_positive() || !height.is_positive() {
        return Err(EditError::InvalidArgument("an image needs a positive size"));
    }
    inline(
        ctx,
        InlineContent::Image(InlineImage {
            content_id,
            width,
            height,
            alt: None,
        }),
    )
}

/// A footnote reference at the caret, and its empty body after everything else; under the
/// caller's id, or one the editor picks.
pub(crate) fn footnote(
    ctx: &mut Ctx<'_, '_>,
    id: Option<FootnoteId>,
) -> Result<Outcome, EditError> {
    let taken = |v: u32| ctx.model.footnotes.iter().any(|f| f.id.0 == v);
    let (id, created) = match id {
        Some(id) if taken(id.0) => {
            return Err(EditError::InvalidArgument("the footnote id is taken"));
        }
        Some(id) => (id, Vec::new()),
        None => {
            let id = FootnoteId(crate::fresh::id(ctx.tx.doc, Purpose::Footnote, taken));
            (id, vec![Created::Footnote(id)])
        }
    };
    ctx.require_regular()?;
    let outcome = inline(ctx, InlineContent::FootnoteRef { id })?;
    let style = cdoc_model::builtin::ids::footnote_text();
    let style = ctx.model.styles.get(&style).map(|s| s.id.clone());
    let end = ctx.tx.doc.len();
    let body = [
        Atom::Footnote(id),
        Atom::Paragraph(ParagraphProperties {
            style,
            ..ParagraphProperties::none()
        }),
        Atom::End,
    ];
    for (k, atom) in body.iter().enumerate() {
        ctx.tx.insert_atom(end + k, atom)?;
    }
    Ok(Outcome { created, ..outcome })
}

/// Links the selection, or inserts the URL as linked text at the caret.
pub(crate) fn link(ctx: &mut Ctx<'_, '_>, href: String) -> Result<Outcome, EditError> {
    if href.is_empty() {
        return Err(EditError::InvalidArgument("an empty link target"));
    }
    let mut range = ctx.selection.range(ctx.tx.doc)?;
    let mut selection = ctx.selection;
    if range.is_empty() {
        let outcome = super::typing::insert_text(ctx, &href)?;
        range = range.start..range.start + href.chars().count();
        selection = outcome.selection;
    }
    ctx.tx
        .format(range, &Mark::Link(Some(Link { href, title: None })), true)?;
    Ok(done(selection))
}

/// The linked run around caret gap `gap` (adjacent pieces with the same link).
fn link_around(ctx: &Ctx<'_, '_>, gap: usize) -> Result<Option<Range<usize>>, EditError> {
    let (_, para) = paragraph(ctx.tx.doc, &ctx.outline, gap)?;
    let mut runs: Vec<(Range<usize>, Option<Link>)> = Vec::new();
    let mut at = para.start;
    for piece in slice(ctx.tx.doc, para)? {
        let len = crate::tx::piece_len(&piece);
        if let Piece::Inline(i) = piece {
            match runs.last_mut() {
                Some((r, l)) if *l == i.marks.link => r.end += len,
                _ => runs.push((at..at + len, i.marks.link)),
            }
        }
        at += len;
    }
    Ok(runs
        .into_iter()
        .find(|(r, l)| {
            l.is_some() && r.start <= gap && gap <= r.end && (r.contains(&gap) || r.end == gap)
        })
        .map(|(r, _)| r))
}

/// Removes the link from the selection, or the whole link around the caret.
pub(crate) fn unlink(ctx: &mut Ctx<'_, '_>) -> Result<Outcome, EditError> {
    let range = ctx.selection.range(ctx.tx.doc)?;
    let range = if range.is_empty() {
        link_around(ctx, range.start)?.ok_or(EditError::NotApplicable("there is no link here"))?
    } else {
        range
    };
    ctx.tx.format(range, &Mark::Link(None), true)?;
    Ok(done(ctx.selection))
}

/// Anchors `comment` on the selection or on the word at the caret.
pub(crate) fn comment(ctx: &mut Ctx<'_, '_>, comment: Comment) -> Result<Outcome, EditError> {
    if ctx.model.comments.iter().any(|c| c.id == comment.id) {
        return Err(EditError::InvalidArgument("the comment id is taken"));
    }
    let mut range = ctx.selection.range(ctx.tx.doc)?;
    if range.is_empty() {
        let (_, para) = paragraph(ctx.tx.doc, &ctx.outline, range.start)?;
        range = word_at(ctx.tx.doc, range.start, &para);
    }
    if range.is_empty() {
        return Err(EditError::NotApplicable("there is nothing to comment on"));
    }
    ctx.tx.format(
        range.clone(),
        &Mark::Comment {
            id: comment.id.clone(),
            anchored: true,
        },
        true,
    )?;
    let mut all = ctx.model.comments.clone();
    let previous = DocAttr::Comments(all.clone());
    all.push(comment);
    ctx.tx.set_attr(&DocAttr::Comments(all), previous)?;
    let doc = &*ctx.tx.doc;
    let at = |g| crate::position::Position::at(doc, g).ok_or(EditError::UnknownPosition);
    Ok(done(Selection {
        anchor: at(range.start)?,
        head: at(range.end)?,
    }))
}
