//! Identity: who wrote an operation and where it sits in that writer's history.

use crate::bytes::{DecodeError, Reader, Writer};
use std::collections::BTreeMap;

/// A replica. Supplied by the caller (the editor mints one per session through its id port);
/// two replicas editing at once must never share one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClientId(pub u64);

/// One unit of one operation: the `clock`-th unit `client` produced. Every character, atom,
/// mark and attribute write consumes clocks, so a client's history is the contiguous range
/// `0..next`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Id {
    /// The writer.
    pub client: ClientId,
    /// The position in the writer's history.
    pub clock: u64,
}

impl Id {
    /// The id of `client`'s `clock`-th unit.
    #[must_use]
    pub const fn new(client: ClientId, clock: u64) -> Self {
        Self { client, clock }
    }

    pub(crate) const fn offset(self, by: u64) -> Self {
        Self::new(self.client, self.clock + by)
    }
}

/// How much of each client's history a replica holds: for every client, the clock of the
/// first unit it does **not** have. Missing clients have seen nothing.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StateVector(BTreeMap<ClientId, u64>);

/// A version of a document is the state vector of a replica that holds exactly it.
pub type Version = StateVector;

impl StateVector {
    /// Nothing from anyone: the version of an empty document.
    #[must_use]
    pub const fn empty() -> Self {
        Self(BTreeMap::new())
    }

    /// The first clock of `client` not covered.
    #[must_use]
    pub fn get(&self, client: ClientId) -> u64 {
        self.0.get(&client).copied().unwrap_or(0)
    }

    /// Whether the unit `id` is covered.
    #[must_use]
    pub fn contains(&self, id: Id) -> bool {
        id.clock < self.get(id.client)
    }

    /// Whether every unit `other` covers, this covers too.
    #[must_use]
    pub fn covers(&self, other: &Self) -> bool {
        other.0.iter().all(|(&c, &n)| self.get(c) >= n)
    }

    /// Clients and their next clocks, in client order.
    pub fn iter(&self) -> impl Iterator<Item = (ClientId, u64)> + '_ {
        self.0.iter().map(|(&c, &n)| (c, n))
    }

    pub(crate) fn set(&mut self, client: ClientId, next: u64) {
        if next == 0 {
            self.0.remove(&client);
        } else {
            self.0.insert(client, next);
        }
    }

    /// The binary form: a count, then `(client, next)` varint pairs in client order.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer::new();
        self.write(&mut w);
        w.finish()
    }

    /// Reads [`encode`](Self::encode)'s output.
    pub fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        let mut r = Reader::new(bytes);
        let sv = Self::read(&mut r)?;
        r.finish()?;
        Ok(sv)
    }

    pub(crate) fn write(&self, w: &mut Writer) {
        w.usize(self.0.len());
        for (client, next) in self.iter() {
            w.uvar(client.0);
            w.uvar(next);
        }
    }

    pub(crate) fn read(r: &mut Reader<'_>) -> Result<Self, DecodeError> {
        let n = r.usize("state vector length")?;
        let mut map = BTreeMap::new();
        let mut last: Option<u64> = None;
        for _ in 0..n {
            let client = r.uvar()?;
            let next = r.uvar()?;
            if last.is_some_and(|l| l >= client) || next == 0 {
                return Err(DecodeError::Inconsistent("state vector not canonical"));
            }
            last = Some(client);
            map.insert(ClientId(client), next);
        }
        Ok(Self(map))
    }
}
