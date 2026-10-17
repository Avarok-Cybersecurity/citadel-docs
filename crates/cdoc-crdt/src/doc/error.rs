//! Why a local edit was refused.

use crate::codec::CodecError;
use crate::item::AtomKind;

/// A local edit that cannot be made.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EditError {
    /// An index past the end of the visible sequence.
    #[error("index {index} is past the end ({len})")]
    OutOfRange {
        /// The index asked for.
        index: usize,
        /// The visible length.
        len: usize,
    },
    /// Empty text, or an empty or reversed range.
    #[error("nothing to edit")]
    Empty,
    /// The unit is not a structural atom.
    #[error("unit {index} is not a structural atom")]
    NotStructural {
        /// The index.
        index: usize,
    },
    /// The unit is a structural atom of another kind.
    #[error("unit {index} is a {found:?} atom, not a {expected:?} atom")]
    KindMismatch {
        /// The index.
        index: usize,
        /// The kind the new properties are for.
        expected: AtomKind,
        /// The kind found.
        found: AtomKind,
    },
    /// The value could not be encoded.
    #[error(transparent)]
    Codec(#[from] CodecError),
    /// The text buffer would pass 4 GiB.
    #[error("the document's text would exceed 4 GiB")]
    TooLarge,
    /// Text goes in with `insert_text`, never as an atom.
    #[error("text is inserted with insert_text, not as an atom")]
    TextAtom,
    /// A section atom's properties must not carry headers or footers: they are blocks.
    #[error("headers and footers are blocks of their own, not section properties")]
    FurnitureInSection,
}
