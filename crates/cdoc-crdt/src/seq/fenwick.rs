//! A Fenwick (binary indexed) tree of [`Weight`]s over the leaves of the sequence, in document
//! order: prefix sums and "which leaf holds visible unit `n`" in `O(log leaves)`.

use crate::item::Weight;

#[derive(Debug, Clone)]
pub(crate) struct Fenwick {
    /// One-based; `tree[0]` is unused.
    tree: Vec<Weight>,
}

const fn low_bit(i: usize) -> usize {
    i.isolate_lowest_one()
}

impl Fenwick {
    pub(crate) fn build(values: impl ExactSizeIterator<Item = Weight>) -> Self {
        let n = values.len();
        let mut tree = Vec::with_capacity(n + 1);
        tree.push(Weight::ZERO);
        tree.extend(values);
        for i in 1..=n {
            let j = i + low_bit(i);
            if j <= n {
                tree[j] = tree[j].plus(tree[i]);
            }
        }
        Self { tree }
    }

    /// Appends a position at the end in `O(log n)`.
    pub(crate) fn push(&mut self, w: Weight) {
        let i = self.tree.len();
        let value = w
            .plus(self.prefix(i - 1))
            .minus(self.prefix(i - low_bit(i)));
        self.tree.push(value);
    }

    /// Adds `plus` and subtracts `minus` at the 0-based position `pos`.
    pub(crate) fn adjust(&mut self, pos: usize, plus: Weight, minus: Weight) {
        let mut i = pos + 1;
        while i < self.tree.len() {
            self.tree[i] = self.tree[i].plus(plus).minus(minus);
            i += low_bit(i);
        }
    }

    /// The sum of the first `count` positions.
    pub(crate) fn prefix(&self, count: usize) -> Weight {
        let mut sum = Weight::ZERO;
        let mut i = count;
        while i > 0 {
            sum = sum.plus(self.tree[i]);
            i -= low_bit(i);
        }
        sum
    }

    /// The first position whose running `len` total exceeds `target`, with the `len` before it;
    /// `None` when the whole tree holds `target` or fewer units.
    pub(crate) fn search(&self, target: u64) -> Option<(usize, u64)> {
        let n = self.tree.len() - 1;
        let mut pos = 0;
        let mut rem = target;
        let mut step = if n == 0 { 0 } else { 1 << n.ilog2() };
        while step > 0 {
            let next = pos + step;
            if next <= n && self.tree[next].len <= rem {
                pos = next;
                rem -= self.tree[next].len;
            }
            step >>= 1;
        }
        (pos < n).then_some((pos, target - rem))
    }
}
