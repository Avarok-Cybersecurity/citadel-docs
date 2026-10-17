//! The glyph set every lookup test shares, and shaping that checks rustybuzz agrees.

use super::TestFont;
use crate::common::{ours, params, rustybuzz};
use cdoc_shape::{Face, Features, Glyph, Script};

pub const A: u16 = 1;
pub const B: u16 = 2;
pub const C: u16 = 3;
pub const F: u16 = 4;
pub const I: u16 = 5;
pub const FI: u16 = 6;
pub const ACUTE: u16 = 7;
pub const V: u16 = 8;
pub const CAP_A: u16 = 9;
pub const ALT: u16 = 10;
pub const GRAVE: u16 = 11;

/// Advances by glyph id: `.notdef`, a, b, c, f, i, fi, acute, V, A, alt, grave.
pub const ADVANCES: [u16; 12] = [500, 600, 610, 620, 300, 250, 550, 200, 640, 650, 660, 200];

/// GDEF classes for the set: bases, the ligature, the two marks.
pub const CLASSES: [(u16, u16, u16); 5] =
    [(1, 5, 1), (6, 6, 2), (7, 7, 3), (8, 10, 1), (11, 11, 3)];

/// A font of the shared glyph set plus `tables`.
pub fn font(tables: Vec<([u8; 4], Vec<u8>)>) -> Vec<u8> {
    TestFont {
        advances: ADVANCES.to_vec(),
        cmap: vec![
            ('a', A),
            ('b', B),
            ('c', C),
            ('f', F),
            ('i', I),
            ('\u{301}', ACUTE),
            ('V', V),
            ('A', CAP_A),
            ('\u{300}', GRAVE),
        ],
        tables,
    }
    .build()
}

/// `text` shaped in `bytes` with `features`, after asserting rustybuzz shapes it identically.
pub fn shape(bytes: &[u8], text: &str, features: Features) -> Vec<Glyph> {
    let face = Face::parse(bytes, 0).expect("test font parses");
    let theirs = rustybuzz::Face::from_slice(bytes, 0).expect("rustybuzz parses the test font");
    let p = params(Script::Latin, features);
    let got = ours(&face, text, p);
    assert_eq!(
        got,
        rustybuzz(&theirs, text, p),
        "rustybuzz disagrees on {text:?}"
    );
    got
}

/// `(glyph, cluster, x advance)` of each glyph.
pub fn simple(glyphs: &[Glyph]) -> Vec<(u16, u32, i32)> {
    glyphs
        .iter()
        .map(|g| (g.id, g.cluster, g.x_advance))
        .collect()
}

/// `(glyph, x advance, x offset, y offset)` of each glyph.
pub fn placed(glyphs: &[Glyph]) -> Vec<(u16, i32, i32, i32)> {
    glyphs
        .iter()
        .map(|g| (g.id, g.x_advance, g.x_offset, g.y_offset))
        .collect()
}
