//! Rendering through the facade: a ProseMirror document laid out and drawn as a PDF, as SVG
//! pages or as a display list (with the glyph outlines and metrics a canvas painter needs),
//! with the faces, images and time the caller supplies. String- and byte-level, so it is
//! tested natively; `render_bindings.rs` only adds the wasm-bindgen signatures.

#[cfg(feature = "paint")]
use cdoc_layout::FontPort as _;
use cdoc_layout::{layout, BreakerChoice, FontLibrary, Layout, LayoutOptions, Params};
use cdoc_model::{ContentId, ImageLibrary};
#[cfg(feature = "pdf")]
use cdoc_pdf::{render_pdf, svg::pages_svg, FixedClock};
use cdoc_pm::from_pm_json;

/// What a render needs from the world: faces, images and the time. Every part is explicit; a
/// render with no faces fails rather than drawing in something the caller did not choose.
/// Images and the time are read by the PDF and SVG renders only; an artefact without them still
/// takes both, so `RenderAssets` has one shape in every artefact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assets {
    fonts: FontLibrary,
    #[cfg_attr(not(feature = "pdf"), allow(dead_code))]
    images: ImageLibrary,
    #[cfg_attr(not(feature = "pdf"), allow(dead_code))]
    now_rfc3339: String,
}

impl Assets {
    /// No faces or images yet; `fallback_family` serves any family not added or aliased, and
    /// `now_rfc3339` is the time the file records.
    #[must_use]
    pub fn new(fallback_family: &str, now_rfc3339: &str) -> Self {
        Self {
            fonts: FontLibrary::new(fallback_family),
            images: ImageLibrary::empty(),
            now_rfc3339: now_rfc3339.to_owned(),
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

    /// Compiles the hyphenation pattern file the manifest tags `tag`.
    pub fn add_patterns(&mut self, tag: &str, text: &str) -> Result<(), String> {
        crate::hyphenation::register(&mut self.fonts, tag, text)
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
    layout(&doc, &assets.fonts, &facade_options()).map_err(|e| e.to_string())
}

/// What every facade entry point lays out with: Word's page conventions and Knuth-Plass
/// line breaking with Word-like parameters.
#[must_use]
pub fn facade_options() -> LayoutOptions {
    LayoutOptions::word_like(BreakerChoice::KnuthPlass(Params::word_like()))
}

#[cfg(feature = "pdf")]
/// ProseMirror JSON → PDF bytes.
pub fn pm_to_pdf(pm_json: &str, assets: &Assets) -> Result<Vec<u8>, String> {
    let laid = laid_out(pm_json, assets)?;
    let clock = FixedClock(assets.now_rfc3339.clone());
    render_pdf(&laid, &assets.fonts, &assets.images, &clock).map_err(|e| e.to_string())
}

#[cfg(feature = "pdf")]
/// ProseMirror JSON → a JSON array of SVG documents, one per page, for the paged preview.
pub fn pm_to_svg_pages(pm_json: &str, assets: &Assets) -> Result<String, String> {
    let laid = laid_out(pm_json, assets)?;
    let pages = pages_svg(&laid, &assets.fonts, &assets.images).map_err(|e| e.to_string())?;
    serde_json::to_string(&pages).map_err(|e| e.to_string())
}

#[cfg(feature = "paint")]
/// ProseMirror JSON → the display list of every page.
pub fn pm_to_display_list(pm_json: &str, assets: &Assets) -> Result<Vec<u8>, String> {
    let laid = laid_out(pm_json, assets)?;
    cdoc_paint::encode(&laid, &assets.fonts).map_err(|e| e.to_string())
}

#[cfg(feature = "paint")]
/// ProseMirror JSON → a display list of only `pages` (zero-based, in the order given).
pub fn pm_to_display_list_pages(
    pm_json: &str,
    assets: &Assets,
    pages: &[u32],
) -> Result<Vec<u8>, String> {
    let laid = laid_out(pm_json, assets)?;
    let pages: Vec<usize> = pages
        .iter()
        .map(|p| usize::try_from(*p).map_err(|e| e.to_string()))
        .collect::<Result<_, _>>()?;
    cdoc_paint::encode_pages(&laid, &assets.fonts, &pages).map_err(|e| e.to_string())
}

#[cfg(feature = "paint")]
fn face_bytes<'a>(
    assets: &'a Assets,
    family: &str,
    bold: bool,
    italic: bool,
) -> Result<&'a [u8], String> {
    Ok(assets
        .fonts
        .face(family, bold, italic)
        .map_err(|e| e.to_string())?
        .bytes)
}

#[cfg(feature = "paint")]
/// The glyph-path record of `glyph_id` in the face the font port serves for the request.
pub fn glyph_path(
    assets: &Assets,
    family: &str,
    bold: bool,
    italic: bool,
    glyph_id: u16,
) -> Result<Vec<u8>, String> {
    let bytes = face_bytes(assets, family, bold, italic)?;
    cdoc_paint::glyph::glyph_path(bytes, glyph_id).map_err(|e| e.to_string())
}

#[cfg(feature = "paint")]
/// The 8-byte metrics record of the face the font port serves for the request.
pub fn face_metrics(
    assets: &Assets,
    family: &str,
    bold: bool,
    italic: bool,
) -> Result<Vec<u8>, String> {
    let bytes = face_bytes(assets, family, bold, italic)?;
    cdoc_paint::glyph::face_metrics(bytes)
        .map(|m| m.to_bytes().to_vec())
        .map_err(|e| e.to_string())
}
