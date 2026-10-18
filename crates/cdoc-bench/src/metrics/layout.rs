//! Layout: a fresh full layout, and the incremental relayout after one keystroke. The keystroke
//! alternates between the document and its one-character-longer copy, so every timed relayout
//! follows a real one-block edit to a warm session.

use super::{paged, BenchError, Ctx, Family, Fixture, Results};
use crate::corpus::{keystroke_target, typed};
use crate::schema::Measurement;
use cdoc_layout::test_support::default_fonts;
use cdoc_layout::{layout, BlockEdit, Dirty, LayoutOptions, LayoutSession};
use std::hint::black_box;

/// The smallest document the keystroke metric runs on: a one-page document has no middle worth
/// measuring an edit in.
pub const KEYSTROKE_MIN_PAGES: u32 = 10;

pub(super) fn run(ctx: &Ctx<'_>, fx: &Fixture, out: &mut Results) -> Result<(), BenchError> {
    let fonts = default_fonts();
    let options = LayoutOptions::word_like();

    let key = paged(Family::LayoutFull, fx.pages);
    let t = ctx.time(&key, || {
        black_box(layout(black_box(&fx.doc), &fonts, &options)?);
        Ok(())
    })?;
    out.insert(key, Measurement::Time(t));

    if fx.pages < KEYSTROKE_MIN_PAGES {
        return Ok(());
    }
    let index = keystroke_target(&fx.doc)
        .ok_or_else(|| BenchError::Corpus(format!("{}p: no body paragraph", fx.pages)))?;
    let edited = typed(&fx.doc, index, "x")
        .ok_or_else(|| BenchError::Corpus(format!("{}p: block {index} has no text", fx.pages)))?;
    let dirty = Dirty::edits(vec![BlockEdit::changed(0, index)]);
    let mut session = LayoutSession::new(&fonts, &options);
    session.relayout(&fx.doc, &Dirty::all())?;
    let mut flip = false;
    let key = paged(Family::LayoutKeystroke, fx.pages);
    let t = ctx.time(&key, || {
        flip = !flip;
        let next = if flip { &edited } else { &fx.doc };
        black_box(
            session
                .relayout(black_box(next), &dirty)?
                .changed_pages()
                .len(),
        );
        Ok(())
    })?;
    out.insert(key, Measurement::Time(t));
    Ok(())
}
