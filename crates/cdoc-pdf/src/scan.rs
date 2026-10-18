//! The one pass over the display list before anything is written: the glyphs and text each
//! face draws (for subsetting and `ToUnicode`), the images drawn (each once, in order of first
//! use), and the file identifier, which is derived from the runs' text and positions.

use crate::font::Used;
use cdoc_paint::{Op, PageOps};
use std::collections::{BTreeMap, BTreeSet};

/// What the pass found.
pub struct Scan<'a> {
    /// Each face's glyphs and their text, by face id.
    pub faces: BTreeMap<u16, Used>,
    /// Content ids, each once, in order of first use.
    pub images: Vec<&'a str>,
    /// The file identifier pair (original, revision).
    pub file_id: (Vec<u8>, Vec<u8>),
}

/// FNV-1a over the runs, salted: the same document gives the same id (no randomness), which is
/// what an incremental update later keeps.
struct Fnv(u64);

impl Fnv {
    fn new(salt: &[u8]) -> Self {
        let mut h = Self(0xCBF2_9CE4_8422_2325);
        h.eat(salt);
        h
    }

    fn eat(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.0 = (self.0 ^ u64::from(*b)).wrapping_mul(0x0000_0100_0000_01B3);
        }
    }

    fn id(mut self) -> Vec<u8> {
        let first = self.0;
        self.eat(b"second half");
        [first.to_be_bytes(), self.0.to_be_bytes()].concat()
    }
}

/// Reads every page once.
pub fn scan(pages: &[PageOps]) -> Scan<'_> {
    let mut faces: BTreeMap<u16, Used> = BTreeMap::new();
    let mut images = Vec::new();
    let mut seen = BTreeSet::new();
    let mut ids = [Fnv::new(b"cdoc-original"), Fnv::new(b"cdoc-revision")];
    for op in pages.iter().flat_map(|p| &p.ops) {
        match op {
            Op::GlyphRun { head, text, glyphs } => {
                faces.entry(head.face.0).or_default().add(text, glyphs);
                for id in &mut ids {
                    id.eat(text.as_bytes());
                    id.eat(&head.x.to_be_bytes());
                    id.eat(&head.y.to_be_bytes());
                }
            }
            Op::Image { content_id, .. } if seen.insert(content_id.as_str()) => {
                images.push(content_id.as_str());
            }
            _ => {}
        }
    }
    let [original, revision] = ids;
    Scan {
        faces,
        images,
        file_id: (original.id(), revision.id()),
    }
}
