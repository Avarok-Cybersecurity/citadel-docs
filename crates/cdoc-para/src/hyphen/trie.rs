//! Liang's pattern trie, packed: each node's edges are a sorted run of one flat array, and each
//! node that ends a pattern points at that pattern's digits in a shared pool.

use std::collections::BTreeMap;

/// One pattern: its letters (with `.` for a word edge) and the digit before, between and
/// after them (`letters.len() + 1` values).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Pattern {
    pub letters: Vec<char>,
    pub values: Vec<u8>,
}

/// The packed trie.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Trie {
    /// Node `i`'s edges are `edge_char[first[i]..first[i + 1]]`, sorted.
    first: Vec<u32>,
    edge_char: Vec<char>,
    edge_child: Vec<u32>,
    /// Node `i`'s pattern digits are `pool[values[i].0..][..values[i].1]`; length 0 is none.
    values: Vec<(u32, u8)>,
    pool: Vec<u8>,
}

#[derive(Default)]
struct Building {
    children: BTreeMap<char, usize>,
    values: Option<Vec<u8>>,
}

fn to_u32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

impl Trie {
    /// Builds the trie. Returns the index of the first pattern whose letters repeat an earlier
    /// pattern's, if any.
    pub fn build(patterns: &[Pattern]) -> Result<Self, usize> {
        let mut nodes = vec![Building::default()];
        for (index, p) in patterns.iter().enumerate() {
            let mut at = 0;
            for &c in &p.letters {
                let fresh = nodes.len();
                at = if let Some(&child) = nodes[at].children.get(&c) {
                    child
                } else {
                    nodes[at].children.insert(c, fresh);
                    nodes.push(Building::default());
                    fresh
                };
            }
            if nodes[at].values.replace(p.values.clone()).is_some() {
                return Err(index);
            }
        }
        let mut trie = Trie {
            first: Vec::with_capacity(nodes.len() + 1),
            edge_char: Vec::with_capacity(nodes.len()),
            edge_child: Vec::with_capacity(nodes.len()),
            values: Vec::with_capacity(nodes.len()),
            pool: Vec::new(),
        };
        for node in &nodes {
            trie.first.push(to_u32(trie.edge_char.len()));
            for (&c, &child) in &node.children {
                trie.edge_char.push(c);
                trie.edge_child.push(to_u32(child));
            }
            trie.values.push(match &node.values {
                Some(v) => {
                    let at = to_u32(trie.pool.len());
                    trie.pool.extend_from_slice(v);
                    (at, u8::try_from(v.len()).unwrap_or(u8::MAX))
                }
                None => (0, 0),
            });
        }
        trie.first.push(to_u32(trie.edge_char.len()));
        Ok(trie)
    }

    fn child(&self, node: usize, c: char) -> Option<usize> {
        let lo = self.first[node] as usize;
        let hi = self.first[node + 1] as usize;
        let edges = &self.edge_char[lo..hi];
        let k = if edges.len() <= 8 {
            edges.iter().position(|&e| e == c)?
        } else {
            edges.binary_search(&c).ok()?
        };
        Some(self.edge_child[lo + k] as usize)
    }

    /// Raises `vals[start + j]` to every matching pattern's digit `j`, for each pattern that
    /// matches `text` from its beginning.
    pub fn apply(&self, text: &[char], start: usize, vals: &mut [u8]) {
        let mut node = 0;
        for &c in &text[start..] {
            let Some(next) = self.child(node, c) else {
                return;
            };
            node = next;
            let (at, len) = self.values[node];
            if len > 0 {
                let digits = &self.pool[at as usize..at as usize + usize::from(len)];
                for (v, &d) in vals[start..].iter_mut().zip(digits) {
                    *v = (*v).max(d);
                }
            }
        }
    }
}
