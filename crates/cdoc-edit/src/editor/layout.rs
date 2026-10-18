//! Layout after an edit: the incremental relayout and the line maps of the changed pages.

use super::Editor;
use crate::error::EditError;
use crate::geometry::derive::{BodyBand, Ctx as GeoCtx, Fallback};
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
        let fallback = self.fallback()?;
        let ctx = GeoCtx {
            doc: &self.doc,
            layout,
            fonts: self.fonts,
            fallback,
            asides: self.asides(),
            band: self.band(),
        };
        self.geometry.derive_missing(&ctx)?;
        Ok(if all { (0..count).collect() } else { changed })
    }

    /// Where every header, footer and footnote body starts.
    fn asides(&self) -> Vec<usize> {
        self.outline
            .nodes
            .iter()
            .filter(|n| {
                matches!(
                    n.kind,
                    cdoc_crdt::AtomKind::HeaderFooter | cdoc_crdt::AtomKind::Footnote
                )
            })
            .filter_map(|n| self.doc.locate(n.id).map(|l| l.index))
            .collect()
    }

    /// The body band over every section (a document has at least one, as `fallback` reads).
    fn band(&self) -> BodyBand {
        let first = &self.model.sections[0].properties.page.margins;
        let start = BodyBand {
            top: first.top.0.abs(),
            bottom: first.bottom.0.abs(),
        };
        self.model.sections.iter().fold(start, |b, s| {
            let m = &s.properties.page.margins;
            BodyBand {
                top: b.top.min(m.top.0.abs()),
                bottom: b.bottom.min(m.bottom.0.abs()),
            }
        })
    }

    fn fallback(&self) -> Result<Fallback, EditError> {
        let page = &self.model.sections[0].properties.page;
        let size = self
            .model
            .styles
            .defaults
            .run
            .size
            .ok_or(EditError::Layout(cdoc_layout::LayoutError::NoFont))?;
        let height = i32::from(size.0) * 60;
        Ok(Fallback {
            x: page.margins.left.0,
            y: page.margins.top.0,
            height,
        })
    }
}
