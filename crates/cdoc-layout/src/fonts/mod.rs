// Adapted from rMazing crates/cc-export/src/layout/fonts.rs
// Copyright (c) Thomas Braun. Relicensed MIT OR Apache-2.0 by the copyright holder, 2026-10-04.

//! Faces, asked for by the cascade's `(family, bold, italic)`, parsed once, and the text set
//! in them shaped (`cdoc-shape`: ligatures, kerning, mark positions) into glyphs.
//!
//! The bytes arrive from the caller through [`FontPort`] (fetched by the browser, read from disk
//! by a test) because this crate is pure and does not know where a file is. The port decides
//! what to serve for a family it does not have, and says what it served, so a substitution is
//! visible in the layout's warnings rather than silent.
//!
//! Measured by shaping, not counted: a run's width is the sum of its shaped advances, and the
//! backends draw exactly those glyphs at exactly those advances, so a line that fits on paper
//! fits on screen.

mod library;
mod sanitise;
mod shaping;
mod trace;

pub use library::FontLibrary;
pub use sanitise::sanitise;

use crate::ir::{FaceId, FaceKey, Pt100};

/// U+00AD, a discretionary hyphen: invisible unless the line breaks there.
pub const SOFT_HYPHEN: char = '\u{ad}';
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

/// Where faces and hyphenation patterns come from. Implemented by the caller: the WASM facade
/// over files handed in from JavaScript, a test over files on disk.
pub trait FontPort {
    /// The face for a resolved run. Substitution is the port's policy; the result names what
    /// was actually served.
    fn face(&self, family: &str, bold: bool, italic: bool) -> Result<FaceSource<'_>, FontError>;
    /// The patterns registered for `language`; text in a language without them is not
    /// hyphenated, and the layout says so (`Warning::NoHyphenationPatterns`).
    fn hyphenator(&self, language: cdoc_para::Language) -> Option<&cdoc_para::Hyphenator>;
}

/// Vertical metrics of a face, in font units.
#[derive(Debug, Clone, Copy)]
struct Vertical {
    ascent: i32,
    descent: i32,
}

struct Parsed<'a> {
    face: ttf_parser::Face<'a>,
    /// Bit `c` set when the face has a glyph for ASCII character `c`: most text asks only
    /// these, and the `cmap` is consulted for the rest.
    ascii: u128,
    vertical: Vertical,
    shaper: cdoc_shape::Shaper<'a, 'a>,
    words: shaping::Words,
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

/// `units` at `size`, by the shaper's one rounding rule.
fn scale(units: i64, size: Pt100, per_em: u16) -> Pt100 {
    cdoc_shape::scale_size(units, size, per_em)
}

impl<'a> Fonts<'a> {
    /// The port's patterns for `language`.
    #[must_use]
    pub fn hyphenator(&self, language: cdoc_para::Language) -> Option<&'a cdoc_para::Hyphenator> {
        self.port.hyphenator(language)
    }

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
            let unreadable = || FontError::Unreadable(served.clone());
            let face = ttf_parser::Face::parse(source.bytes, 0).map_err(|_| unreadable())?;
            let shaping = cdoc_shape::Face::parse(source.bytes, 0).map_err(|_| unreadable())?;
            let index = u16::try_from(self.keys.len()).map_err(|_| unreadable())?;
            let ascii = (0..128_u8)
                .filter(|c| face.glyph_index(char::from(*c)).is_some_and(|g| g.0 != 0))
                .fold(0_u128, |bits, c| bits | (1 << c));
            self.parsed.push(Parsed {
                ascii,
                vertical: vertical(&face),
                face,
                shaper: cdoc_shape::Shaper::owning(shaping),
                words: shaping::Words::new(),
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
    #[inline]
    pub fn has_glyph(&self, id: FaceId, ch: char) -> bool {
        let parsed = self.parsed(id);
        if ch.is_ascii() {
            return parsed.ascii & (1 << u32::from(ch)) != 0;
        }
        parsed.face.glyph_index(ch).is_some_and(|gid| gid.0 != 0)
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
