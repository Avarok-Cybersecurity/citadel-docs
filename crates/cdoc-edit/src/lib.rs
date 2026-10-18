//! `cdoc-edit`: the Citadel Docs editing core. Pure Rust, no I/O, no clock, no randomness: the
//! client id, footnote and comment ids, authors and dates arrive from the caller, faces through
//! the layout's `FontPort`.
//!
//! # Shape
//!
//! An [`Editor`] owns a `cdoc_crdt::Doc` (the source of truth), a model view of it (kept equal
//! to `to_model` incrementally: only the top-level blocks an edit touched are re-read), the
//! block **outline** (`outline.rs`: which top-level block, container and table each structural
//! atom belongs to), a `LayoutSession`, the line maps (`geometry`), the selection, the undo
//! history and an IME composition.
//!
//! # Commands
//!
//! A [`Command`] runs as **one CRDT transaction**: its primitive edits go through a `Tx` that
//! records, for each, the step that undoes it, and the units it touched. Afterwards the touched
//! units become block ordinals, the outline is rebuilt if structure changed, the changed
//! top-level blocks become the layout's `Dirty` and are re-read into the model, and the layout
//! relays out only what moved. [`Applied`] carries the update for the relay (the diff since
//! before the command), the pages to repaint and the selection. Typing inherits the formatting
//! of the character before the caret (the one after it at a paragraph start), as Word does;
//! a format toggled on a collapsed selection becomes *pending* and applies to the next text.
//!
//! # Positions
//!
//! A [`Position`] is the gap after a unit, named by the unit's CRDT id: remote edits before it
//! move it with the text, and a remote deletion of that unit leaves it where the unit was.
//! [`BlockOffset`] and [`ModelPath`] convert it for callers that think in blocks.
//!
//! # Undo
//!
//! Undo inverts the recorded steps through the CRDT: inserted units are deleted by id, deleted
//! content is inserted again after the unit that stood before it (restored copies get new ids,
//! and older history entries are remapped to them), formatting and block properties are set
//! back. Concurrent remote edits are not touched, and remote updates never
//! enter the history. Typing groups without a clock (see `history.rs`).
//!
//! # Geometry
//!
//! Caret rectangles, selection rectangles, hit testing and vertical moves read line maps
//! derived from the layout IR's caret lines, for the body and for headers, footers and
//! footnote bodies. Each line names the paragraph it was laid out from and its caret stops by
//! that paragraph's gaps, so no text is matched: `geometry/derive.rs` maps them to positions.
//!
//! # IME, awareness, wire
//!
//! Composition text is a local overlay until commit (`ime.rs` explains the choice). Peers'
//! selections travel as a small binary [`Awareness`] record. Requests and results cross the
//! WASM boundary as the binary records of `docs/edit-commands.md` ([`wire`]).

#![forbid(unsafe_code)]

mod address;
mod attr_revert;
mod awareness;
mod caret;
mod caret_list;
mod commands;
mod editor;
mod error;
mod footnotes;
mod fresh;
mod geometry;
mod history;
mod ime;
mod outline;
mod position;
mod restore;
mod run_keys;
mod step;
mod story;
mod sync;
mod text;
mod tx;
pub mod wire;

pub use address::{BlockOffset, ModelPath};
pub use awareness::Awareness;
pub use caret::{CaretFormatting, ParagraphKey};
pub use caret_list::{CaretList, CaretListKind};
pub use commands::{Command, Created, DeleteUnit, MarkKind};
pub use editor::{
    AddedImage, Applied, Editor, PeerView, RemoteApplied, StyleEntry, MAX_SELECTION_TEXT_BYTES,
};
pub use error::EditError;
pub use ime::Composition;
pub use position::{Direction, Granularity, Position, Selection};
pub use story::Story;
