//! What a run is shaped as: script, language system, direction and the features switched on.
//!
//! Nothing here has a default. [`Features::word_default`] is the named choice that matches Word;
//! [`Features::none`] is the empty set to build from.

use ttf_parser::Tag;

/// A script the shaper implements, or text with no script of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Script {
    /// Latin (`latn`).
    Latin,
    /// Greek (`grek`).
    Greek,
    /// Cyrillic (`cyrl`).
    Cyrillic,
    /// Digits, punctuation and spaces only; shaped with the font's default script.
    Common,
    /// A script this release does not shape (Arabic, Devanagari, Han, ...).
    Unsupported,
}

impl Script {
    /// The OpenType script tag tried first. `Common` asks for `Zyyy`, which no font has, so the
    /// table's `DFLT`, `dflt` then `latn` fallbacks decide, as in every other shaper.
    pub(crate) fn tag(self) -> Option<Tag> {
        match self {
            Self::Latin => Some(Tag::from_bytes(b"latn")),
            Self::Greek => Some(Tag::from_bytes(b"grek")),
            Self::Cyrillic => Some(Tag::from_bytes(b"cyrl")),
            Self::Common => Some(Tag::from_bytes(b"zyyy")),
            Self::Unsupported => None,
        }
    }
}

/// Which way a run goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    /// Left to right.
    LeftToRight,
    /// Right to left. Refused for now with [`crate::ShapeError::UnsupportedDirection`].
    RightToLeft,
}

/// The OpenType language system a run uses: the script's default, or a named one (`TRK `,
/// `ROM `, ...), which is what `locl` keys on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Language(Option<[u8; 4]>);

impl Language {
    /// The script's default language system.
    #[must_use]
    pub const fn default_system() -> Self {
        Self(None)
    }

    /// A language system by its four-byte OpenType tag (space padded, e.g. `*b"TRK "`).
    #[must_use]
    pub const fn opentype(tag: [u8; 4]) -> Self {
        Self(Some(tag))
    }

    pub(crate) fn tag(self) -> Option<Tag> {
        self.0.map(|t| Tag::from_bytes(&t))
    }
}

/// An OpenType feature the shaper implements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Feature {
    /// Glyph composition and decomposition (`ccmp`).
    Ccmp,
    /// Localised forms (`locl`).
    Locl,
    /// Standard ligatures (`liga`).
    Liga,
    /// Contextual ligatures (`clig`).
    Clig,
    /// Contextual alternates (`calt`).
    Calt,
    /// Kerning (`kern`), from GPOS or else the legacy `kern` table.
    Kern,
    /// Mark to base and mark to ligature positioning (`mark`).
    Mark,
    /// Mark to mark positioning (`mkmk`).
    Mkmk,
}

impl Feature {
    /// Every feature, in the order the shaper collects them.
    pub const ALL: [Self; 8] = [
        Self::Ccmp,
        Self::Locl,
        Self::Mark,
        Self::Mkmk,
        Self::Calt,
        Self::Clig,
        Self::Kern,
        Self::Liga,
    ];

    /// The feature's OpenType tag.
    #[must_use]
    pub fn tag(self) -> Tag {
        Tag::from_bytes(match self {
            Self::Ccmp => b"ccmp",
            Self::Locl => b"locl",
            Self::Liga => b"liga",
            Self::Clig => b"clig",
            Self::Calt => b"calt",
            Self::Kern => b"kern",
            Self::Mark => b"mark",
            Self::Mkmk => b"mkmk",
        })
    }

    /// Mark positioning sees joiners (ZWJ/ZWNJ) as glyphs instead of skipping them.
    pub(crate) fn manual_joiners(self) -> bool {
        matches!(self, Self::Mark | Self::Mkmk)
    }

    fn bit(self) -> u8 {
        1 << (self as u8)
    }
}

/// The features a run is shaped with: on or off, nothing implied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Features(u8);

impl Features {
    /// No features at all: nominal glyphs and their advances.
    #[must_use]
    pub const fn none() -> Self {
        Self(0)
    }

    /// What Word applies to a run it has not been told otherwise about: composition, locale
    /// forms, standard and contextual ligatures, contextual alternates, kerning and mark
    /// positioning, all on.
    #[must_use]
    pub fn word_default() -> Self {
        Feature::ALL
            .into_iter()
            .fold(Self::none(), |f, x| f.with(x, true))
    }

    /// The same set with `feature` switched on or off.
    #[must_use]
    pub fn with(self, feature: Feature, on: bool) -> Self {
        if on {
            Self(self.0 | feature.bit())
        } else {
            Self(self.0 & !feature.bit())
        }
    }

    /// Whether `feature` is on.
    #[must_use]
    pub fn contains(self, feature: Feature) -> bool {
        self.0 & feature.bit() != 0
    }
}

/// Everything a run is shaped as.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShapeParams {
    /// The run's script, normally from [`crate::itemise`].
    pub script: Script,
    /// The language system.
    pub language: Language,
    /// The direction.
    pub direction: Direction,
    /// The features.
    pub features: Features,
}
