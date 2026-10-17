//! The glyph definition table: each glyph's class (base, ligature, mark, component) and mark
//! attachment class, read once into a flat array; mark glyph sets; and ligature carets, which
//! `ttf-parser` skips and this module reads from the raw table.

use super::be::{coverage_index, i16_at, u16_at};
use ttf_parser::gdef::GlyphClass;
use ttf_parser::GlyphId;

/// Glyph property bits, numbered as the lookup flags that ignore them.
pub(crate) const BASE: u16 = 0x02;
pub(crate) const LIGATURE: u16 = 0x04;
pub(crate) const MARK: u16 = 0x08;
/// Set by the shaper, never by GDEF.
pub(crate) const SUBSTITUTED: u16 = 0x10;
pub(crate) const LIGATED: u16 = 0x20;
pub(crate) const MULTIPLIED: u16 = 0x40;
pub(crate) const PRESERVE: u16 = SUBSTITUTED | LIGATED | MULTIPLIED;

pub(crate) struct Gdef<'a> {
    table: Option<ttf_parser::gdef::Table<'a>>,
    raw: Option<&'a [u8]>,
    props: Vec<u16>,
    has_classes: bool,
}

impl<'a> Gdef<'a> {
    pub(crate) fn new(face: &ttf_parser::Face<'a>) -> Self {
        let table = face.tables().gdef;
        let has_classes = table.is_some_and(|t| t.has_glyph_classes());
        let props = match table {
            Some(t) if has_classes => (0..face.number_of_glyphs())
                .map(|g| {
                    let glyph = GlyphId(g);
                    match t.glyph_class(glyph) {
                        Some(GlyphClass::Base) => BASE,
                        Some(GlyphClass::Ligature) => LIGATURE,
                        Some(GlyphClass::Mark) => {
                            (t.glyph_mark_attachment_class(glyph) << 8) | MARK
                        }
                        _ => 0,
                    }
                })
                .collect(),
            _ => Vec::new(),
        };
        let raw = face.raw_face().table(ttf_parser::Tag::from_bytes(b"GDEF"));
        Self {
            table,
            raw,
            props,
            has_classes,
        }
    }

    /// Whether the font classifies its glyphs; when it does not, the shaper guesses from
    /// Unicode.
    pub(crate) fn has_classes(&self) -> bool {
        self.has_classes
    }

    /// The class bits and mark attachment class of a glyph (0 when unclassified).
    pub(crate) fn props(&self, glyph: u16) -> u16 {
        self.props.get(usize::from(glyph)).copied().unwrap_or(0)
    }

    pub(crate) fn is_mark_in_set(&self, glyph: u16, set: u16) -> bool {
        self.table
            .is_some_and(|t| t.is_mark_glyph(GlyphId(glyph), Some(set)))
    }

    /// The caret positions (font units from the glyph origin) the font gives a ligature, when
    /// it gives them as coordinates.
    pub(crate) fn carets(&self, glyph: u16) -> Option<Vec<i32>> {
        let raw = self.raw?;
        let list = raw.get(Some(usize::from(u16_at(raw, 8)?)).filter(|&o| o != 0)?..)?;
        let index = usize::from(coverage_index(
            list.get(usize::from(u16_at(list, 0)?)..)?,
            glyph,
        )?);
        let lig = list.get(usize::from(u16_at(list, 4 + index * 2)?)..)?;
        let count = usize::from(u16_at(lig, 0)?);
        (0..count)
            .map(|i| {
                let value = lig.get(usize::from(u16_at(lig, 2 + i * 2)?)..)?;
                match u16_at(value, 0)? {
                    1 | 3 => i16_at(value, 2).map(i32::from),
                    _ => None,
                }
            })
            .collect()
    }
}
