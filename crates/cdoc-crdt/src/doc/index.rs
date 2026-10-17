//! A client's history by clock: a sorted list split into chunks of at most [`CHUNK`] entries,
//! with each chunk's first clock in a separate array. A lookup is two binary searches over
//! contiguous memory; an insertion moves at most one chunk. New operations append to the last
//! chunk; splits of old runs insert in the middle, which a single sorted vector would pay for
//! with a move of the whole history and `BTreeMap` with pointer-chasing on every lookup.

use super::Entry;

const CHUNK: usize = 128;

#[derive(Debug, Clone)]
pub(crate) struct ClockIndex {
    chunks: Vec<Vec<(u64, Entry)>>,
    firsts: Vec<u64>,
}

impl ClockIndex {
    pub(crate) const fn new() -> Self {
        Self {
            chunks: Vec::new(),
            firsts: Vec::new(),
        }
    }

    /// From entries already sorted by clock, without duplicates.
    pub(crate) fn from_sorted(entries: &[(u64, Entry)]) -> Self {
        let chunks: Vec<Vec<(u64, Entry)>> = entries
            .chunks(CHUNK / 2)
            .map(<[(u64, Entry)]>::to_vec)
            .collect();
        let firsts = chunks.iter().map(|c| c[0].0).collect();
        Self { chunks, firsts }
    }

    /// The entry with the greatest clock at or before `clock`.
    pub(crate) fn floor(&self, clock: u64) -> Option<(u64, Entry)> {
        let c = self
            .firsts
            .partition_point(|&f| f <= clock)
            .checked_sub(1)?;
        let chunk = &self.chunks[c];
        let k = chunk.partition_point(|e| e.0 <= clock);
        Some(chunk[k - 1])
    }

    /// Adds an entry, replacing one at the same clock.
    pub(crate) fn insert(&mut self, clock: u64, entry: Entry) {
        let c = self
            .firsts
            .partition_point(|&f| f <= clock)
            .saturating_sub(1);
        let Some(chunk) = self.chunks.get_mut(c) else {
            self.chunks.push(vec![(clock, entry)]);
            self.firsts.push(clock);
            return;
        };
        let k = chunk.partition_point(|e| e.0 < clock);
        match chunk.get_mut(k) {
            Some(slot) if slot.0 == clock => {
                slot.1 = entry;
                return;
            }
            _ => chunk.insert(k, (clock, entry)),
        }
        self.firsts[c] = chunk[0].0;
        if chunk.len() > CHUNK {
            let tail = chunk.split_off(CHUNK / 2);
            self.firsts.insert(c + 1, tail[0].0);
            self.chunks.insert(c + 1, tail);
        }
    }

    /// Every entry in clock order.
    pub(crate) fn iter(&self) -> impl Iterator<Item = (u64, Entry)> + '_ {
        self.chunks.iter().flatten().copied()
    }

    /// Every entry from the one starting at `clock` (or the first after it) on.
    pub(crate) fn iter_from(&self, clock: u64) -> impl Iterator<Item = (u64, Entry)> + '_ {
        let c = self
            .firsts
            .partition_point(|&f| f <= clock)
            .saturating_sub(1);
        let k = self
            .chunks
            .get(c)
            .map_or(0, |chunk| chunk.partition_point(|e| e.0 < clock));
        self.chunks
            .iter()
            .skip(c)
            .enumerate()
            .flat_map(move |(i, chunk)| chunk[if i == 0 { k } else { 0 }..].iter().copied())
    }
}
