//! Awareness: where a peer's caret and selection are, as positions, in a small binary record a
//! relay forwards as is.
//!
//! ```text
//! awareness := "CDA" 0x01 client:varint anchor:position head:position
//! position  := client:varint clock:varint        (the id of the unit before the gap)
//! ```

use crate::error::EditError;
use crate::position::Selection;
use crate::wire::{Reader, Writer};
use cdoc_crdt::{ClientId, DecodeError};

const MAGIC: &[u8; 4] = b"CDA\x01";

/// A peer's selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Awareness {
    /// The peer.
    pub client: ClientId,
    /// Its selection.
    pub selection: Selection,
}

impl Awareness {
    /// The binary record.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer::new();
        w.raw(MAGIC);
        w.uvar(self.client.0);
        w.position(self.selection.anchor);
        w.position(self.selection.head);
        w.finish()
    }

    /// Reads [`Awareness::encode`]'s output.
    pub fn decode(bytes: &[u8]) -> Result<Self, EditError> {
        let mut r = Reader::new(bytes);
        if r.take(4)? != MAGIC {
            return Err(DecodeError::BadMagic.into());
        }
        let client = ClientId(r.uvar()?);
        let anchor = r.position()?;
        let head = r.position()?;
        r.finish()?;
        Ok(Self {
            client,
            selection: Selection { anchor, head },
        })
    }
}
