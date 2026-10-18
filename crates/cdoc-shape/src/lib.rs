//! Citadel Docs shaping: text to positioned glyphs, for Latin, Greek and Cyrillic.
//!
//! Our own OpenType shaper, pure Rust over `ttf-parser`'s table readers: no I/O, no clock, no
//! randomness, no global state. It follows the OpenType shaping model HarfBuzz established
//! (Unicode normalisation against the font's coverage, grapheme clusters, GSUB then GPOS lookups
//! in lookup order per stage, mark attachment propagated at the end) so its output matches
//! HarfBuzz glyph for glyph for the lookups it implements; `tests/oracle.rs` holds it to that.
//!
//! - [`Face`] parses a font once.
//! - [`itemise`] splits text into script runs.
//! - [`Shaper`] shapes a run with explicit [`ShapeParams`] into a [`ShapedRun`] of glyphs in font
//!   units, caching one plan per parameter set and reusing its buffers between calls.
//! - [`ShapedRun`] scales to hundredths of a point and maps characters to glyphs and back
//!   (hit testing, ligature carets).
//!
//! Supported: GSUB lookup types 1, 2, 3 (first alternate), 4, 5, 6, 7 and 8; GPOS types 1, 2, 4,
//! 5, 6, 7, 8 and 9; the legacy `kern` table when GPOS has no kerning. Left to right only; a
//! right-to-left request is refused, not guessed at.

#![forbid(unsafe_code)]

mod apply;
mod buffer;
mod error;
mod face;
mod itemise;
mod normalize;
mod params;
mod plan;
mod position;
mod run;
mod shaper;
mod substitute;
mod unicode;

pub use error::ShapeError;
pub use face::{Face, GlyphBounds, VerticalMetrics};
pub use itemise::{itemise, ScriptRun};
pub use params::{
    Direction, Feature, Features, Language, Script, ShapeParams, HARFBUZZ_DEFAULTS_NOT_IMPLEMENTED,
};
pub use run::{scale, scale_size, ClusterSpan, Glyph, Pt100, ShapedRun};
pub use shaper::Shaper;
pub use unicode::UNICODE_VERSION;
