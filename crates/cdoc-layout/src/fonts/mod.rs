// Adapted from rMazing crates/cc-export/src/layout/fonts.rs
// Copyright (c) Thomas Braun. Relicensed MIT OR Apache-2.0 by the copyright holder, 2026-10-04.

//! Faces, asked for by the cascade's `(family, bold, italic)`, parsed once and measured glyph
//! by glyph.
//!
//! The bytes arrive from the caller through [`FontPort`] (fetched by the browser, read from disk
//! by a test) because this crate is pure and does not know where a file is. The port decides
//! what to serve for a family it does not have, and says what it served, so a substitution is
//! visible in the layout's warnings rather than silent.
//!
//! Measured, not counted, and without kerning: a PDF `Tj` places glyphs by advance, and the SVG
//! preview is told not to kern, so a line that fits on paper fits on screen.

mod library;
mod trace;

pub use library::FontLibrary;

use crate::ir::{FaceId, FaceKey, Pt100};
use std::collections::BTreeMap;

/// A face's bytes and what it actually is.
#[derive(Debug, Clone, Copy)]
pub struct FaceSource<'a> {
    /// The face served, which may differ from the one requested.
    pub family: &'a str,
    /// Whether the served face is bold.
    pub bold: bool,
    /// Whether the served face is italic.
    pub italic: bool,
    /// A TrueType or OpenType file.
    pub bytes: &'a [u8],
}

/// A font the port could not provide or the layout could not read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FontError {
    /// The port has nothing for this request, not even a substitute.
    #[error("no face for {0:?}")]
    Missing(FaceKey),
    /// The bytes did not parse as a TrueType or OpenType font.
    #[error("the face {0:?} did not parse as a TrueType or OpenType font")]
    Unreadable(FaceKey),
}

/// Where faces come from. Implemented by the caller: the WASM facade over fonts handed in from
/// JavaScript, a test over files on disk.
pub trait FontPort {
    /// The face for a resolved run. Substitution is the port's policy; the result names what
    /// was actually served.
    fn face(&self, family: &str, bold: bool, italic: bool) -> Result<FaceSource<'_>, FontError>;
}

/// Vertical metrics of a face, in font units.
#[derive(Debug, Clone, Copy)]
struct Vertical {
    ascent: i32,
    descent: i32,
}

struct Parsed<'a> {
    face: ttf_parser::Face<'a>,
    vertical: Vertical,
}

/// The faces a layout has used, parsed. Indexed by [`FaceId`].
pub struct Fonts<'a> {
    port: &'a dyn FontPort,
    keys: Vec<FaceKey>,
    parsed: Vec<Parsed<'a>>,
    by_request: BTreeMap<FaceKey, usize>,
    /// Every request met, by request index: what was asked for and the face that served it.
    /// `None` once a rebase dropped the face it was served by.
    requests: Vec<Option<(FaceKey, FaceId)>>,
    /// Request indices, one per [`Fonts::resolve`] call, until taken.
    log: Vec<usize>,
}

impl std::fmt::Debug for Fonts<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(&self.keys).finish()
    }
}

fn vertical(face: &ttf_parser::Face<'_>) -> Vertical {
    // The Windows metrics are what Word sets single spacing from; hhea is the fallback.
    if let Some(os2) = face.tables().os2 {
        let ascent = i32::from(os2.windows_ascender());
        let descent = i32::from(os2.windows_descender()).abs();
        if ascent > 0 {
            return Vertical { ascent, descent };
        }
    }
    Vertical {
        ascent: i32::from(face.ascender()),
        descent: i32::from(face.descender()).abs(),
    }
}

/// `units * size / units_per_em`, rounded, in hundredths.
fn scale(units: i64, size: Pt100, per_em: u16) -> Pt100 {
    let value = (units * i64::from(size) + i64::from(per_em) / 2) / i64::from(per_em).max(1);
    Pt100::try_from(value).unwrap_or(Pt100::MAX)
}

impl<'a> Fonts<'a> {
    /// No faces yet; each is fetched and parsed the first time a run asks for it.
    #[must_use]
    pub fn new(port: &'a dyn FontPort) -> Self {
        Self {
            port,
            keys: Vec::new(),
            parsed: Vec::new(),
            by_request: BTreeMap::new(),
            requests: Vec::new(),
            log: Vec::new(),
        }
    }

