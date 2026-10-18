//! The editor's result record with the whole body of the 500-page document selected: the
//! worst case of the keystroke rule, since the record carries the caret's formatting (mixed
//! over the selection), the selection's text (capped) and its rectangles, those for a
//! three-page viewport only.

use super::{paged, BenchError, Ctx, Family, Fixture, Results};
use crate::schema::Measurement;
use cdoc_crdt::ClientId;
use cdoc_edit::wire::{encode_result, Request};
use cdoc_edit::{Command, Editor};
use cdoc_layout::test_support::default_fonts;
use cdoc_layout::LayoutOptions;
use std::hint::black_box;

/// The document the metric runs on.
pub const SELECT_ALL_PAGES: u32 = 500;

/// The pages on screen.
const VIEWPORT: [usize; 3] = [0, 1, 2];

fn edit(e: impl std::fmt::Display) -> BenchError {
    BenchError::Crdt(format!("editor: {e}"))
}

pub(super) fn run(ctx: &Ctx<'_>, fx: &Fixture, out: &mut Results) -> Result<(), BenchError> {
    if fx.pages != SELECT_ALL_PAGES {
        return Ok(());
    }
    let fonts = default_fonts();
    let options = LayoutOptions::word_like();
    let mut editor = Editor::from_document(&fx.doc, ClientId(1), &fonts, &options).map_err(edit)?;
    editor
        .handle(Request::SetViewport(VIEWPORT.to_vec()))
        .map_err(edit)?;
    editor.apply(Command::SelectAll).map_err(edit)?;
    let key = paged(Family::KeystrokeSelectAll, fx.pages);
    let t = ctx.time(&key, || {
        let outcome = editor
            .outcome(Vec::new(), Vec::new(), Vec::new())
            .map_err(edit)?;
        black_box(encode_result(&outcome).len());
        Ok(())
    })?;
    out.insert(key, Measurement::Time(t));
    Ok(())
}
