// Adapted from rMazing crates/cc-export/src/layout/fonts.rs (subsetting) and
// crates/cc-export/src/backends/pdf.rs (embedding)
// Copyright (c) Thomas Braun. Relicensed MIT OR Apache-2.0 by the copyright holder, 2026-10-04.

//! Faces embedded as subsets: every face a display-list run is set in, cut down to the glyphs the document
//! draws, as a Type 0 / `CIDFontType2` font with `Identity-H` encoding. A CID is the glyph id
//! the shaper chose (a `CIDToGIDMap` stream sends it to the subset's renumbered glyph), so the
//! content stream draws exactly the layout's glyphs, ligatures and all, without consulting a
//! `cmap`. The `ToUnicode` map is built from the runs' clusters, so text copied out of the
//! file is the text that was laid out.
//!
//! Strings reach the content stream as two-byte glyph ids, so a run reading `:)` cannot close
//! the string it is written into: the parenthesis is a number.

mod used;

pub use used::Used;

use cdoc_layout::{FaceKey, FontPort};
use cdoc_paint::{FaceEntry, Glyph};
use pdf_writer::types::{CidFontType, FontFlags, SystemInfo, UnicodeCmap};
use pdf_writer::{Filter, Finish, Name, Pdf, Rect, Ref, Str};
use std::collections::BTreeMap;
use subsetter::GlyphRemapper;

/// A font that could not be embedded.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EmbedError {
    /// The port no longer serves a face the layout used.
    #[error(transparent)]
    Font(#[from] cdoc_layout::FontError),
    /// The face did not parse, or the subsetter refused it.
    #[error("the face {0:?} could not be embedded: {1}")]
    Subset(FaceKey, String),
}

/// One embedded face: where its font dictionary is, the width of each CID, and the text each
/// CID extracts as.
pub struct Embedded {
    /// The Type 0 font dictionary.
    pub reference: Ref,
    /// Each CID's width in thousandths of an em, as the `W` array gives it.
    widths: BTreeMap<u16, f32>,
    unicode: BTreeMap<u16, String>,
}

impl Embedded {
    /// The width the PDF reader advances after drawing glyph `id`, in thousandths of an em.
    #[must_use]
    pub fn width(&self, id: u16) -> f32 {
        self.widths.get(&id).copied().unwrap_or(0.0)
    }

    /// Whether the `ToUnicode` map alone reads a run of `glyphs` back as its `text`; when it
    /// does not (a glyph drawn for different text elsewhere, a cluster of several glyphs), the
    /// run needs an `/ActualText` span.
    #[must_use]
    pub fn extracts(&self, text: &str, glyphs: &[Glyph]) -> bool {
        used::extracted(&self.unicode, glyphs) == text
    }
}

/// A subset tag from the glyph set: six capitals, deterministic (no randomness), as PDF's
/// subset naming convention asks.
fn tag(key: &FaceKey, glyphs: &[u16]) -> String {
    let mut h: u32 = 0x811C_9DC5;
    for b in key
        .family
        .bytes()
        .chain(glyphs.iter().flat_map(|g| g.to_be_bytes()))
    {
        h = (h ^ u32::from(b)).wrapping_mul(0x0100_0193);
    }
    (0..6)
        .map(|i| char::from(b'A' + u8::try_from((h >> (i * 5)) % 26).unwrap_or(0)))
        .collect()
}

fn base_name(face: &ttf_parser::Face<'_>, key: &FaceKey) -> String {
    let postscript = face
        .names()
        .into_iter()
        .find(|n| n.name_id == ttf_parser::name_id::POST_SCRIPT_NAME && n.is_unicode())
        .and_then(|n| n.to_string());
    let fallback = || {
        let style = match (key.bold, key.italic) {
            (false, false) => "Regular",
            (true, false) => "Bold",
            (false, true) => "Italic",
            (true, true) => "BoldItalic",
        };
        format!("{}-{style}", key.family)
    };
    postscript
        .unwrap_or_else(fallback)
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .chain(std::iter::empty())
        .collect()
}

fn units(value: i16, per_em: f32) -> f32 {
    f32::from(value) * 1000.0 / per_em
}

