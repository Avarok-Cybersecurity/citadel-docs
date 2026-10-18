//! What an update writes, read without a replica: for a relay that stores and forwards updates
//! it never applies, and must know whose clocks each one carries.

use super::decode::decode;
use crate::bytes::DecodeError;
use crate::id::{ClientId, StateVector};
use std::ops::Range;

/// The clocks one update writes, and the GC floor its sender required.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateWrites {
    /// The sender's GC floor: a replica that does not hold all of it cannot apply the update.
    pub floor: StateVector,
    /// Each run of one client's operations, in the order the update holds them: the clocks
    /// `start..end` that client wrote. Never empty; a client may appear more than once.
    pub runs: Vec<(ClientId, Range<u64>)>,
}

impl UpdateWrites {
    /// The smallest state vector holding every clock the update writes. Clocks below a run's
    /// start are not the update's, so this is what a replica holds after applying it to one
    /// that already held everything before each run.
    #[must_use]
    pub fn version(&self) -> StateVector {
        let mut sv = StateVector::empty();
        for (client, run) in &self.runs {
            if run.end > sv.get(*client) {
                sv.set(*client, run.end);
            }
        }
        sv
    }
}

/// Decodes `bytes` as an update, every operation read and checked as
/// [`Doc::apply_update`](crate::Doc::apply_update) reads it, and reports what it writes
/// without applying it. Bytes `apply_update` would refuse to decode are refused here alike.
pub fn update_writes(bytes: &[u8]) -> Result<UpdateWrites, DecodeError> {
    let decoded = decode(bytes)?;
    let runs = decoded
        .clients
        .into_iter()
        .filter_map(|(client, ops)| {
            let start = ops.first()?.0;
            let (last, op) = ops.last()?;
            // `decode` refused a run whose clocks overflow, so this end is in range.
            Some((client, start..last + op.len()))
        })
        .collect();
    Ok(UpdateWrites {
        floor: decoded.floor,
        runs,
    })
}
