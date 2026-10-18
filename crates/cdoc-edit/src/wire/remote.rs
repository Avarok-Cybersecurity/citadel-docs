//! The result's remote section (version 5): what applying a peer's update left waiting.
//!
//! ```text
//! remote := 0                                 a local request's result
//!         | 1 missing:(0 | 1 client:varint clock:varint) dropped:varint
//! ```

use super::{Reader, Writer};
use cdoc_crdt::{ClientId, DecodeError, Id};

/// What a peer's update left waiting, for the caller to act on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Remote {
    /// The first unit the buffered operations wait for: ask the peer for a diff when set.
    pub missing: Option<Id>,
    /// Waiting operations dropped to stay within the limits (a later diff sends them again).
    pub dropped: u64,
}

pub(super) fn write(w: &mut Writer, remote: Option<Remote>) {
    let Some(remote) = remote else {
        w.u8(0);
        return;
    };
    w.u8(1);
    match remote.missing {
        Some(id) => {
            w.u8(1);
            w.uvar(id.client.0);
            w.uvar(id.clock);
        }
        None => w.u8(0),
    }
    w.uvar(remote.dropped);
}

pub(super) fn read(r: &mut Reader<'_>) -> Result<Option<Remote>, DecodeError> {
    match r.u8()? {
        0 => return Ok(None),
        1 => {}
        tag => {
            return Err(DecodeError::BadTag {
                what: "remote",
                tag,
            })
        }
    }
    let missing = match r.u8()? {
        0 => None,
        1 => Some(Id::new(ClientId(r.uvar()?), r.uvar()?)),
        tag => {
            return Err(DecodeError::BadTag {
                what: "missing",
                tag,
            })
        }
    };
    Ok(Some(Remote {
        missing,
        dropped: r.uvar()?,
    }))
}