    /// The face for a request, fetched and parsed on first use.
    pub fn resolve(&mut self, family: &str, bold: bool, italic: bool) -> Result<FaceId, FontError> {
        let requested = FaceKey {
            family: family.to_owned(),
            bold,
            italic,
        };
        if let Some(&index) = self.by_request.get(&requested) {
            if let Some((_, id)) = &self.requests[index] {
                self.log.push(index);
                return Ok(*id);
            }
        }
        let source = self.port.face(family, bold, italic)?;
        let served = FaceKey {
            family: source.family.to_owned(),
            bold: source.bold,
            italic: source.italic,
        };
        let id = if let Some(index) = self.keys.iter().position(|k| *k == served) {
            FaceId(u16::try_from(index).map_err(|_| FontError::Unreadable(served.clone()))?)
        } else {
            let face = ttf_parser::Face::parse(source.bytes, 0)
                .map_err(|_| FontError::Unreadable(served.clone()))?;
            let index = u16::try_from(self.keys.len())
                .map_err(|_| FontError::Unreadable(served.clone()))?;
            self.parsed.push(Parsed {
                vertical: vertical(&face),
                face,
            });
            self.keys.push(served.clone());
            FaceId(index)
        };
        let index = self.requests.len();
        self.requests.push(Some((requested.clone(), id)));
        self.by_request.insert(requested, index);
        self.log.push(index);
        Ok(id)
    }

    fn parsed(&self, id: FaceId) -> &Parsed<'a> {
        &self.parsed[usize::from(id.0)]
    }

    /// Whether the face has a glyph for the character. A control character never does.
    #[must_use]
    pub fn has_glyph(&self, id: FaceId, ch: char) -> bool {
        self.parsed(id)
            .face
            .glyph_index(ch)
            .is_some_and(|gid| gid.0 != 0)
    }

    /// The advance of one character at `size`. Unknown glyphs measure as `.notdef`, which is
    /// what the page would draw.
    #[must_use]
    pub fn advance(&self, id: FaceId, size: Pt100, ch: char) -> Pt100 {
        let face = &self.parsed(id).face;
        let gid = face.glyph_index(ch).unwrap_or(ttf_parser::GlyphId(0));
        let units = face.glyph_hor_advance(gid).unwrap_or(0);
        scale(i64::from(units), size, face.units_per_em())
    }

    /// The width of a string at `size`: the sum of its advances in font units, scaled once.
    #[must_use]
    pub fn width(&self, id: FaceId, size: Pt100, text: &str) -> Pt100 {
        let face = &self.parsed(id).face;
        let units: i64 = text
            .chars()
            .map(|ch| {
                let gid = face.glyph_index(ch).unwrap_or(ttf_parser::GlyphId(0));
                i64::from(face.glyph_hor_advance(gid).unwrap_or(0))
            })
            .sum();
        scale(units, size, face.units_per_em())
    }

    /// Ascent above the baseline at `size`.
    #[must_use]
    pub fn ascent(&self, id: FaceId, size: Pt100) -> Pt100 {
        let p = self.parsed(id);
        scale(i64::from(p.vertical.ascent), size, p.face.units_per_em())
    }

    /// Descent below the baseline at `size`, as a positive length.
    #[must_use]
    pub fn descent(&self, id: FaceId, size: Pt100) -> Pt100 {
        let p = self.parsed(id);
        scale(i64::from(p.vertical.descent), size, p.face.units_per_em())
    }

    /// The faces used so far, by id.
    #[must_use]
    pub fn keys(&self) -> &[FaceKey] {
        &self.keys
    }

    /// Every request that was served by a different face, in the order first asked.
    #[must_use]
    pub fn substitutions(&self) -> Vec<(FaceKey, FaceKey)> {
        (0..self.requests.len())
            .filter_map(|i| self.substitution(i))
            .collect()
    }
}

/// Text the face can draw, with every character it cannot replaced by a visible marker, and
/// how many were replaced, so the count can be reported rather than left for a reader to
/// notice.
///
/// Plain ASCII marker: a non-ASCII one looked safe once and rendered as a euro sign, which is
/// exactly the silent mangling the marker exists to make visible.
#[must_use]
pub fn sanitise(text: &str, fonts: &Fonts<'_>, face: FaceId) -> (String, usize) {
    const MARKER: &str = "[?]";
    let mut out = String::with_capacity(text.len());
    let mut replaced = 0;
    for ch in text.chars() {
        if fonts.has_glyph(face, ch) {
            out.push(ch);
        } else {
            out.push_str(MARKER);
            replaced += 1;
        }
    }
    (out, replaced)
}
