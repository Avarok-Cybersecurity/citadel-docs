//! Layout after an edit: the incremental relayout and the line maps of the changed pages.

use super::Editor;
use crate::error::EditError;
use crate::geometry::derive::Ctx as GeoCtx;
use cdoc_layout::Dirty;

impl Editor<'_> {
    /// Lays out again and refreshes the line maps of the pages that changed.
    pub(crate) fn relayout(&mut self, dirty: &Dirty) -> Result<Vec<usize>, EditError> {
        let all = self.relayout_all || dirty.everything;
        let dirty = if all { &Dirty::all() } else { dirty };
        self.relayout_all = true;
        let changed = self
            .session
            .relayout(&self.model, dirty)?
            .changed_pages()
            .to_vec();
        self.relayout_all = false;
        let layout = self
            .session
            .layout()
            .ok_or(EditError::Layout(cdoc_layout::LayoutError::NoBaseline))?;
        let count = layout.pages.len();
        if all {
            self.geometry.clear(count);
        } else {
            self.geometry.invalidate(&changed, count);
        }
        let ctx = GeoCtx {
            doc: &self.doc,
            layout,
            outline: &self.outline,
        };
        self.geometry.derive_missing(&ctx)?;
        Ok(if all { (0..count).collect() } else { changed })
    }
}
