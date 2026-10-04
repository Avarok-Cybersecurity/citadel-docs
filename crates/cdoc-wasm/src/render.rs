//! Rendering through the facade: a ProseMirror document laid out and drawn as a PDF or as SVG
//! pages, with the faces, images and time the caller supplies. String- and byte-level, so it
//! is tested natively; `lib.rs` only adds the wasm-bindgen signatures.

use cdoc_layout::{layout, FontLibrary, Layout, LayoutOptions};
use cdoc_model::ContentId;
use cdoc_pdf::{render_pdf, svg::pages_svg, FixedClock, ImageLibrary};
use cdoc_pm::from_pm_json;

/// What a render needs from the world: faces, images and the time. Every part is explicit; a
/// render with no faces fails rather than drawing in something the caller did not choose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assets {
    fonts: FontLibrary,
    images: ImageLibrary,
    clock: FixedClock,
}

impl Assets {
    /// No faces or images yet; `fallback_family` serves any family not added or aliased, and
    /// `now_rfc3339` is the time the file records.
    #[must_use]
    pub fn new(fallback_family: &str, now_rfc3339: &str) -> Self {
        Self {
            fonts: FontLibrary::new(fallback_family),
            images: ImageLibrary::empty(),
            clock: FixedClock(now_rfc3339.to_owned()),
        }
    }

    /// Adds a face (a TrueType or OpenType file).
    pub fn add_font(&mut self, family: &str, bold: bool, italic: bool, bytes: &[u8]) {
        self.fonts.add(family, bold, italic, bytes.to_vec());
    }

    /// Serves `target` whenever a document asks for `name`.
    pub fn alias_font(&mut self, name: &str, target: &str) {
        self.fonts.alias(name, target);
    }

    /// Adds an image by the content id documents refer to it by.
    pub fn add_image(&mut self, content_id: &str, mime: &str, bytes: &[u8]) {
        self.images
            .add(ContentId::new(content_id), mime, bytes.to_vec());
    }
}

fn laid_out(pm_json: &str, assets: &Assets) -> Result<Layout, String> {
    let doc = from_pm_json(pm_json).map_err(|e| e.to_string())?;
    if let Err(errors) = doc.validate() {
        let messages: Vec<String> = errors.iter().map(ToString::to_string).collect();
        return Err(format!("invalid document: {}", messages.join("; ")));
    }
    layout(&doc, &assets.fonts, &LayoutOptions::word_like()).map_err(|e| e.to_string())
}

/// ProseMirror JSON → PDF bytes.
pub fn pm_to_pdf(pm_json: &str, assets: &Assets) -> Result<Vec<u8>, String> {
    let laid = laid_out(pm_json, assets)?;
    render_pdf(&laid, &assets.fonts, &assets.images, &assets.clock).map_err(|e| e.to_string())
}

/// ProseMirror JSON → a JSON array of SVG documents, one per page, for the paged preview.
pub fn pm_to_svg_pages(pm_json: &str, assets: &Assets) -> Result<String, String> {
    let laid = laid_out(pm_json, assets)?;
    let pages = pages_svg(&laid, &assets.images).map_err(|e| e.to_string())?;
    serde_json::to_string(&pages).map_err(|e| e.to_string())
}
