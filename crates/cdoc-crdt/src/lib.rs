//! `cdoc-crdt`: the Citadel Docs rich-text CRDT. Pure Rust, no I/O, no clock, no randomness:
//! the caller supplies the client id ([`Doc::new`]); everything else is derived.
//!
//! # Model
//!
//! A document is **one flat sequence of units**: characters, inline atoms (image, break, tab,
//! footnote reference, field, ...) and structural atoms that delimit blocks ([`Atom`]: section,
//! header/footer, footnote, paragraph, table, row, cell, end). Headers, footers and footnotes
//! are delimited sub-sequences of the same sequence, so one integration algorithm covers all of
//! it. [`to_model`] folds the sequence into a `cdoc_model::Document`; [`from_model`] writes one
//! out; `to_model(from_model(d)) == d` normalised, for every document the model's strategies
//! generate. The model stays the source of truth for what values mean: atoms, marks and
//! attributes carry model values in the [`codec`], never copies of the model's types.
//!
//! # Sequence
//!
//! A YATA list CRDT with Yjs's conflict rule (`doc/integrate.rs`). Items are **runs**:
//! consecutive units from one client, each the left origin of the next, all sharing a right
//! origin. Sequential typing extends the run in place (one item for a million characters); a
//! run is split only when something lands inside it, and split pieces that are still chained
//! are written as one operation, so encodings do not depend on how a replica's items happen to
//! be split. Items sit in an arena; document order is a list of 64-item leaves under a Fenwick
//! tree of visible weights, so position lookups are `O(log n)`, and integrating a remote run
//! is a lookup of each origin (a `BTreeMap` per client) plus the YATA scan between them.
//! Deleted items keep their id and origins (concurrent edits may name them) and drop their
//! content; deletes are state, so they are idempotent and commute.
//!
//! # Formatting and attributes
//!
//! Character properties, links, tracked changes and comment anchors are **Peritext-style mark
//! ranges** (`marks.rs`): each end anchors to a unit's id on one side of it, so concurrent
//! "bold this range" and "type inside it" commute, and bold grows at its end while a link does
//! not. Each `RunProperties` field is its own key. The covering mark with the greatest Lamport
//! timestamp, ties to the greater client, decides a key; comments are multi-valued.
//!
//! Document attributes ([`DocAttr`]), each field of each block's properties (keyed by its
//! structural atom and the field's path) and each element of a list-valued attribute (comments,
//! styles, numbering definitions and instances, keyed by the element's id) are **multi-value
//! registers ordered by vector clocks**: a write carries the writer's state vector and
//! overwrites what it saw; among concurrent survivors the greatest client id wins (`attrs.rs`).
//!
//! # Sync
//!
//! Every unit, mark and attribute write takes one clock of its client, so a [`StateVector`]
//! (= [`Version`]) says exactly what a replica holds. Deletions are operations too (each a
//! clock of its deleter naming the ranges it deletes), so [`Doc::encode_diff`] writes exactly
//! what a peer at a given state vector lacks, deletions included, never the whole delete set; [`Doc::apply_update`] integrates it, buffers operations whose
//! dependencies have not arrived (reporting them in [`Applied::missing`]), ignores duplicates,
//! and reports the dirty blocks and attributes ([`Changes`]). For a transport that caps message
//! size, [`Doc::encode_diff_chunked`] cuts the same diff into updates of at most a given size,
//! causally ordered so that applied in order none waits for a later one ([`ChunkError`] names
//! a limit no update or operation fits). [`Doc::encode_snapshot`] writes
//! the full state in document order; [`Doc::decode_snapshot`] loads it in one pass without
//! integrating anything; [`diff_snapshots`] turns two snapshots into the update between them.
//! [`Doc::gc`] collects tombstones and delete operations every known peer has seen; it never
//! runs implicitly (`gc/mod.rs` says what it keeps and why).
//!
//! # Binary format
//!
//! Integers are unsigned LEB128 varints; strings and byte strings are a varint length then
//! the bytes. There are no floats anywhere: the [`codec`] refuses them.
//!
//! ```text
//! header      := "CDC" 0x02  kind:u8           (0 = update, 1 = snapshot; version 1 is refused
//!                by name, DecodeError::Version { found, supported })
//! update      := header  floor:state_vector  n:varint client_ops{n}
//! client_ops  := client:varint first_clock:varint count:varint op{count}
//!                (each op's clock is the previous op's clock plus its length)
//! op          := head:u8 body                  (head & 7 is the tag)
//!   0 text    := origins  chars:varint  utf8:bytes
//!   1 atom    := origins  kind:u8  payload:bytes                   (one unit)
//!   2 gone    := origins  len:varint                               (a tombstone)
//!   3 mark    := lamport:varint start:anchor end:anchor key value
//!   4 attr    := attr_key deps:state_vector value:bytes
//!   5 skip    := len:varint                    (clocks that carry nothing any more)
//!   6 delete  := k:varint (client:varint start:varint len:varint){k}  (one clock; ranges
//!                sorted, disjoint, not touching, non-empty)
//! origins     := head bit 3: has a left origin; bit 4: it is the previous clock (no bytes);
//!                bit 5: same client (clock only); bit 6: has a right origin; bit 7: same
//!                client (clock only); otherwise an id
//! id          := client:varint clock:varint
//! anchor      := 0 start | 1 end | 2 before:id | 3 after:id
//! key         := 0..=11 RunKey | 12 link | 13 revision | 14 comment:str
//!                (15 kern, 16 ligatures, 17 lang are reserved for RunKey)
//! value       := 0 (remove) | 1 bytes
//! attr_key    := 0 name:str | 1 block:id field:str | 2 list:u8 element_id:bytes
//!                (list 0 comments, 1 styles, 2 numbering definitions, 3 numbering instances)
//! state_vector:= n:varint (client:varint next_clock:varint){n}, ascending clients
//! snapshot    := header state_vector floor:state_vector text:bytes items:varint item*
//!                ops:varint op_at*
//! item        := client_index:varint clock:varint op    (document order; a text op holds
//!                a byte length into the text section instead of its bytes)
//! op_at       := client_index:varint gap:varint (mark | attr | delete)   (client indices
//!                ascending; clock = gap + the client's previous op_at's clock + 1, or gap)
//! ```
//!
//! A snapshot's clocks covered by no item, mark or attribute are skips.

#![forbid(unsafe_code)]

mod attrs;
mod bridge;
mod bytes;
mod changes;
pub mod codec;
mod doc;
mod gc;
mod id;
mod item;
mod marks;
mod seq;
mod snapshot;
mod update;

pub use bridge::{
    atom_at, blank, fold_blocks, formats, from_model, names, slice, to_model, Atom, BridgeError,
    DocAttr, Formats, Furniture, Mark, Piece,
};
pub use bytes::DecodeError;
pub use changes::Changes;
pub use doc::{Doc, EditError, Located, Structural, Unit, Units};
pub use gc::{Collected, GcError};
pub use id::{ClientId, Id, StateVector, Version};
pub use item::AtomKind;
pub use marks::RunKey;
pub use snapshot::{diff_snapshots, snapshot_version};
pub use update::{Applied, ChunkError};
