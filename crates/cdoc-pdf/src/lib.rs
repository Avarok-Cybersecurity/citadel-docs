//! Citadel Docs PDF and SVG backends over the `cdoc-layout` IR, and the base of the annotate
//! and sign layer: reading existing PDFs and appending incremental updates to them.
//!
//! One IR, two drawings (rMazing's pattern): [`render_pdf`] writes the file and
//! [`svg::pages_svg`] the in-app preview, so the preview cannot disagree with the file.
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
pub mod svg;

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
    /// The clock port's time is not RFC 3339.
    #[error("the clock gave {0:?}, which is not an RFC 3339 time")]
    Clock(String),
}

/// The layout as a PDF file.
pub fn render_pdf(
    layout: &Layout,
    fonts: &dyn FontPort,
    images: &dyn ImagePort,
    clock: &dyn ClockPort,
) -> Result<Vec<u8>, PdfError> {
    pdf::render(layout, fonts, images, clock)
}
