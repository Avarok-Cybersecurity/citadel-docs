//! Reading the editor: the document, pages, rectangles, hit tests and display lists.

use super::Editor;
use crate::error::EditError;
use crate::position::{Position, Selection};
use cdoc_crdt::Doc;
use cdoc_layout::{Layout, Pt100, Rect};
use cdoc_model::Document;

impl Editor<'_> {
    /// The replica.
    #[must_use]
    pub const fn doc(&self) -> &Doc {
        &self.doc
    }

    /// The document as the model sees it (kept equal to `to_model` of the replica).
    #[must_use]
    pub const fn model(&self) -> &Document {
        &self.model
    }

    /// The current layout.
    ///
    /// # Panics
    /// Never: an editor exists only with a layout.
    #[must_use]
    pub fn layout(&self) -> &Layout {
        self.session
            .layout()
            .expect("an editor always holds a layout")
    }

    /// The selection.
    #[must_use]
    pub const fn selection(&self) -> Selection {
        self.selection
    }

    /// Character properties the next typed text will get on top of what it inherits.
    #[must_use]
    pub const fn pending_marks(&self) -> Option<&cdoc_model::RunProperties> {
        self.pending.as_ref()
    }

    /// Pages.
    #[must_use]
    pub fn page_count(&self) -> usize {
        self.layout().pages.len()
    }

    /// Page `i`'s width and height.
    #[must_use]
    pub fn page_size(&self, i: usize) -> Option<(Pt100, Pt100)> {
        self.layout().pages.get(i).map(|p| (p.width, p.height))
    }

    /// The caret rectangle (zero width, line height) of `pos`, with its page.
    pub fn caret_rect(&self, pos: Position) -> Result<(usize, Rect), EditError> {
        self.geometry.caret(&self.doc, pos)
    }

    /// The rectangles that highlight `selection`, one per line, with their pages.
    pub fn selection_rects(&self, selection: Selection) -> Result<Vec<(usize, Rect)>, EditError> {
        Ok(self.geometry.rects(&self.doc, selection.range(&self.doc)?))
    }

    /// The selection's rectangles on `page` alone: for a page scrolled into view.
    pub fn selection_rects_on(&self, page: usize) -> Result<Vec<Rect>, EditError> {
        let range = self.selection.range(&self.doc)?;
        Ok(self
            .geometry
            .rects_on(&self.doc, range, |p| p == page)
            .into_iter()
            .map(|(_, r)| r)
            .collect())
    }

    /// The rectangles a result carries: on the viewport's and the dirty pages once the UI set
    /// a viewport, on every page before.
    fn result_rects(&self, dirty_pages: &[usize]) -> Result<Vec<(usize, Rect)>, EditError> {
        let range = self.selection.range(&self.doc)?;
        Ok(match &self.viewport {
            None => self.geometry.rects(&self.doc, range),
            Some(view) => self.geometry.rects_on(&self.doc, range, |p| {
                view.contains(&p) || dirty_pages.contains(&p)
            }),
        })
    }

    /// The position nearest `(x, y)` on page `page` (page space, hundredths of a point).
    pub fn hit_test(&self, page: usize, x: Pt100, y: Pt100) -> Result<Position, EditError> {
        if page >= self.page_count() {
            return Err(EditError::NoPage(page));
        }
        match self.geometry.hit(page, x, y) {
            Some(p) => Ok(p),
            None => Position::at(&self.doc, crate::position::caret_gap(&self.doc, 0)?)
                .ok_or(EditError::UnknownPosition),
        }
    }

    /// The display list of `pages` (`cdoc_paint::encode_pages`).
    pub fn paint(&self, pages: &[usize]) -> Result<Vec<u8>, EditError> {
        Ok(cdoc_paint::encode_pages(self.layout(), self.fonts, pages)?)
    }

    /// Every caret stop on page `page`: the body's in document order, then the headers',
    /// footers' and footnotes' (for tests and accessibility).
    #[must_use]
    pub fn stops(&self, page: usize) -> Vec<(Position, Rect)> {
        self.geometry
            .pages
            .get(page)
            .and_then(Option::as_ref)
            .map(|m| {
                m.lines
                    .iter()
                    .chain(&m.aside)
                    .flat_map(|l| l.stops.iter().map(move |s| (s.pos, l.caret(s.x))))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The full snapshot of the replica.
    #[must_use]
    pub fn snapshot(&self) -> Vec<u8> {
        self.doc.encode_snapshot()
    }

    /// The replica's state vector, binary.
    #[must_use]
    pub fn state_vector(&self) -> Vec<u8> {
        self.doc.encode_state_vector()
    }

    /// What a peer at state vector `sv` (binary) lacks.
    pub fn encode_diff(&self, sv: &[u8]) -> Result<Vec<u8>, EditError> {
        Ok(self.doc.encode_diff(&cdoc_crdt::StateVector::decode(sv)?))
    }
}

impl Editor<'_> {
    /// A local request's `applied` with the caret, selection rectangles and formatting, as the
    /// wire carries it.
    pub fn outcome(
        &self,
        update: Vec<u8>,
        dirty_pages: Vec<usize>,
        created: Vec<crate::Created>,
    ) -> Result<crate::wire::Outcome, EditError> {
        let rects = self.result_rects(&dirty_pages)?;
        Ok(crate::wire::Outcome {
            update,
            dirty_pages,
            created,
            page_count: self.page_count(),
            selection: self.selection,
            caret: self.caret_rect(self.selection.head).ok(),
            rects,
            formatting: self.caret_formatting()?,
            section: self.caret_section()?.clone(),
            selection_text: self.capped_selection_text()?,
            remote: None,
            story: self.story()?,
        })
    }
}
