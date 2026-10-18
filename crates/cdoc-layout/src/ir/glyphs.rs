//! Shaped text as the layout sets it: glyph ids with their advances and offsets, and the
//! characters each glyph stands for.

use super::{FaceId, Pt100, Units};
use cdoc_model::Color;

/// One glyph as set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PlacedGlyph {
    /// The glyph id in the face.
    pub id: u16,
    /// How far the pen moves after it.
    pub advance: Pt100,
    /// Horizontal displacement of the drawing from the pen, rightwards; the pen does not move.
    pub x_offset: Pt100,
    /// Vertical displacement of the drawing from the pen, **downwards** (page space); the pen
    /// does not move.
    pub y_offset: Pt100,
    /// Byte offset in the run's text of the first character this glyph stands for. Glyphs of
    /// one cluster share it (a ligature is one glyph for several characters, a decomposed
    /// accent several glyphs for one); clusters never decrease along a run.
    pub cluster: u32,
}

/// Shaped text set at one point: what every backend draws. Glyph ids come from the shaper (so
/// ligatures, kerning and mark positions are already applied); the text is kept beside them
/// so the preview stays selectable and the PDF's text can be copied and searched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlyphRun {
    /// The characters the glyphs stand for: what extraction and selection read.
    pub text: String,
    /// The glyphs, left to right; the pen starts at `x`.
    pub glyphs: Vec<PlacedGlyph>,
    /// The baseline's left end.
    pub x: Pt100,
    /// The baseline.
    pub y: Pt100,
    /// The font size.
    pub size: Pt100,
    /// The face.
    pub face: FaceId,
    /// The fill colour.
    pub colour: Color,
    /// The paragraph unit each character draws, when the run is document text: what an
    /// editor turns into caret stops (see [`super::CaretLine`]).
    pub units: Units,
}

impl GlyphRun {
    /// The sum of the advances: where the pen ends, from `x`.
    #[must_use]
    pub fn width(&self) -> Pt100 {
        self.glyphs.iter().map(|g| g.advance).sum()
    }

    /// The text of glyph `index`'s cluster (empty for every glyph of a cluster but its
    /// first).
    #[must_use]
    pub fn cluster_text(&self, index: usize) -> &str {
        cluster_text(
            &self.text,
            self.glyphs.len(),
            |i| self.glyphs[i].cluster,
            index,
        )
    }
}

/// The text of glyph `index`'s cluster in a run of `count` glyphs over `text`, where
/// `cluster(i)` is glyph `i`'s cluster: from its byte offset to the next larger cluster (or
/// the end of the text), and empty for every glyph of a cluster but its first. The one rule
/// every reader of shaped text (the IR, the display list) shares.
pub fn cluster_text(
    text: &str,
    count: usize,
    cluster: impl Fn(usize) -> u32,
    index: usize,
) -> &str {
    if index >= count || (index > 0 && cluster(index - 1) == cluster(index)) {
        return "";
    }
    let start = cluster(index) as usize;
    let end = (index + 1..count)
        .map(|i| cluster(i) as usize)
        .find(|c| *c > start)
        .unwrap_or(text.len());
    text.get(start..end).unwrap_or("")
}
