//! What an applied update changed, for incremental layout.
//!
//! Blocks are counted the way the flat sequence delimits them: block `0` is everything before
//! the second visible structural atom, block `n` starts at the `n + 1`-th. A paragraph is one
//! block; a table contributes one per table, row and cell atom and one for its end. A touched
//! unit marks its block dirty; a new block boundary marks the block it split too; a deleted
//! unit marks the block it was in, which after the deletion is the block before it when the
//! unit was itself a block boundary.

use crate::doc::Doc;
use crate::id::Id;
use crate::marks::Anchor;
use crate::seq::LEAF_CAP;
use std::collections::BTreeSet;
use std::ops::Range;

/// The dirty regions of a document after [`Doc::apply_update`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Changes {
    /// Disjoint, ascending ranges of block indices whose content, formatting or properties
    /// changed.
    pub blocks: Vec<Range<usize>>,
    /// Document attribute names whose value changed (`meta`, `styles`, ...), sorted.
    pub attrs: Vec<String>,
}

impl Changes {
    /// Whether nothing changed.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty() && self.attrs.is_empty()
    }
}

/// What integration touched, before it is turned into block indices.
#[derive(Debug, Default)]
pub(crate) struct Touched {
    pub(crate) items: Vec<u32>,
    pub(crate) marks: Vec<(Anchor, Anchor)>,
    pub(crate) blocks: Vec<Id>,
    pub(crate) attrs: BTreeSet<String>,
}

impl Doc {
    fn block_of_item(&self, idx: u32) -> usize {
        let before = self.seq.weight_before(idx).blocks;
        let own = self.seq.item(idx).weight().blocks;
        to_usize((before + own).saturating_sub(1))
    }

    fn block_of_anchor(&self, anchor: Anchor) -> usize {
        match anchor.id().and_then(|id| self.item_at(id)) {
            Some((idx, _)) => self.block_of_item(idx),
            None if matches!(anchor, Anchor::Start) => 0,
            None => to_usize(self.seq.total().blocks.saturating_sub(1)),
        }
    }

    pub(crate) fn changes(&self, touched: Touched) -> Changes {
        let mut spans: Vec<Range<usize>> = Vec::new();
        // A new boundary also cuts the block it landed in short, hence `b - opens`.
        let span = |b: usize, opens: u64| b.saturating_sub(to_usize(opens))..b + 1;
        if touched.items.len() * LEAF_CAP > self.seq.item_count() {
            // Many items: one pass in document order beats a lookup per item.
            let mut hit = vec![false; self.seq.item_count()];
            for &idx in &touched.items {
                hit[idx as usize] = true;
            }
            let mut blocks = 0u64;
            for idx in self.seq.iter() {
                let opens = self.seq.item(idx).weight().blocks;
                blocks += opens;
                if hit[idx as usize] {
                    spans.push(span(to_usize(blocks.saturating_sub(1)), opens));
                }
            }
        } else {
            for &idx in &touched.items {
                let opens = self.seq.item(idx).weight().blocks;
                spans.push(span(self.block_of_item(idx), opens));
            }
        }
        for id in &touched.blocks {
            if let Some((idx, _)) = self.item_at(*id) {
                let b = self.block_of_item(idx);
                spans.push(b..b + 1);
            }
        }
        for &(start, end) in &touched.marks {
            let (a, b) = (self.block_of_anchor(start), self.block_of_anchor(end));
            spans.push(a.min(b)..a.max(b) + 1);
        }
        spans.sort_by_key(|r| r.start);
        let mut blocks: Vec<Range<usize>> = Vec::new();
        for span in spans {
            match blocks.last_mut() {
                Some(last) if span.start <= last.end => last.end = last.end.max(span.end),
                _ => blocks.push(span),
            }
        }
        Changes {
            blocks,
            attrs: touched.attrs.into_iter().collect(),
        }
    }
}

fn to_usize(n: u64) -> usize {
    usize::try_from(n).expect("fits the address space")
}
