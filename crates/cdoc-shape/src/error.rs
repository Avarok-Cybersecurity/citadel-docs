//! What can go wrong, as values.

use crate::params::Script;

/// A request the shaper refuses, or a font it cannot use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ShapeError {
    /// The bytes did not parse as a TrueType or OpenType font.
    #[error("the bytes did not parse as a TrueType or OpenType font")]
    Unreadable,
    /// The face maps no Unicode (or Windows symbol) characters to glyphs.
    #[error("the face has no Unicode character map")]
    NoUnicodeCmap,
    /// The script is outside what this shaper implements.
    #[error("the script {0:?} is not supported by this shaper")]
    UnsupportedScript(Script),
    /// Right-to-left runs are not shaped yet; bidi comes in a later release.
    #[error("right-to-left shaping is not supported yet")]
    UnsupportedDirection,
    /// The text is longer than a `u32` byte index can address.
    #[error("the text is {0} bytes long, more than a cluster index can address")]
    TextTooLong(usize),
    /// The font's lookups grew the run or recursed past the shaper's budget (a malformed or
    /// hostile font).
    #[error("shaping exceeded its operation budget; the font's lookups do not terminate sanely")]
    Budget,
}
