//! Why an edit, a conversion or a remote update was refused.

use cdoc_crdt::{BridgeError, DecodeError};
use cdoc_layout::LayoutError;
use cdoc_paint::PaintError;

/// What went wrong.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EditError {
    /// The CRDT refused an edit.
    #[error(transparent)]
    Crdt(#[from] cdoc_crdt::EditError),
    /// The CRDT and the model disagree.
    #[error(transparent)]
    Bridge(#[from] BridgeError),
    /// An update, snapshot, command or awareness payload did not decode.
    #[error(transparent)]
    Decode(#[from] DecodeError),
    /// The layout failed.
    #[error(transparent)]
    Layout(#[from] LayoutError),
    /// An image file is not one the documents carry.
    #[error(transparent)]
    Image(#[from] cdoc_model::image_probe::ProbeError),
    /// An image file is larger than the caller allows.
    #[error("the image is {size} bytes; at most {limit} are allowed")]
    ImageTooLarge {
        /// Its size.
        size: usize,
        /// The caller's limit.
        limit: usize,
    },
    /// Tombstone collection was refused.
    #[error(transparent)]
    Gc(#[from] cdoc_crdt::GcError),
    /// A diff could not be cut under the caller's limit.
    #[error(transparent)]
    Chunk(#[from] cdoc_crdt::ChunkError),
    /// A style the document names does not resolve.
    #[error(transparent)]
    Style(#[from] cdoc_model::StyleError),
    /// Painting failed.
    #[error(transparent)]
    Paint(#[from] PaintError),
    /// A position names a unit this replica does not hold.
    #[error("the position names a unit this replica does not hold")]
    UnknownPosition,
    /// The selection is not where the command applies (a table command outside a table, a
    /// merge across two tables, ...).
    #[error("the command does not apply here: {0}")]
    NotApplicable(&'static str),
    /// A command's arguments are out of range (zero rows, list level above 8, ...).
    #[error("invalid command argument: {0}")]
    InvalidArgument(&'static str),
    /// The page index is past the last page.
    #[error("there is no page {0}")]
    NoPage(usize),
    /// The document's flat structure is not one the editor can map to blocks (only concurrent
    /// structural edits produce it); the document is still shown, but structural commands wait
    /// until it is well formed again.
    #[error("the document structure is irregular")]
    Irregular,
}
