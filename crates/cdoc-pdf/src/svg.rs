//! The layout as SVG pages: the in-app paged preview, painted from the display list
//! (`cdoc-paint`) rather than from the IR, so the preview, the canvas editor and the file all
//! draw one description. This module only adds what the PDF backend owns: proving each
//! image's bytes are the type they claim before a browser is handed them.

pub use cdoc_paint::svg::escape;

use crate::picture::Picture;
use crate::ports::ImagePort;
use crate::PdfError;
use cdoc_layout::{FontPort, Layout};
use cdoc_model::ContentId;
use cdoc_paint::{DisplayListView, OpView};

fn check_images(view: &DisplayListView<'_>, images: &dyn ImagePort) -> Result<(), PdfError> {
    for slot in 0..view.page_count() {
        let page = view.page(slot).map_err(cdoc_paint::PaintError::from)?;
        for op in page.ops() {
            if let OpView::Image { content_id, .. } = op {
                let bytes = images.image(&ContentId::new(content_id))?;
                Picture::decode(bytes.mime, bytes.bytes)
                    .map_err(|e| PdfError::Image(content_id.to_owned(), e))?;
            }
        }
    }
    Ok(())
}

/// Every sheet as an SVG document (`viewBox` and size in points), painted from the layout's
/// display list.
pub fn pages_svg(
    layout: &Layout,
    fonts: &dyn FontPort,
    images: &dyn ImagePort,
) -> Result<Vec<String>, PdfError> {
    let bytes = cdoc_paint::encode(layout, fonts)?;
    let view = cdoc_paint::decode(&bytes).map_err(cdoc_paint::PaintError::from)?;
    check_images(&view, images)?;
    Ok(cdoc_paint::svg::render_pages(&view, fonts, images)?)
}
