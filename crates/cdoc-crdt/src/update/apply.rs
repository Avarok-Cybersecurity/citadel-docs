//! Applying an update: integrate what can be, buffer what waits for a dependency, report what
//! is missing and what changed.

use super::blank::{is_base, NOT_BASE};
use super::decode::decode;
use super::ops::Op;
use super::pending::PendingLimits;
use crate::bytes::DecodeError;
use crate::changes::{Changes, Touched};
use crate::doc::Doc;
use crate::id::{ClientId, Id};
use std::collections::{BTreeMap, VecDeque};

/// The outcome of [`Doc::apply_update`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Applied {
    /// What changed.
    pub changes: Changes,
    /// Units the buffered remainder waits for, one per blocked client; empty when everything
    /// was integrated. Ask a peer for a diff against this replica's state vector to fill them.
    pub missing: Vec<Id>,
    /// Buffered operations dropped to stay within the [`PendingLimits`]. They are not held,
    /// so a later diff against this replica's state vector sends them again.
    pub dropped: u64,
}

type Queue<'a> = VecDeque<(u64, Op<'a>)>;

impl Doc {
    /// Applies an update (or a snapshot's diff). Operations already held are ignored, so
    /// applying twice changes nothing; operations whose dependencies are absent wait in a
    /// buffer and are integrated by a later update that supplies them. The buffer is kept
    /// within `limits`; what is over them is dropped and counted in [`Applied::dropped`].
    /// Operations claiming client 0 ([`ClientId::BLANK`]) are refused unless they are the
    /// blank document's own.
    ///
    /// An update that fails to decode, or that claims client 0 for anything but the base,
    /// changes nothing. One that decodes but contradicts this replica (an origin that is not
    /// an item) is an error, and what was integrated before the contradiction stays, as does
    /// the buffer, within `limits`.
    ///
    /// # Panics
    /// Never: an operation stays queued only when it lacks a clock or a dependency.
    pub fn apply_update(
        &mut self,
        bytes: &[u8],
        limits: PendingLimits,
    ) -> Result<Applied, DecodeError> {
        let decoded = decode(bytes)?;
        if !self.state_vector().covers(&decoded.floor) {
            return Err(DecodeError::Compacted);
        }
        // Refused before anything is integrated or buffered: the update changes nothing.
        let forged = decoded
            .clients
            .iter()
            .filter(|(client, _)| *client == ClientId::BLANK)
            .flat_map(|(_, ops)| ops)
            .any(|(clock, op)| !is_base(*clock, op));
        if forged {
            return Err(DecodeError::Inconsistent(NOT_BASE));
        }
        let mut queues: BTreeMap<ClientId, Queue<'_>> = BTreeMap::new();
        for (client, ops) in std::mem::take(&mut self.pending.ops) {
            queues.entry(client).or_default().extend(ops);
        }
        for (client, ops) in decoded.clients {
            let queue = queues.entry(client).or_default();
            let merge = !queue.is_empty();
            queue.extend(ops);
            if merge {
                queue.make_contiguous().sort_by_key(|(clock, _)| *clock);
            }
        }
        let mut touched = Touched::default();
        let mut failure = None;
        'rounds: loop {
            let mut progress = false;
            for (&client, queue) in &mut queues {
                match self.drain(client, queue, &mut touched) {
                    Ok(p) => progress |= p,
                    Err(e) => {
                        failure = Some(e);
                        break 'rounds;
                    }
                }
            }
            if !progress {
                break;
            }
        }
        for (client, queue) in queues {
            if !queue.is_empty() {
                let held = self.pending.ops.entry(client).or_default();
                held.extend(queue.into_iter().map(|(c, op)| (c, op.into_owned())));
            }
        }
        let dropped = self.pending.enforce(limits);
        if let Some(e) = failure {
            return Err(e);
        }
        let mut missing = Vec::new();
        for (&client, queue) in &self.pending.ops {
            if let Some((&clock, op)) = queue.first_key_value() {
                let next = self.next_clock(client);
                missing.push(if clock > next {
                    Id::new(client, next)
                } else {
                    self.missing_dep(op)
                        .expect("a blocked operation lacks something")
                });
            }
        }
        missing.sort_unstable();
        missing.dedup();
        Ok(Applied {
            changes: self.changes(touched),
            missing,
            dropped,
        })
    }

    /// Integrates `client`'s queued operations while they are next in line and ready.
    fn drain(
        &mut self,
        client: ClientId,
        queue: &mut Queue<'_>,
        touched: &mut Touched,
    ) -> Result<bool, DecodeError> {
        let mut progress = false;
        while let Some((clock, op)) = queue.front() {
            let next = self.next_clock(client);
            let end = clock + op.len();
            if end <= next {
                queue.pop_front();
                continue;
            }
            if *clock > next || self.missing_dep(op).is_some() {
                break;
            }
            let (clock, op) = queue.pop_front().expect("front exists");
            let op = if clock < next {
                op.trim(Id::new(client, clock), next - clock)
            } else {
                op
            };
            self.integrate(Id::new(client, next), op, touched)?;
            progress = true;
        }
        Ok(progress)
    }
}