/// Embeds every face in `used` (by face id into `faces`, the list's face table); `alloc`
/// hands out object numbers.
pub fn embed(
    pdf: &mut Pdf,
    alloc: &mut Ref,
    faces: &[FaceEntry],
    used: BTreeMap<u16, Used>,
    fonts: &dyn FontPort,
) -> Result<BTreeMap<u16, Embedded>, EmbedError> {
    let mut out = BTreeMap::new();
    for (face_id, used) in used {
        // Lowering and the decoder both check every run's face id against the table.
        let key = &faces[usize::from(face_id)].key;
        let source = fonts.face(&key.family, key.bold, key.italic)?;
        let failed = |why: &dyn std::fmt::Display| EmbedError::Subset(key.clone(), why.to_string());
        let face = ttf_parser::Face::parse(source.bytes, 0).map_err(|e| failed(&e))?;
        let mut remapper = GlyphRemapper::new();
        let map: Vec<(u16, u16)> = used
            .glyphs
            .iter()
            .map(|old| (*old, remapper.remap(*old)))
            .collect();
        let subset =
            subsetter::subset(source.bytes, 0, &remapper).map_err(|e| failed(&format!("{e:?}")))?;
        let per_em = f32::from(face.units_per_em());
        let widths: BTreeMap<u16, f32> = used
            .glyphs
            .iter()
            .map(|g| {
                let advance = face.glyph_hor_advance(ttf_parser::GlyphId(*g)).unwrap_or(0);
                (*g, f32::from(advance) * 1000.0 / per_em)
            })
            .collect();
        let old_gids: Vec<u16> = used.glyphs.iter().copied().collect();
        let name = format!("{}+{}", tag(key, &old_gids), base_name(&face, key));
        let [type0, cid, descriptor, file, cmap, gids] = std::array::from_fn(|_| alloc.bump());
        let system = SystemInfo {
            registry: Str(b"Adobe"),
            ordering: Str(b"Identity"),
            supplement: 0,
        };
        pdf.type0_font(type0)
            .base_font(Name(name.as_bytes()))
            .encoding_predefined(Name(b"Identity-H"))
            .descendant_font(cid)
            .to_unicode(cmap);
        let mut cid_font = pdf.cid_font(cid);
        cid_font
            .subtype(CidFontType::Type2)
            .base_font(Name(name.as_bytes()))
            .system_info(system)
            .font_descriptor(descriptor)
            .default_width(0.0)
            .cid_to_gid_map_stream(gids);
        let mut w = cid_font.widths();
        for (id, width) in &widths {
            w.consecutive(*id, [*width]);
        }
        w.finish();
        cid_font.finish();
        let gid_map =
            miniz_oxide::deflate::compress_to_vec_zlib(&used::cid_to_gid(map.into_iter()), 6);
        pdf.stream(gids, &gid_map).filter(Filter::FlateDecode);
        let bbox = face.global_bounding_box();
        let mut flags = FontFlags::NON_SYMBOLIC;
        flags.set(FontFlags::ITALIC, face.is_italic());
        flags.set(FontFlags::FORCE_BOLD, key.bold);
        pdf.font_descriptor(descriptor)
            .name(Name(name.as_bytes()))
            .flags(flags)
            .bbox(Rect::new(
                units(bbox.x_min, per_em),
                units(bbox.y_min, per_em),
                units(bbox.x_max, per_em),
                units(bbox.y_max, per_em),
            ))
            .italic_angle(face.italic_angle())
            .ascent(units(face.ascender(), per_em))
            .descent(units(face.descender(), per_em))
            .cap_height(units(
                face.capital_height().unwrap_or(face.ascender()),
                per_em,
            ))
            .stem_v(if key.bold { 140.0 } else { 80.0 })
            .font_file2(file);
        let compressed = miniz_oxide::deflate::compress_to_vec_zlib(&subset, 6);
        pdf.stream(file, &compressed)
            .filter(Filter::FlateDecode)
            .pair(
                Name(b"Length1"),
                i32::try_from(subset.len()).unwrap_or(i32::MAX),
            );
        let mut unicode = UnicodeCmap::new(Name(b"Custom"), system);
        for (id, text) in &used.unicode {
            unicode.pair_with_multiple(*id, text.chars());
        }
        pdf.cmap(cmap, &unicode.finish());
        out.insert(
            face_id,
            Embedded {
                reference: type0,
                widths,
                unicode: used.unicode,
            },
        );
    }
    Ok(out)
}
