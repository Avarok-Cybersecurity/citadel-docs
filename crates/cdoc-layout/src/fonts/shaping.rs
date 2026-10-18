//! Text shaped into glyphs: script runs found, each shaped with the run's features (or, for a
//! script the shaper does not implement, set glyph by glyph from the `cmap`), and the glyphs
//! scaled to hundredths of a point so that rounding never accumulates along a word.
//!
//! A face keeps what it has shaped, word by word and per feature set, in font units: words
//! repeat, and a word's glyphs do not depend on the size it is set at.

use super::{scale, Fonts};
use crate::ir::{FaceId, PlacedGlyph, Pt100};
use cdoc_shape::{Direction, Features, Glyph, Language, Script, ShapeParams, ShapedRun};
use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};

/// Shaped words kept per face, beyond which a feature set's words are forgotten and gathered
/// afresh (a long editing session must not grow without bound).
const WORDS_KEPT: usize = 1 << 15;

/// A word's hash for the shaped-word map: a multiply-rotate over eight bytes at a time.
/// Deterministic (no random seed: the core draws no randomness), and fast on the short keys a
/// word map holds; not collision-resistant, which a cache of the caller's own words needs not
/// be.
#[derive(Default)]
struct WordHasher(u64);

impl Hasher for WordHasher {
    fn write(&mut self, bytes: &[u8]) {
        for chunk in bytes.chunks(8) {
            let mut lane = [0; 8];
            lane[..chunk.len()].copy_from_slice(chunk);
            self.0 = (self.0.rotate_left(5) ^ u64::from_le_bytes(lane))
                .wrapping_mul(0x517c_c1b7_2722_0a95);
        }
    }

    fn finish(&self) -> u64 {
        self.0
    }
}

type WordMap = HashMap<Box<str>, Box<[Glyph]>, BuildHasherDefault<WordHasher>>;

/// One face's shaped words, per feature set, in font units.
pub struct Words {
    by_features: Vec<(Features, WordMap)>,
    scratch: ShapedRun,
}

impl Words {
    /// Nothing shaped yet.
    pub fn new() -> Self {
        Self {
            by_features: Vec::new(),
            scratch: ShapedRun::empty(),
        }
    }
}

/// `text` as the face's nominal glyphs and `hmtx` advances, clusters offset by `base`: what a
/// script the shaper refuses is set as.
fn nominal(face: &cdoc_shape::Face<'_>, text: &str, base: usize, out: &mut Vec<Glyph>) {
    for (at, ch) in text.char_indices() {
        let id = face.glyph(ch).unwrap_or(0);
        out.push(Glyph {
            id,
            cluster: u32::try_from(base + at).unwrap_or(u32::MAX),
            x_advance: face.advance(id),
            y_advance: 0,
            x_offset: 0,
            y_offset: 0,
        });
    }
}

fn shape_word(
    shaper: &mut cdoc_shape::Shaper<'_, '_>,
    scratch: &mut ShapedRun,
    text: &str,
    features: Features,
) -> Box<[Glyph]> {
    let mut glyphs = Vec::with_capacity(text.len());
    for run in cdoc_shape::itemise(text) {
        let slice = &text[run.range.clone()];
        let params = ShapeParams {
            script: run.script,
            language: Language::default_system(),
            direction: Direction::LeftToRight,
            features,
        };
        let set_by_shaper =
            run.script != Script::Unsupported && shaper.shape_into(slice, &params, scratch).is_ok();
        if set_by_shaper {
            let base = u32::try_from(run.range.start).unwrap_or(u32::MAX);
            glyphs.extend(scratch.glyphs.iter().map(|g| Glyph {
                cluster: g.cluster.saturating_add(base),
                ..*g
            }));
        } else {
            nominal(shaper.face(), slice, run.range.start, &mut glyphs);
        }
    }
    glyphs.into_boxed_slice()
}

impl Fonts<'_> {
    /// Shapes `text` in `face` with `features` and sets it at `size`, appending the glyphs to
    /// `out` with their clusters offset by `base`; returns the width, which is exactly the sum
    /// of the appended advances.
    pub fn shape(
        &mut self,
        face: FaceId,
        size: Pt100,
        text: &str,
        features: Features,
        base: u32,
        out: &mut Vec<PlacedGlyph>,
    ) -> Pt100 {
        let parsed = &mut self.parsed[usize::from(face.0)];
        let per_em = parsed.face.units_per_em();
        let words = &mut parsed.words;
        let slot = if let Some(i) = words.by_features.iter().position(|(f, _)| *f == features) {
            i
        } else {
            words.by_features.push((features, WordMap::default()));
            words.by_features.len() - 1
        };
        let map = &mut words.by_features[slot].1;
        if !map.contains_key(text) {
            if map.len() >= WORDS_KEPT {
                map.clear();
            }
            let glyphs = shape_word(&mut parsed.shaper, &mut words.scratch, text, features);
            map.insert(text.into(), glyphs);
        }
        let Some(glyphs) = map.get(text) else {
            return 0;
        };
        let (mut units, mut pen) = (0_i64, 0);
        out.reserve(glyphs.len());
        for g in glyphs {
            let next = units + i64::from(g.x_advance);
            let end = scale(next, size, per_em);
            let x_offset = if g.x_offset == 0 {
                0
            } else {
                scale(units + i64::from(g.x_offset), size, per_em) - pen
            };
            let y_offset = if g.y_offset == 0 {
                0
            } else {
                -scale(i64::from(g.y_offset), size, per_em)
            };
            out.push(PlacedGlyph {
                id: g.id,
                advance: end - pen,
                x_offset,
                y_offset,
                cluster: g.cluster.saturating_add(base),
            });
            units = next;
            pen = end;
        }
        pen
    }

    /// The width of `text` shaped in `face` at `size` with `features`.
    pub fn width(&mut self, face: FaceId, size: Pt100, text: &str, features: Features) -> Pt100 {
        let mut glyphs = Vec::new();
        self.shape(face, size, text, features, 0, &mut glyphs)
    }
}
