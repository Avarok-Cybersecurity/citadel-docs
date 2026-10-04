// Adapted from rMazing crates/cc-export/src/layout/fonts.rs (subsetting) and
// crates/cc-export/src/backends/pdf.rs (embedding)
// Copyright (c) Thomas Braun. Relicensed MIT OR Apache-2.0 by the copyright holder, 2026-10-04.

//! Faces embedded as subsets: every face a run is set in, cut down to the glyphs the document
//! draws, as a Type 0 / `CIDFontType2` font with `Identity-H` encoding and a `ToUnicode` map so
//! text copied out of the file is the text that was laid out.
//!
//! Strings reach the content stream as two-byte glyph ids, so a run reading `:)` cannot close
//! the string it is written into: the parenthesis is a number.

use cdoc_layout::{FaceKey, FontPort, Layout};
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

/// One embedded face: how to encode its text and where its font dictionary is.
pub struct Embedded {
    /// The Type 0 font dictionary.
    pub reference: Ref,
    remap: BTreeMap<char, u16>,
}

impl Embedded {
    /// The run's text as two-byte glyph ids in the subset.
    #[must_use]
    pub fn encode(&self, text: &str) -> Vec<u8> {
        text.chars()
            .flat_map(|c| self.remap.get(&c).copied().unwrap_or(0).to_be_bytes())
            .collect()
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

/// Embeds every face the layout's runs use; `alloc` hands out object numbers.
pub fn embed(
    pdf: &mut Pdf,
    alloc: &mut Ref,
    layout: &Layout,
    fonts: &dyn FontPort,
) -> Result<BTreeMap<u16, Embedded>, EmbedError> {
    let mut chars: BTreeMap<u16, std::collections::BTreeSet<char>> = BTreeMap::new();
    for run in layout.pages.iter().flat_map(cdoc_layout::Page::runs) {
        chars
            .entry(run.face.0)
            .or_default()
            .extend(run.text.chars());
    }
    let mut out = BTreeMap::new();
    for (face_id, used) in chars {
        let key = &layout.faces[usize::from(face_id)];
        let source = fonts.face(&key.family, key.bold, key.italic)?;
        let failed = |why: &dyn std::fmt::Display| EmbedError::Subset(key.clone(), why.to_string());
        let face = ttf_parser::Face::parse(source.bytes, 0).map_err(|e| failed(&e))?;
        let mut remapper = GlyphRemapper::new();
        let mut remap = BTreeMap::new();
        let mut to_unicode = BTreeMap::new();
        for c in used {
            let old = face.glyph_index(c).map_or(0, |g| g.0);
            let new = remapper.remap(old);
            remap.insert(c, new);
            to_unicode.entry(new).or_insert(c);
        }
        let subset =
            subsetter::subset(source.bytes, 0, &remapper).map_err(|e| failed(&format!("{e:?}")))?;
        let per_em = f32::from(face.units_per_em());
        let old_gids: Vec<u16> = remapper.remapped_gids().collect();
        let widths: Vec<f32> = old_gids
            .iter()
            .map(|g| {
                f32::from(face.glyph_hor_advance(ttf_parser::GlyphId(*g)).unwrap_or(0)) * 1000.0
                    / per_em
            })
            .collect();
        let name = format!("{}+{}", tag(key, &old_gids), base_name(&face, key));
        let [type0, cid, descriptor, file, cmap] = std::array::from_fn(|_| alloc.bump());
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
            .cid_to_gid_map_predefined(Name(b"Identity"));
        cid_font.widths().consecutive(0, widths);
        cid_font.finish();
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
        for (gid, c) in &to_unicode {
            unicode.pair(*gid, *c);
        }
        pdf.cmap(cmap, &unicode.finish());
        out.insert(
            face_id,
            Embedded {
                reference: type0,
                remap,
            },
        );
    }
    Ok(out)
}
