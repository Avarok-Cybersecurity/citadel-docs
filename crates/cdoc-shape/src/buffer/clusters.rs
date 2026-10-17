//! Cluster bookkeeping: merging the clusters of glyphs that become one, moving the cursor for
//! nested lookups, deleting glyphs and reordering marks without losing which characters a
//! glyph stands for.

use super::{Buffer, GlyphInfo};

impl Buffer {
    /// Gives `info[start..end]` one cluster, the smallest, extending over neighbours that
    /// shared the clusters being changed.
    pub(crate) fn merge_clusters(&mut self, mut start: usize, mut end: usize) {
        if end - start < 2 {
            return;
        }
        let cluster = self.info[start..end]
            .iter()
            .map(|i| i.cluster)
            .min()
            .unwrap_or(0);
        if cluster != self.info[end - 1].cluster {
            while end < self.len && self.info[end - 1].cluster == self.info[end].cluster {
                end += 1;
            }
        }
        if cluster != self.info[start].cluster {
            while start > 0 && self.info[start - 1].cluster == self.info[start].cluster {
                start -= 1;
            }
        }
        if self.idx == start && self.info[start].cluster != cluster {
            let old = self.info[start].cluster;
            let mut i = self.out_len;
            while i != 0 && self.out_info(i - 1).cluster == old {
                self.out_info_mut(i - 1).cluster = cluster;
                i -= 1;
            }
        }
        for info in &mut self.info[start..end] {
            info.cluster = cluster;
        }
    }

    /// [`Self::merge_clusters`] over the output.
    pub(crate) fn merge_out_clusters(&mut self, mut start: usize, mut end: usize) {
        if end - start < 2 {
            return;
        }
        let cluster = (start..end)
            .map(|i| self.out_info(i).cluster)
            .min()
            .unwrap_or(0);
        while start != 0 && self.out_info(start - 1).cluster == self.out_info(start).cluster {
            start -= 1;
        }
        while end < self.out_len && self.out_info(end - 1).cluster == self.out_info(end).cluster {
            end += 1;
        }
        if end == self.out_len {
            let last = self.out_info(end - 1).cluster;
            let mut i = self.idx;
            while i < self.len && self.info[i].cluster == last {
                self.info[i].cluster = cluster;
                i += 1;
            }
        }
        for i in start..end {
            self.out_info_mut(i).cluster = cluster;
        }
    }

    /// Drops the current glyph, handing its cluster to a neighbour so no character is lost.
    pub(crate) fn delete_glyph(&mut self) {
        let cluster = self.info[self.idx].cluster;
        let same_next = self.idx + 1 < self.len && cluster == self.info[self.idx + 1].cluster;
        let same_prev = self.out_len != 0 && cluster == self.prev().cluster;
        if same_next || same_prev {
            self.skip_glyph();
            return;
        }
        if self.out_len != 0 {
            let old = self.prev().cluster;
            if cluster < old {
                let mut i = self.out_len;
                while i != 0 && self.out_info(i - 1).cluster == old {
                    self.out_info_mut(i - 1).cluster = cluster;
                    i -= 1;
                }
            }
            self.skip_glyph();
            return;
        }
        if self.idx + 1 < self.len {
            self.merge_clusters(self.idx, self.idx + 2);
        }
        self.skip_glyph();
    }

    /// Removes, in place and outside any pass, every glyph `drop` selects, merging its cluster
    /// into a neighbour's.
    pub(crate) fn delete_where(&mut self, drop: impl Fn(&GlyphInfo) -> bool) {
        let mut j = 0;
        for i in 0..self.len {
            if drop(&self.info[i]) {
                let cluster = self.info[i].cluster;
                if i + 1 < self.len && cluster == self.info[i + 1].cluster {
                    continue;
                }
                if j != 0 {
                    if cluster < self.info[j - 1].cluster {
                        let old = self.info[j - 1].cluster;
                        let mut k = j;
                        while k > 0 && self.info[k - 1].cluster == old {
                            self.info[k - 1].cluster = cluster;
                            k -= 1;
                        }
                    }
                    continue;
                }
                if i + 1 < self.len {
                    self.merge_clusters(i, i + 2);
                }
                continue;
            }
            self.info[j] = self.info[i];
            self.pos[j] = self.pos[i];
            j += 1;
        }
        self.len = j;
    }

    /// Moves the cursor so that `i` glyphs are behind it, for a nested lookup at a matched
    /// position.
    pub(crate) fn move_to(&mut self, i: usize) -> bool {
        if !self.have_output {
            self.idx = i;
            return true;
        }
        if self.failed {
            return false;
        }
        if self.out_len < i {
            let count = i - self.out_len;
            if !self.make_room_for(count, count) {
                return false;
            }
            for j in 0..count {
                let info = self.info[self.idx + j];
                *self.out_info_mut(self.out_len + j) = info;
            }
            self.idx += count;
            self.out_len += count;
        } else if self.out_len > i {
            let count = self.out_len - i;
            if self.idx < count && !self.shift_forward(count - self.idx) {
                return false;
            }
            self.idx -= count;
            self.out_len -= count;
            for j in 0..count {
                self.info[self.idx + j] = *self.out_info(self.out_len + j);
            }
        }
        true
    }

    fn shift_forward(&mut self, count: usize) -> bool {
        if !self.ensure(self.len + count) {
            return false;
        }
        self.info.copy_within(self.idx..self.len, self.idx + count);
        if self.idx + count > self.len {
            for info in &mut self.info[self.len..self.idx + count] {
                *info = GlyphInfo::EMPTY;
            }
        }
        self.len += count;
        self.idx += count;
        true
    }

    /// Stable insertion sort of `info[start..end]` by `key`, merging the clusters of every
    /// glyph a move jumps over.
    pub(crate) fn sort_by_key(&mut self, start: usize, end: usize, key: impl Fn(&GlyphInfo) -> u8) {
        for i in start + 1..end {
            let mut j = i;
            while j > start && key(&self.info[j - 1]) > key(&self.info[i]) {
                j -= 1;
            }
            if i == j {
                continue;
            }
            self.merge_clusters(j, i + 1);
            let moved = self.info[i];
            self.info.copy_within(j..i, j + 1);
            self.info[j] = moved;
        }
    }
}
