//! The sequence: every item (tombstones included) in document order, indexed for position.
//!
//! Items live in an arena (`items`, stable indices). Document order is a list of *leaves* of at
//! most [`LEAF_CAP`] item indices; a [`Fenwick`] tree over the leaves holds each leaf's visible
//! weight. So "the item at visible index `n`" is a tree search plus a scan of one leaf, and
//! "the visible index of this item" is a prefix sum plus a scan of one leaf: `O(log n)` with a
//! small constant, against the `O(n)` walk of a plain linked list. Splitting a full leaf
//! rebuilds the leaf-position table and the tree in `O(leaves)`, once per `LEAF_CAP / 2`
//! insertions.

mod fenwick;

use crate::item::{Item, Weight};
use fenwick::Fenwick;

/// Items per leaf before it splits in two.
pub(crate) const LEAF_CAP: usize = 64;

#[derive(Debug, Clone)]
struct Leaf {
    items: Vec<u32>,
    weight: Weight,
}

#[derive(Debug, Clone)]
pub(crate) struct Seq {
    items: Vec<Item>,
    leaves: Vec<Leaf>,
    /// Leaf ids in document order.
    order: Vec<u32>,
    /// For each leaf id, its position in `order`.
    pos: Vec<u32>,
    fen: Fenwick,
}

fn to_u32(n: usize) -> u32 {
    u32::try_from(n).expect("the sequence holds fewer than 2^32 items")
}

impl Seq {
    pub(crate) fn new() -> Self {
        Self {
            items: Vec::new(),
            leaves: vec![Leaf {
                items: Vec::new(),
                weight: Weight::ZERO,
            }],
            order: vec![0],
            pos: vec![0],
            fen: Fenwick::build([Weight::ZERO].into_iter()),
        }
    }

    pub(crate) fn item(&self, idx: u32) -> &Item {
        &self.items[idx as usize]
    }

    pub(crate) fn item_count(&self) -> usize {
        self.items.len()
    }

    /// The visible weight of the whole sequence.
    pub(crate) fn total(&self) -> Weight {
        self.fen.prefix(self.order.len())
    }

    /// Every item index in document order.
    pub(crate) fn iter(&self) -> impl Iterator<Item = u32> + '_ {
        self.order
            .iter()
            .flat_map(|&l| self.leaves[l as usize].items.iter().copied())
    }

    fn locate(&self, idx: u32) -> (usize, usize) {
        let leaf = self.items[idx as usize].leaf as usize;
        let at = self.leaves[leaf]
            .items
            .iter()
            .position(|&i| i == idx)
            .expect("an item is in the leaf it names");
        (leaf, at)
    }

    pub(crate) fn first(&self) -> Option<u32> {
        self.iter().next()
    }

    pub(crate) fn next(&self, idx: u32) -> Option<u32> {
        let (leaf, at) = self.locate(idx);
        if let Some(&n) = self.leaves[leaf].items.get(at + 1) {
            return Some(n);
        }
        let start = self.pos[leaf] as usize + 1;
        self.order[start..]
            .iter()
            .find_map(|&l| self.leaves[l as usize].items.first().copied())
    }

    /// Room for `n` more items, for loading.
    pub(crate) fn reserve(&mut self, n: usize) {
        self.items.reserve(n);
    }

    /// Appends at the end of the document, for loading in order.
    pub(crate) fn push(&mut self, item: Item) -> u32 {
        let mut last = *self.order.last().expect("never empty") as usize;
        if self.leaves[last].items.len() >= LEAF_CAP {
            last = self.leaves.len();
            self.leaves.push(Leaf {
                // Loading fills it to the cap: one allocation instead of a doubling series.
                items: Vec::with_capacity(LEAF_CAP),
                weight: Weight::ZERO,
            });
            self.pos.push(to_u32(self.order.len()));
            self.order.push(to_u32(last));
            self.fen.push(Weight::ZERO);
        }
        self.place(last, self.leaves[last].items.len(), item)
    }

    /// Inserts directly after `left`, or at the very start.
    pub(crate) fn insert_after(&mut self, left: Option<u32>, item: Item) -> u32 {
        let (leaf, at) = match left {
            Some(l) => {
                let (leaf, at) = self.locate(l);
                (leaf, at + 1)
            }
            None => (self.order[0] as usize, 0),
        };
        let idx = self.place(leaf, at, item);
        if self.leaves[leaf].items.len() > LEAF_CAP {
            self.split_leaf(leaf);
        }
        idx
    }

    fn place(&mut self, leaf: usize, at: usize, mut item: Item) -> u32 {
        let idx = to_u32(self.items.len());
        item.leaf = to_u32(leaf);
        let w = item.weight();
        self.items.push(item);
        let l = &mut self.leaves[leaf];
        l.items.insert(at, idx);
        l.weight = l.weight.plus(w);
        self.fen.adjust(self.pos[leaf] as usize, w, Weight::ZERO);
        idx
    }

    fn split_leaf(&mut self, leaf: usize) {
        let half = self.leaves[leaf].items.len() / 2;
        let moved = self.leaves[leaf].items.split_off(half);
        let new_id = self.leaves.len();
        let mut weight = Weight::ZERO;
        for &i in &moved {
            let item = &mut self.items[i as usize];
            item.leaf = to_u32(new_id);
            weight = weight.plus(item.weight());
        }
        let old = &mut self.leaves[leaf];
        old.weight = old.weight.minus(weight);
        self.leaves.push(Leaf {
            items: moved,
            weight,
        });
        let at = self.pos[leaf] as usize + 1;
        self.order.insert(at, to_u32(new_id));
        self.pos.push(0);
        for (p, &l) in self.order.iter().enumerate().skip(at) {
            self.pos[l as usize] = to_u32(p);
        }
        let leaves = &self.leaves;
        self.fen = Fenwick::build(self.order.iter().map(|&l| leaves[l as usize].weight));
    }

    /// The item holding visible unit `index`, and the unit's offset in it.
    pub(crate) fn find(&self, index: u64) -> Option<(u32, u64)> {
        let (p, before) = self.fen.search(index)?;
        let mut rem = index - before;
        for &i in &self.leaves[self.order[p] as usize].items {
            let len = self.items[i as usize].weight().len;
            if rem < len {
                return Some((i, rem));
            }
            rem -= len;
        }
        None
    }

    /// The visible weight of everything before `idx`.
    pub(crate) fn weight_before(&self, idx: u32) -> Weight {
        let (leaf, at) = self.locate(idx);
        let mut w = self.fen.prefix(self.pos[leaf] as usize);
        for &i in &self.leaves[leaf].items[..at] {
            w = w.plus(self.items[i as usize].weight());
        }
        w
    }

    /// Changes an item in place, keeping the weights right.
    pub(crate) fn modify(&mut self, idx: u32, f: impl FnOnce(&mut Item)) {
        let item = &mut self.items[idx as usize];
        let before = item.weight();
        f(item);
        let after = item.weight();
        if before != after {
            let leaf = item.leaf as usize;
            let l = &mut self.leaves[leaf];
            l.weight = l.weight.plus(after).minus(before);
            self.fen.adjust(self.pos[leaf] as usize, after, before);
        }
    }
}
