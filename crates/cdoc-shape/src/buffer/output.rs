//! Writing the output side of a substitution pass: copying glyphs across, replacing them,
//! and growing the run, which switches the output to storage of its own.

use super::{Buffer, GlyphInfo};

impl Buffer {
    pub(super) fn ensure(&mut self, size: usize) -> bool {
        if size > self.max_len {
            self.failed = true;
            return false;
        }
        if self.out.len() < size {
            self.out.resize(size, GlyphInfo::EMPTY);
        }
        if self.info.len() < size {
            self.info.resize(size, GlyphInfo::EMPTY);
        }
        true
    }

    pub(super) fn make_room_for(&mut self, num_in: usize, num_out: usize) -> bool {
        if !self.ensure(self.out_len + num_out) {
            return false;
        }
        if !self.separate && self.out_len + num_out > self.idx + num_in {
            self.separate = true;
            self.out[..self.out_len].copy_from_slice(&self.info[..self.out_len]);
        }
        true
    }

    pub(crate) fn next_glyph(&mut self) {
        self.next_glyphs(1);
    }

    pub(crate) fn next_glyphs(&mut self, n: usize) {
        if self.have_output {
            if self.separate || self.out_len != self.idx {
                if !self.make_room_for(n, n) {
                    return;
                }
                for i in 0..n {
                    let info = self.info[self.idx + i];
                    self.set_out(self.out_len + i, info);
                }
            }
            self.out_len += n;
        }
        self.idx += n;
    }

    pub(crate) fn skip_glyph(&mut self) {
        self.idx += 1;
    }

    /// Replaces the current glyph, keeping everything else about it.
    pub(crate) fn replace_glyph(&mut self, glyph: u16) {
        if self.separate || self.out_len != self.idx {
            if !self.make_room_for(1, 1) {
                return;
            }
            let info = self.info[self.idx];
            self.set_out(self.out_len, info);
        }
        let at = self.out_len;
        self.out_info_mut(at).glyph = glyph;
        self.idx += 1;
        self.out_len += 1;
    }

    /// Appends a copy of the current glyph (or of the last output glyph at the end) with
    /// another glyph id, without consuming the current one.
    pub(crate) fn output_glyph(&mut self, glyph: u16) {
        if !self.make_room_for(0, 1) || (self.idx == self.len && self.out_len == 0) {
            return;
        }
        let mut info = if self.idx < self.len {
            self.info[self.idx]
        } else {
            *self.out_info(self.out_len - 1)
        };
        info.glyph = glyph;
        self.set_out(self.out_len, info);
        self.out_len += 1;
    }

    pub(crate) fn output_info(&mut self, info: GlyphInfo) {
        if self.make_room_for(0, 1) {
            self.set_out(self.out_len, info);
            self.out_len += 1;
        }
    }
}
