//! Reading existing PDFs: page count and page boxes, the base for showing, annotating and
//! signing an uploaded file. Parsing is `hayro-syntax` (pure Rust, no clock or randomness),
//! which also repairs a damaged cross-reference table the way viewers do.

use hayro_syntax::page::Rotation;
use hayro_syntax::{LoadPdfError, Pdf};
use std::sync::Arc;

/// A PDF that could not be read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ReadError {
    /// The bytes are not a PDF this reader can parse.
    #[error("not a readable PDF")]
    Invalid,
    /// The file is encrypted with a password this reader was not given.
    #[error("the PDF is encrypted")]
    Encrypted,
}

/// A page's boxes in PDF user space (points, y up), as `[x0, y0, x1, y1]`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageBoxes {
    /// The media box: the sheet.
    pub media: [f64; 4],
    /// The crop box: what a viewer shows (the media box when the file sets none).
    pub crop: [f64; 4],
    /// Clockwise rotation in degrees: 0, 90, 180 or 270.
    pub rotation: u16,
}

/// A parsed PDF.
pub struct PdfFile {
    pdf: Pdf,
}

impl std::fmt::Debug for PdfFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PdfFile({} pages)", self.page_count())
    }
}

fn corners(r: hayro_syntax::object::Rect) -> [f64; 4] {
    [r.x0, r.y0, r.x1, r.y1]
}

impl PdfFile {
    /// Parses a file.
    pub fn parse(bytes: &[u8]) -> Result<Self, ReadError> {
        let pdf = Pdf::new(Arc::new(bytes.to_vec())).map_err(|e| match e {
            LoadPdfError::Decryption(_) => ReadError::Encrypted,
            LoadPdfError::Invalid => ReadError::Invalid,
        })?;
        Ok(Self { pdf })
    }

    /// The number of pages.
    #[must_use]
    pub fn page_count(&self) -> usize {
        self.pdf.pages().len()
    }

    /// Every page's boxes, in page order.
    #[must_use]
    pub fn page_boxes(&self) -> Vec<PageBoxes> {
        self.pdf
            .pages()
            .iter()
            .map(|page| PageBoxes {
                media: corners(page.media_box()),
                crop: corners(page.crop_box()),
                rotation: match page.rotation() {
                    Rotation::None => 0,
                    Rotation::Horizontal => 90,
                    Rotation::Flipped => 180,
                    Rotation::FlippedHorizontal => 270,
                },
            })
            .collect()
    }

    /// The parsed document, for the incremental writer.
    pub(crate) const fn pdf(&self) -> &Pdf {
        &self.pdf
    }
}
