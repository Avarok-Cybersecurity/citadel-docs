// Adapted from rMazing crates/cc-export/src/backends/svg.rs
// Copyright (c) Thomas Braun. Relicensed MIT OR Apache-2.0 by the copyright holder, 2026-10-04.

//! The SVG painter: a decoded display-list page as one `<svg>` document, the in-app paged
//! preview.
//!
//! Text is written as `<text>` in the faces the UI loads with `@font-face` under the family
//! names the face table holds, so the preview is selectable and is the document rather than a
//! picture of something like it. Every character carries its own position (from the run's
//! shaped glyphs), and the browser's kerning is switched off so it does not move them again.
//! Page space is already y-down, so coordinates are written as they are, in points.
//!
//! The document is untrusted: every attribute and text node is escaped, family names are
//! reduced to characters a CSS family name can carry, and only [`allowed_link`] targets
//! become links. Image bytes are written as given; a caller that must prove them first (the
//! `cdoc-pdf` forwarder does) checks them before painting.

mod shapes;
mod text;

use shapes::{attr_paint, rect_attrs};

use crate::{allowed_link, DisplayListView, OpView, PaintError};
use base64::Engine as _;
use cdoc_layout::ir::to_pt;
use cdoc_model::ports::ImagePort;
use cdoc_model::ContentId;
use std::fmt::Write as _;

/// The characters XML text and attribute values cannot carry raw.
#[must_use]
pub fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Paints pages of one display list, reusing what it learns about faces across pages.
pub struct SvgPainter<'a> {
    view: &'a DisplayListView<'a>,
    images: &'a dyn ImagePort,
}

impl<'a> SvgPainter<'a> {
    /// A painter over `view`, with the images it shows from `images`.
    #[must_use]
    pub const fn new(view: &'a DisplayListView<'a>, images: &'a dyn ImagePort) -> Self {
        Self { view, images }
    }

    /// The page in `slot` as an SVG document: `viewBox` and size in points.
    pub fn page(&mut self, slot: usize) -> Result<String, PaintError> {
        let page = self.view.page(slot)?;
        let (w, h) = (to_pt(page.width), to_pt(page.height));
        let mut out = String::with_capacity(4096 + page.body_len() * 8);
        let _ = write!(
            out,
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {w} {h}\" width=\"{w}\" \
             height=\"{h}\" style=\"font-kerning:none;background:#fff\">"
        );
        let mut clips = 0_usize;
        for op in page.ops() {
            self.op(&mut out, op, page.number, &mut clips)?;
        }
        out.push_str("</svg>");
        Ok(out)
    }

    fn op(
        &mut self,
        out: &mut String,
        op: OpView<'_>,
        number: u32,
        clips: &mut usize,
    ) -> Result<(), PaintError> {
        if shapes::shape(out, &op) {
            return Ok(());
        }
        let faces = self.view.faces();
        match op {
            OpView::GlyphRun { head, text, glyphs } => {
                text::shaped(out, &head, &faces[usize::from(head.face.0)], text, glyphs);
            }
            OpView::Image { rect, content_id } => {
                let bytes = self.images.image(&ContentId::new(content_id))?;
                if cdoc_model::image_probe::ImageKind::from_mime(bytes.mime).is_none() {
                    let _ = write!(
                        out,
                        "<rect {} fill=\"#eeeeee\" stroke=\"#999999\" stroke-width=\"0.5\"/>",
                        rect_attrs(rect)
                    );
                    return Ok(());
                }
                let data = base64::engine::general_purpose::STANDARD.encode(bytes.bytes);
                let _ = write!(
                    out,
                    "<image {} preserveAspectRatio=\"none\" href=\"data:{};base64,{data}\"/>",
                    rect_attrs(rect),
                    escape(bytes.mime),
                );
            }
            OpView::Link { rect, href } => {
                if allowed_link(href) {
                    let _ = write!(
                        out,
                        "<a href=\"{}\"><rect {} fill=\"transparent\"/></a>",
                        escape(href),
                        rect_attrs(rect)
                    );
                }
            }
            OpView::ClipPush(rect) => {
                *clips += 1;
                let id = format!("p{number}c{clips}");
                let _ = write!(
                    out,
                    "<clipPath id=\"{id}\"><rect {}/></clipPath><g clip-path=\"url(#{id})\">",
                    rect_attrs(rect)
                );
            }
            OpView::ClipPop => out.push_str("</g>"),
            _ => {}
        }
        Ok(())
    }
}

/// The page in `slot` of `view` as an SVG document.
pub fn render_page(
    view: &DisplayListView<'_>,
    slot: usize,
    images: &dyn ImagePort,
) -> Result<String, PaintError> {
    SvgPainter::new(view, images).page(slot)
}

/// Every page of `view` as an SVG document, in slot order.
pub fn render_pages(
    view: &DisplayListView<'_>,
    images: &dyn ImagePort,
) -> Result<Vec<String>, PaintError> {
    let mut painter = SvgPainter::new(view, images);
    (0..view.page_count())
        .map(|slot| painter.page(slot))
        .collect()
}
