//! What a run's glyphs are predicted from: the character each glyph's cluster starts with, and
//! a face's char table mapping characters to glyph-table indices. The writer and the reader
//! share these functions, so a prediction cannot differ between them.

/// No glyph-table index for a character.
const NONE: u32 = u32::MAX;

/// A face's characters → glyph-table indices: a direct table for ASCII, a sorted list for
/// the rest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharTable {
    ascii: [u32; 128],
    other: Vec<(u32, u32)>,
}

impl Default for CharTable {
    fn default() -> Self {
        Self {
            ascii: [NONE; 128],
            other: Vec::new(),
        }
    }
}

impl CharTable {
    /// Adds a character; entries arrive in increasing code-point order.
    pub fn push(&mut self, code_point: u32, index: u32) {
        match self.ascii.get_mut(code_point as usize) {
            Some(slot) => *slot = index,
            None => self.other.push((code_point, index)),
        }
    }

    /// The glyph-table index predicted for `code_point`.
    #[inline]
    pub fn get(&self, code_point: u32) -> Option<u32> {
        match self.ascii.get(code_point as usize) {
            Some(&NONE) => None,
            Some(&index) => Some(index),
            None => self
                .other
                .binary_search_by_key(&code_point, |e| e.0)
                .ok()
                .map(|at| self.other[at].1),
        }
    }

    /// The index predicted for `code_point`: its entry, or 0 when it has none.
    #[inline]
    pub fn predict(&self, code_point: u32) -> u32 {
        self.get(code_point).unwrap_or(0)
    }

    /// Every entry, in increasing code-point order.
    pub fn entries(&self) -> impl Iterator<Item = (u32, u32)> + '_ {
        (0_u32..)
            .zip(self.ascii)
            .filter(|e| e.1 != NONE)
            .chain(self.other.iter().copied())
    }
}

/// The character starting at byte `at` of `text` and its length in bytes, or `None` at the
/// end of the text or off a character boundary.
#[inline]
pub fn char_at(text: &str, at: usize) -> Option<(u32, u32)> {
    match text.as_bytes().get(at) {
        Some(&b) if b < 0x80 => Some((u32::from(b), 1)),
        Some(_) => {
            let c = text.get(at..)?.chars().next()?;
            #[expect(clippy::cast_possible_truncation, reason = "a char is at most 4 bytes")]
            Some((u32::from(c), c.len_utf8() as u32))
        }
        None => None,
    }
}

/// Where the glyph after one at `cluster` is predicted to start: one character on (or at the
/// same place at the end of the text).
#[inline]
pub const fn after(cluster: u32, width: u32) -> u32 {
    cluster.saturating_add(width)
}

/// The glyph-table index predicted for the character `ch` at a cluster (`char_at`), and that
/// character's width: the char table's entry, or index 0 when it has none (or at the end).
#[inline]
pub fn predicted(table: &CharTable, ch: Option<(u32, u32)>) -> (u32, u32) {
    match ch {
        Some((cp, width)) => (table.predict(cp), width),
        None => (0, 0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_char_table_answers_ascii_and_the_rest_and_lists_both_in_order() {
        let mut t = CharTable::default();
        for (cp, index) in [(0x41, 3), (0x7F, 0), (0xE9, 7), (0x1F600, 2)] {
            t.push(cp, index);
        }
        assert_eq!(
            [0x41, 0x7F, 0xE9, 0x1F600, 0x42, 0xEA].map(|c| t.get(c)),
            [Some(3), Some(0), Some(7), Some(2), None, None]
        );
        let all: Vec<_> = t.entries().collect();
        assert_eq!(all, [(0x41, 3), (0x7F, 0), (0xE9, 7), (0x1F600, 2)]);
    }

    #[test]
    fn char_at_reads_any_width_and_refuses_the_end_and_a_continuation_byte() {
        let text = "aé😀";
        assert_eq!(char_at(text, 0), Some((0x61, 1)));
        assert_eq!(char_at(text, 1), Some((0xE9, 2)));
        assert_eq!(char_at(text, 3), Some((0x1F600, 4)));
        assert_eq!(char_at(text, 2), None);
        assert_eq!(char_at(text, 7), None);
    }
}
