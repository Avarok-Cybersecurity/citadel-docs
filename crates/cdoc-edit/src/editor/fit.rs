//! The size an image is inserted at when the caller gives none: its own size at 96 pixels per
//! inch (a pixel is 0.75 pt), scaled down to the width of a column of the caret's section,
//! keeping its aspect ratio. Never scaled up. Only images the editor holds have a known
//! pixel size.

use super::Editor;
use crate::error::EditError;
use cdoc_crdt::AtomKind;
use cdoc_model::image_probe::probe;
use cdoc_model::{ContentId, ImagePort, Length, PageSetup};

/// Hundredths of a point per pixel at 96 pixels per inch.
const PT100_PER_PIXEL: i64 = 75;

/// The width of one column of `page`.
fn column_width(page: &PageSetup) -> i64 {
    let body = i64::from(page.size.width.0 - page.margins.left.0 - page.margins.right.0);
    let count = i64::from(page.columns.count.max(1));
    (body - i64::from(page.columns.gap.0) * (count - 1)) / count
}

/// `(width, height)` pixels at 96 per inch, no wider than `column`.
pub(crate) fn fitted(width: u32, height: u32, column: i64) -> Result<(Length, Length), EditError> {
    let (w, h) = (
        i64::from(width) * PT100_PER_PIXEL,
        i64::from(height) * PT100_PER_PIXEL,
    );
    let (w, h) = if w > column {
        (column, (h * column / w).max(1))
    } else {
        (w, h)
    };
    let length = |v: i64| {
        i32::try_from(v)
            .map(Length)
            .map_err(|_| EditError::InvalidArgument("the image is too large to insert"))
    };
    if w < 1 {
        return Err(EditError::NotApplicable("the column has no width"));
    }
    Ok((length(w)?, length(h)?))
}

impl Editor<'_> {
    /// The size [`crate::Command::InsertImage`] without a size inserts `id` at.
    pub(crate) fn fitted(&self, id: &ContentId) -> Result<(Length, Length), EditError> {
        let image = self.images.image(id).map_err(|_| {
            EditError::NotApplicable("the editor does not hold that image: give its size")
        })?;
        let header = probe(image.bytes)?;
        let at = self.selection.head.index(&self.doc)?;
        let section = self
            .outline
            .nodes
            .iter()
            .filter(|n| n.kind == AtomKind::Section)
            .filter_map(|n| self.doc.locate(n.id).map(|l| l.index))
            .filter(|&i| i < at)
            .count()
            .saturating_sub(1);
        let page = &self
            .model
            .sections
            .get(section)
            .ok_or(EditError::UnknownPosition)?
            .properties
            .page;
        fitted(header.width, header.height, column_width(page))
    }
}
