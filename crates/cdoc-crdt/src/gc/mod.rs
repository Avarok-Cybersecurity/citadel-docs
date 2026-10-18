//! Garbage collection of tombstones every known peer has acknowledged.
//!
//! Never automatic: the relay calls [`Doc::gc`] in its compaction step, with the latest state
//! vector each peer has acknowledged, when it stores a snapshot. The **frontier** is their
//! pointwise minimum: every known peer holds every operation below it. GC removes
//!
//! - delete operations below the frontier (every peer has applied them; their clocks become
//!   skips), and
//! - deleted units below the frontier whose deletion is below it too, except the units a later
//!   operation can still name or something held resolves through: the first deleted unit
//!   after anything that may be visible on some peer (a new insertion's right origin is the
//!   item right after a visible one) and units a mark anchors on.
//!
//! Why the rest are never needed again: a peer at or beyond the frontier inserts between a
//! visible unit and the item right after it, so the stretch a later insertion scans holds only
//! items its author lacked, which are above the frontier; a collected unit is never visible
//! and never right after a visible one. An item that names a collected unit as its origin is
//! harmless: integration compares origins by id, and an origin outside the scanned stretch
//! stops the scan whether or not it is still an item. The replica's **floor** becomes the frontier: its
//! updates carry it and a replica that does not hold all of it refuses them
//! ([`DecodeError::Compacted`](crate::DecodeError)) instead of being corrupted; operations
//! that name a collected unit, or whose stretch holds a unit below the floor (their author was
//! behind it), are refused the same way. Peers behind the floor reload from a snapshot.

mod plan;

pub(crate) use plan::Plan;

use crate::doc::Doc;
use crate::id::{ClientId, StateVector};

/// Why [`Doc::gc`] refused to collect.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GcError {
    /// No known peers: there is no frontier to collect below.
    #[error("no known peer state vectors: nothing is acknowledged")]
    NoPeers,
    /// Operations wait for dependencies. They may name what would be collected, so GC runs
    /// only between applied updates.
    #[error("operations are waiting for their dependencies")]
    Pending,
    /// A known peer holds operations of this client that this replica lacks; they may name
    /// what would be collected. Apply that peer's diff first.
    #[error("a known peer holds operations of {0:?} that this replica lacks")]
    Behind(ClientId),
}

/// What [`Doc::gc`] removed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Collected {
    /// Deleted units removed.
    pub units: u64,
    /// Delete operations removed.
    pub deletes: u64,
}

impl Doc {
    /// Collects the tombstones and delete operations that every peer in `known` (each peer's
    /// latest acknowledged state vector) has seen, as the module documentation describes, and
    /// raises this replica's GC floor to their frontier. Call it when storing a snapshot, never
    /// between computing a peer's state vector and applying the diff made for it.
    ///
    /// # Panics
    /// Never: `known` is checked to be non-empty, and a replica's own snapshot decodes.
    pub fn gc(&mut self, known: &[StateVector]) -> Result<Collected, GcError> {
        if known.is_empty() {
            return Err(GcError::NoPeers);
        }
        if self.has_pending() {
            return Err(GcError::Pending);
        }
        let held = self.state_vector();
        for sv in known {
            if let Some((client, _)) = sv.iter().find(|&(c, n)| held.get(c) < n) {
                return Err(GcError::Behind(client));
            }
        }
        let mut frontier = StateVector::empty();
        for (client, _) in held.iter() {
            let low = known.iter().map(|sv| sv.get(client)).min();
            frontier.set(client, low.expect("known is not empty"));
        }
        let plan = Plan::new(self, frontier);
        let collected = plan.collected();
        let bytes = self.snapshot_bytes(Some(&plan));
        *self =
            Self::decode_snapshot(&bytes, self.client).expect("a replica's own snapshot decodes");
        Ok(collected)
    }

    /// The GC floor: the frontier of the last collection (empty if there was none). Updates
    /// from this replica need a receiver that holds all of it.
    #[must_use]
    pub const fn gc_floor(&self) -> &StateVector {
        &self.floor
    }
}
