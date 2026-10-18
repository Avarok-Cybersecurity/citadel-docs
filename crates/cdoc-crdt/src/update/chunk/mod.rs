//! A diff as a sequence of updates, none larger than a limit, for transports that refuse large
//! messages. The operations are put in an order in which every one's dependencies come before
//! it (each client's in clock order; a cross-client origin, a mark's anchors, an attribute's
//! deps and a delete's ranges in an earlier operation or already at the receiver), then cut on
//! operation boundaries. A text, tombstone or skip run that does not fit is cut on a unit
//! boundary into chained pieces, which is how a diff starting inside a run sends it anyway.
//! So each chunk is a valid update, applied in order none waits for a later one, and the
//! chunks together integrate to what the unchunked diff does.

mod frame;

use super::decode::decode;
use super::ops::{Body, ItemOp, Op};
use crate::doc::Doc;
use crate::id::{ClientId, Id, StateVector};
use crate::item::byte_prefix;
use frame::Chunks;
use std::borrow::Cow;
use std::collections::VecDeque;

/// Why a diff cannot be cut under a limit. Nothing is ever sent over the limit.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ChunkError {
    /// The limit is below the bytes every update carries before its first operation (header,
    /// GC floor, counts).
    #[error("max_bytes {max_bytes} is below an update's {frame}-byte frame")]
    LimitBelowFrame {
        /// The limit asked for.
        max_bytes: usize,
        /// The smallest update this replica can write.
        frame: usize,
    },
    /// One operation that cannot be cut (an atom, mark, attribute write or delete, or one unit
    /// of a run) needs more than the limit as an update of its own.
    #[error(
        "the operation at {at:?} needs {bytes} bytes as an update, over max_bytes {max_bytes}"
    )]
    OversizeOperation {
        /// Its first unit.
        at: Id,
        /// The size of the smallest update that carries it.
        bytes: usize,
        /// The limit asked for.
        max_bytes: usize,
    },
    /// An operation depends on something neither the receiver nor the diff holds. An
    /// integrated replica never encodes one; this is a refusal rather than a chunk that would
    /// wait forever.
    #[error("the operation at {at:?} depends on a unit the diff does not supply")]
    Unordered {
        /// Its first unit.
        at: Id,
    },
}

impl Doc {
    /// [`Doc::encode_diff`] cut into updates of at most `max_bytes` each, to apply in order:
    /// no chunk names a unit that only a later chunk introduces, so each integrates fully when
    /// its predecessors have, and all of them integrate to what the unchunked diff does.
    /// Applied out of order or with one missing, a chunk's blocked operations wait in the
    /// buffer and [`Applied::missing`](crate::Applied) names what they lack, as for any update.
    /// An empty diff is one chunk, the update with no operations.
    ///
    /// # Errors
    /// [`ChunkError`]: a limit below the update frame, or one uncuttable operation over it.
    ///
    /// # Panics
    /// Never: the diff this replica encodes always decodes.
    pub fn encode_diff_chunked(
        &self,
        remote: &StateVector,
        max_bytes: usize,
    ) -> Result<Vec<Vec<u8>>, ChunkError> {
        let full = self.encode_diff(remote);
        let decoded = decode(&full).expect("an update this replica encodes decodes");
        let mut chunks = Chunks::new(&decoded.floor, max_bytes)?;
        let mut queues: Vec<(ClientId, VecDeque<(u64, Op<'_>)>)> = decoded
            .clients
            .into_iter()
            .map(|(client, ops)| (client, ops.into()))
            .collect();
        let mut known = remote.clone();
        loop {
            let mut progress = false;
            for (client, queue) in &mut queues {
                while let Some((_, op)) = queue.front() {
                    let held = |id: Id| known.contains(id) || decoded.floor.contains(id);
                    if op.first_missing(held).is_some() {
                        break;
                    }
                    let (clock, op) = queue.pop_front().expect("front exists");
                    let end = clock + op.len();
                    push(&mut chunks, *client, clock, op)?;
                    known.set(*client, end);
                    progress = true;
                }
            }
            if !progress {
                break;
            }
        }
        if let Some((client, queue)) = queues.iter().find(|(_, q)| !q.is_empty()) {
            let clock = queue.front().expect("not empty").0;
            return Err(ChunkError::Unordered {
                at: Id::new(*client, clock),
            });
        }
        Ok(chunks.finish())
    }
}

/// Adds `op` (first unit `clock` of `client`) to the open chunk, cutting a run across as many
/// chunks as it needs.
fn push(chunks: &mut Chunks, client: ClientId, clock: u64, op: Op<'_>) -> Result<(), ChunkError> {
    let (mut clock, mut op) = (clock, op);
    loop {
        let own = Id::new(client, clock);
        if chunks.try_add(client, clock, &op) {
            return Ok(());
        }
        if let Some(k) = largest_fitting_prefix(chunks, client, clock, &op) {
            let head = prefix(&op, k).expect("a prefix exists below the length");
            assert!(
                chunks.try_add(client, clock, &head),
                "the prefix was measured"
            );
            op = op.trim(own, k);
            clock += k;
        } else if chunks.is_open_empty() {
            return Err(ChunkError::OversizeOperation {
                at: own,
                bytes: chunks.alone(client, clock, &smallest_piece(&op)),
                max_bytes: chunks.max_bytes(),
            });
        }
        chunks.close();
    }
}

/// The most units of `op` that still fit the open chunk, when it is a run that can be cut and
/// at least one unit fits.
fn largest_fitting_prefix(
    chunks: &Chunks,
    client: ClientId,
    clock: u64,
    op: &Op<'_>,
) -> Option<u64> {
    let fits = |k: u64| prefix(op, k).is_some_and(|p| chunks.fits(client, clock, &p));
    if !fits(1) {
        return None;
    }
    let (mut lo, mut hi) = (1, op.len() - 1);
    while lo < hi {
        let mid = lo + (hi - lo).div_ceil(2);
        if fits(mid) {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    Some(lo)
}

/// The first `k` units of a run (`0 < k < len`), borrowing its text; `None` for an operation
/// that cannot be cut there.
fn prefix<'o>(op: &'o Op<'_>, k: u64) -> Option<Op<'o>> {
    if k == 0 || k >= op.len() {
        return None;
    }
    match op {
        Op::Item(item) => {
            let body = match &item.body {
                Body::Text(text) => {
                    let chars = usize::try_from(k).expect("inside a u32 run");
                    Body::Text(Cow::Borrowed(&text[..byte_prefix(text, chars)]))
                }
                Body::Gone => Body::Gone,
                Body::Atom(..) => return None,
            };
            Some(Op::Item(ItemOp {
                len: u32::try_from(k).expect("inside a u32 run"),
                origin_left: item.origin_left,
                origin_right: item.origin_right,
                body,
            }))
        }
        Op::Skip(_) => Some(Op::Skip(k)),
        Op::Mark(_) | Op::Attr(..) | Op::Delete(_) => None,
    }
}

/// What has to travel whole: one unit of a run, or the operation itself.
fn smallest_piece<'o>(op: &'o Op<'_>) -> Op<'o> {
    prefix(op, 1).unwrap_or_else(|| op.clone())
}
