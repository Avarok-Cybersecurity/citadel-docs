// Adapted from rMazing crates/cc-export/src/backends/svg.rs
// Copyright (c) Thomas Braun. Relicensed MIT OR Apache-2.0 by the copyright holder, 2026-10-04.

//! The layout, as SVG: the in-app paged preview.
//!
//! The same pages the PDF is written from, one `<svg>` per sheet, in the faces the UI loads
//! with `@font-face` under the family names the layout reports, so the preview is the
//! document and not a picture of something like it. Kerning is switched off, because the PDF
//! places glyphs by advance and the preview must too. Page space is already y-down, so
//! coordinates are written as they are, in points.
//!
//! The document is untrusted: every attribute and text node is escaped, family names are
//! reduced to characters a CSS family name can carry, and only web and mail links are kept.

use crate::meta::allowed_link;
use crate::picture::Picture;
use crate::ports::ImagePort;
use crate::PdfError;
use base64::Engine as _;
use cdoc_layout::ir::to_pt;
use cdoc_layout::{Draw, Layout, Page, Segment};
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

fn family(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_'))
        .collect()
}

fn path_data(segments: &[Segment]) -> String {
    let mut d = String::new();
    for segment in segments {
        let _ = match *segment {
            Segment::Move(x, y) => write!(d, "M{} {} ", to_pt(x), to_pt(y)),
            Segment::Line(x, y) => write!(d, "L{} {} ", to_pt(x), to_pt(y)),
            Segment::Cubic([x1, y1, x2, y2, x3, y3]) => write!(
                d,
                "C{} {} {} {} {} {} ",
                to_pt(x1),
                to_pt(y1),
                to_pt(x2),
                to_pt(y2),
                to_pt(x3),
                to_pt(y3)
            ),
            Segment::Close => write!(d, "Z "),
        };
    }
    d.trim_end().to_owned()
}

fn image(out: &mut String, images: &dyn ImagePort, draw: &Draw) -> Result<(), PdfError> {
    let Draw::Image { rect, content_id } = draw else {
        return Ok(());
    };
    let bytes = images.image(content_id)?;
    // Decoded only to prove the bytes are the declared type before a browser is handed them.
    Picture::decode(bytes.mime, bytes.bytes)
        .map_err(|e| PdfError::Image(content_id.as_str().to_owned(), e))?;
    let data = base64::engine::general_purpose::STANDARD.encode(bytes.bytes);
    let _ = write!(
        out,
        "<image x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" preserveAspectRatio=\"none\" \
         href=\"data:{};base64,{data}\"/>",
        to_pt(rect.x),
        to_pt(rect.y),
        to_pt(rect.width),
        to_pt(rect.height),
        escape(bytes.mime),
    );
    Ok(())
}

/// One sheet as an SVG document: `viewBox` and size in points.
pub fn page_svg(layout: &Layout, page: &Page, images: &dyn ImagePort) -> Result<String, PdfError> {
    let (w, h) = (to_pt(page.width), to_pt(page.height));
    let mut out = String::with_capacity(4096 + page.draws.len() * 128);
    let _ = write!(
        out,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {w} {h}\" width=\"{w}\" \
         height=\"{h}\" style=\"font-kerning:none;background:#fff\">"
    );
    for d in &page.draws {
        match d {
            Draw::Text(run) => {
                let face = &layout.faces[usize::from(run.face.0)];
                let _ = write!(
                    out,
                    "<text x=\"{}\" y=\"{}\" font-family=\"'{}'\" font-weight=\"{}\" \
                     font-style=\"{}\" font-size=\"{}\" fill=\"{}\" xml:space=\"preserve\">{}</text>",
                    to_pt(run.x),
                    to_pt(run.y),
                    family(&face.family),
                    if face.bold { 700 } else { 400 },
                    if face.italic { "italic" } else { "normal" },
                    to_pt(run.size),
                    run.colour.hex(),
                    escape(&run.text)
                );
            }
            Draw::Rule {
                from,
                to,
                width,
                colour,
            } => {
                let _ = write!(
                    out,
                    "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{}\" stroke-width=\"{}\"/>",
                    to_pt(from.0),
                    to_pt(from.1),
                    to_pt(to.0),
                    to_pt(to.1),
                    colour.hex(),
                    to_pt(*width)
                );
            }
            Draw::Fill { rect, colour } => {
                let _ = write!(
                    out,
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\"/>",
                    to_pt(rect.x),
                    to_pt(rect.y),
                    to_pt(rect.width),
                    to_pt(rect.height),
                    colour.hex()
                );
            }
            Draw::Path { segments, colour } => {
                let _ = write!(
                    out,
                    "<path d=\"{}\" fill=\"{}\"/>",
                    path_data(segments),
                    colour.hex()
                );
            }
            Draw::Stroke {
                segments,
                width,
                colour,
            } => {
                let _ = write!(
                    out,
                    "<path d=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\" \
                     stroke-linecap=\"round\" stroke-linejoin=\"round\"/>",
                    path_data(segments),
                    colour.hex(),
                    to_pt(*width)
                );
            }
            Draw::Image { .. } => image(&mut out, images, d)?,
        }
    }
    for link in page.links.iter().filter(|l| allowed_link(&l.href)) {
        let r = link.rect;
        let _ = write!(
            out,
            "<a href=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"transparent\"/></a>",
            escape(&link.href),
            to_pt(r.x),
            to_pt(r.y),
            to_pt(r.width),
            to_pt(r.height)
        );
    }
    out.push_str("</svg>");
    Ok(out)
}

/// Every sheet as an SVG document.
pub fn pages_svg(layout: &Layout, images: &dyn ImagePort) -> Result<Vec<String>, PdfError> {
    layout
        .pages
        .iter()
        .map(|p| page_svg(layout, p, images))
        .collect()
}
