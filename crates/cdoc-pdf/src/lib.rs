//! Citadel Docs PDF and SVG backends over the `cdoc-paint` display list, and the base of the
//! annotate and sign layer: reading existing PDFs and appending incremental updates to them.
//!
//! One layout, one description: [`render_pdf`] lowers the layout to its display list
//! (`cdoc-paint`) and writes the file from the list ([`render_display_list`]), and
//! [`svg::pages_svg`] paints the in-app preview from the same list. The canvas editor paints
//! it too, so no painter can draw a page differently from the others.
//!
//! Pure: faces arrive through `cdoc_layout::FontPort`, images through [`ImagePort`], the time
//! through [`ClockPort`]. Nothing here reads a file, the clock or randomness.

#![forbid(unsafe_code)]

mod font;
mod image;
pub mod incremental;
mod meta;
mod ops;
mod pdf;
pub mod picture;
pub mod ports;
pub mod read;
mod scan;
pub mod svg;
mod text;

pub use image::unrendered;
pub use incremental::{append_annotations, Annotation, AnnotationKind, UpdateError};
pub use meta::allowed_link;
pub use ports::{ClockPort, FixedClock, ImageBytes, ImageError, ImageLibrary, ImagePort};
pub use read::{PageBoxes, PdfFile, ReadError};

use cdoc_layout::{FontPort, Layout};

/// Why a layout could not be written.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PdfError {
    /// A face could not be embedded.
    #[error(transparent)]
    Embed(#[from] font::EmbedError),
    /// The image port had no image for a content id.
    #[error(transparent)]
    MissingImage(#[from] ImageError),
    /// An image's bytes are not what its type says.
    #[error("image {0:?}: {1}")]
    Image(String, picture::PictureError),
    /// The display list could not be built or painted.
    #[error(transparent)]
    Paint(#[from] cdoc_paint::PaintError),
    /// The clock port's time is not RFC 3339.
    #[error("the clock gave {0:?}, which is not an RFC 3339 time")]
    Clock(String),
    /// A PDF is written from a full display list; this one holds page `0` out of place.
    #[error("the display list is partial (page {0} is out of place); a PDF needs every page")]
    PartialList(u32),
    /// A heading names a page the display list does not have.
    #[error("a heading is on page {0}, which the display list does not have")]
    NoOutlinePage(u32),
}

/// The layout as a PDF file: its display list (`cdoc_paint::lower`), written by the same
/// writer as [`render_display_list`]. The list stays in its owned form, so it is not encoded
/// only to be decoded again.
pub fn render_pdf(
    layout: &Layout,
    fonts: &dyn FontPort,
    images: &dyn ImagePort,
    clock: &dyn ClockPort,
) -> Result<Vec<u8>, PdfError> {
    let all: Vec<usize> = (0..layout.pages.len()).collect();
    let list = cdoc_paint::lower(layout, fonts, &all)?;
    pdf::render(&list, fonts, images, clock)
}

/// A full display list (`cdoc_paint::encode`, every page in order) as a PDF file. `fonts`
/// serves the faces the list's face table names, for embedding.
pub fn render_display_list(
    list: &[u8],
    fonts: &dyn FontPort,
    images: &dyn ImagePort,
    clock: &dyn ClockPort,
) -> Result<Vec<u8>, PdfError> {
    let view = cdoc_paint::decode(list).map_err(cdoc_paint::PaintError::from)?;
    let list = view.to_owned().map_err(cdoc_paint::PaintError::from)?;
    pdf::render(&list, fonts, images, clock)
}
