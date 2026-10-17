//! Characters to glyphs: one character map chosen the way every major shaper chooses it, and a
//! direct table for the first 2048 code points, where nearly all Latin, Greek and Cyrillic text
//! lives, so the hot path is an index rather than a binary search.

use crate::error::ShapeError;
use ttf_parser::cmap::Subtable;
use ttf_parser::PlatformId;

const DIRECT: u32 = 0x800;
const WINDOWS_SYMBOL: u16 = 0;
const WINDOWS_BMP: u16 = 1;
const WINDOWS_FULL: u16 = 10;

pub(crate) struct Cmap<'a> {
    subtable: Subtable<'a>,
    symbol: bool,
    direct: Vec<u16>,
}

fn find<'a>(
    face: &ttf_parser::Face<'a>,
    platform: PlatformId,
    encoding: u16,
) -> Option<Subtable<'a>> {
    face.tables()
        .cmap?
        .subtables
        .into_iter()
        .find(|s| s.platform_id == platform && s.encoding_id == encoding)
}

impl<'a> Cmap<'a> {
    /// Symbol first (so a symbol font's private-use mapping wins), then 32-bit Unicode, then
    /// 16-bit Unicode. A face with none of them is refused: Mac Roman-only fonts are not
    /// supported.
    pub(crate) fn new(face: &ttf_parser::Face<'a>) -> Result<Self, ShapeError> {
        let windows = PlatformId::Windows;
        let unicode = PlatformId::Unicode;
        let (subtable, symbol) = find(face, windows, WINDOWS_SYMBOL)
            .map(|s| (s, true))
            .or_else(|| {
                [
                    (windows, WINDOWS_FULL),
                    (unicode, 4),
                    (unicode, 6),
                    (windows, WINDOWS_BMP),
                    (unicode, 3),
                    (unicode, 2),
                    (unicode, 1),
                    (unicode, 0),
                ]
                .into_iter()
                .find_map(|(p, e)| find(face, p, e))
                .map(|s| (s, false))
            })
            .ok_or(ShapeError::NoUnicodeCmap)?;
        let mut cmap = Self {
            subtable,
            symbol,
            direct: Vec::new(),
        };
        cmap.direct = (0..DIRECT).map(|c| cmap.search(c).unwrap_or(0)).collect();
        Ok(cmap)
    }

    fn search(&self, c: u32) -> Option<u16> {
        let found = self
            .subtable
            .glyph_index(c)
            .map(|g| g.0)
            .filter(|&g| g != 0);
        if found.is_none() && self.symbol && c <= 0xFF {
            // Symbol fonts map U+F0xx; Windows reads U+00xx through that range.
            return self.search(0xF000 + c);
        }
        found
    }

    /// The nominal glyph of a code point; `None` when the face has none (glyph 0 is `.notdef`).
    pub(crate) fn glyph(&self, c: u32) -> Option<u16> {
        match usize::try_from(c).ok().and_then(|i| self.direct.get(i)) {
            Some(&0) => None,
            Some(&g) => Some(g),
            None => self.search(c),
        }
    }
}
