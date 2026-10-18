//! What a collection removes, decided in one pass over the sequence (see the module above).

use crate::doc::{Doc, Entry};
use crate::id::{ClientId, Id, StateVector};
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// Sorted, disjoint, non-touching clock ranges.
type Ranges = Vec<(u64, u64)>;

fn normalise(mut r: Ranges) -> Ranges {
    r.sort_unstable();
    let mut out: Ranges = Vec::with_capacity(r.len());
    for (s, e) in r {
        match out.last_mut() {
            Some(last) if last.1 >= s => last.1 = last.1.max(e),
            _ => out.push((s, e)),
        }
    }
    out
}

/// `(s, e)` without what `minus` covers.
fn subtract(s: u64, e: u64, minus: &[(u64, u64)]) -> Ranges {
    let mut out = Vec::new();
    let mut cur = s;
    let from = minus.partition_point(|&(_, me)| me <= s);
    for &(ms, me) in &minus[from..] {
        if ms >= e {
            break;
        }
        if ms > cur {
            out.push((cur, ms));
        }
        cur = cur.max(me);
    }
    if cur < e {
        out.push((cur, e));
    }
    out
}

/// A collection: what goes, and the floor that results.
pub(crate) struct Plan {
    /// Deleted items that lose units: the unit ranges (offsets in the item) they keep.
    pub(crate) kept: HashMap<u32, Ranges>,
    /// Delete operations below the frontier are removed.
    pub(crate) frontier: StateVector,
    /// The new floor.
    pub(crate) floor: StateVector,
    units: u64,
    deletes: u64,
}

impl Plan {
    pub(crate) fn new(doc: &Doc, frontier: StateVector) -> Self {
        // Units deleted only by operations some peer has not seen yet.
        let mut all: BTreeMap<ClientId, Ranges> = BTreeMap::new();
        let mut acked: BTreeMap<ClientId, Ranges> = BTreeMap::new();
        let mut deletes = 0;
        for (&client, log) in &doc.logs {
            for (clock, entry) in log.entries.iter() {
                let Entry::Delete(k) = entry else { continue };
                let seen = clock < frontier.get(client);
                deletes += u64::from(seen);
                for r in doc.delete_op(k) {
                    all.entry(r.client).or_default().push((r.start, r.end));
                    if seen {
                        acked.entry(r.client).or_default().push((r.start, r.end));
                    }
                }
            }
        }
        let unseen: BTreeMap<ClientId, Ranges> = all
            .into_iter()
            .map(|(c, r)| {
                let seen = normalise(acked.remove(&c).unwrap_or_default());
                let r = normalise(r)
                    .into_iter()
                    .flat_map(|(s, e)| subtract(s, e, &seen))
                    .collect();
                (c, r)
            })
            .collect();
        let pinned = pinned(doc);
        let mut kept = HashMap::new();
        let mut units = 0;
        let mut prev_candidate = false;
        for idx in doc.seq.iter() {
            let item = doc.seq.item(idx);
            let (client, s, len) = (item.id.client, item.id.clock, u64::from(item.len));
            let limit = frontier.get(client).min(s + len);
            if !item.is_deleted() || limit <= s {
                prev_candidate = false;
                continue;
            }
            let none = Vec::new();
            let candidates = subtract(s, limit, unseen.get(&client).unwrap_or(&none));
            let mut collected: Ranges = Vec::new();
            for (cs, ce) in candidates.iter().copied() {
                // The first deleted unit after a unit that may be visible somewhere stays.
                let first_stays = cs > s || !prev_candidate;
                let cs = cs + u64::from(first_stays);
                let pins: Ranges = pinned
                    .get(&client)
                    .into_iter()
                    .flat_map(|p| p.range(cs..ce).map(|&c| (c, c + 1)))
                    .collect();
                collected.extend(subtract(cs, ce, &pins));
            }
            prev_candidate = candidates.last().is_some_and(|&(_, ce)| ce == s + len);
            if collected.is_empty() {
                continue;
            }
            units += collected.iter().map(|(a, b)| b - a).sum::<u64>();
            let keep = subtract(s, s + len, &collected);
            kept.insert(idx, keep.into_iter().map(|(a, b)| (a - s, b - s)).collect());
        }
        let mut floor = doc.floor.clone();
        for (client, next) in frontier.iter() {
            floor.set(client, next.max(floor.get(client)));
        }
        Self {
            kept,
            frontier,
            floor,
            units,
            deletes,
        }
    }

    /// What this collection removes, with the `pending` operations dropped alongside it.
    pub(crate) const fn collected(&self, pending: u64) -> super::Collected {
        super::Collected {
            units: self.units,
            deletes: self.deletes,
            pending,
        }
    }

    /// Whether the operation at `id` is a delete this collection removes.
    pub(crate) fn drops_delete(&self, id: Id) -> bool {
        self.frontier.contains(id)
    }
}

/// Units that must stay because a held mark anchors on them: formatting resolves its ranges by
/// the anchors' positions.
fn pinned(doc: &Doc) -> BTreeMap<ClientId, BTreeSet<u64>> {
    let mut out: BTreeMap<ClientId, BTreeSet<u64>> = BTreeMap::new();
    for m in &doc.marks {
        for id in m.start.id().into_iter().chain(m.end.id()) {
            out.entry(id.client).or_default().insert(id.clock);
        }
    }
    out
}
