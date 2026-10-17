//! The run being shaped: an input array and, while a substitution pass runs, an output array
//! that shares the input's storage until a lookup makes the run longer.
//!
//! Lookups read ahead in the input (`info[idx..len]`) and behind in the output
//! (`out[..out_len]`); `sync` makes the output the next pass's input. Positioning works in
//! place.

mod clusters;
mod info;
mod output;

pub(crate) use info::{GlyphInfo, GlyphPos, ATTACH_CURSIVE, ATTACH_MARK};

/// How far a run may grow, and how many nested lookups it may run, per input glyph.
const MAX_LEN_FACTOR: usize = 64;
const MAX_LEN_MIN: usize = 16384;
const MAX_OPS_FACTOR: i64 = 1024;
const MAX_OPS_MIN: i64 = 16384;

#[expect(
    clippy::struct_excessive_bools,
    reason = "independent per-run facts read at separate stages; packing them would only hide them"
)]
pub(crate) struct Buffer {
    pub(crate) info: Vec<GlyphInfo>,
    out: Vec<GlyphInfo>,
    pub(crate) pos: Vec<GlyphPos>,
    pub(crate) len: usize,
    pub(crate) idx: usize,
    pub(crate) out_len: usize,
    pub(crate) have_output: bool,
    separate: bool,
    serial: u8,
    pub(crate) max_ops: i64,
    max_len: usize,
    /// The run outgrew its budget; the result is discarded with an error.
    pub(crate) failed: bool,
    pub(crate) has_attachment: bool,
    pub(crate) has_ignorables: bool,
    pub(crate) has_space_fallback: bool,
    pub(crate) non_ascii: bool,
    pub(crate) has_cgj: bool,
}

impl Buffer {
    pub(crate) fn new() -> Self {
        Self {
            info: Vec::new(),
            out: Vec::new(),
            pos: Vec::new(),
            len: 0,
            idx: 0,
            out_len: 0,
            have_output: false,
            separate: false,
            serial: 0,
            max_ops: 0,
            max_len: 0,
            failed: false,
            has_attachment: false,
            has_ignorables: false,
            has_space_fallback: false,
            non_ascii: false,
            has_cgj: false,
        }
    }

    /// Loads `text`, one glyph per character, clusters at byte offsets. Keeps allocations.
    pub(crate) fn reset(&mut self, text: &str) {
        self.info.clear();
        self.non_ascii = false;
        self.has_ignorables = false;
        self.has_cgj = false;
        for (at, ch) in text.char_indices() {
            let cp = u32::from(ch);
            let info = GlyphInfo::new(cp, u32::try_from(at).unwrap_or(u32::MAX));
            self.non_ascii |= cp >= 0x80;
            self.has_ignorables |= info.props.is_ignorable();
            self.has_cgj |= cp == 0x034F;
            self.info.push(info);
        }
        self.len = self.info.len();
        self.idx = 0;
        self.out_len = 0;
        self.have_output = false;
        self.separate = false;
        self.serial = 0;
        self.failed = false;
        self.has_attachment = false;
        self.has_space_fallback = false;
        self.max_len = (self.len.saturating_mul(MAX_LEN_FACTOR)).max(MAX_LEN_MIN);
        let ops = i64::try_from(self.len).unwrap_or(i64::MAX / MAX_OPS_FACTOR);
        self.max_ops = ops.saturating_mul(MAX_OPS_FACTOR).max(MAX_OPS_MIN);
    }

    pub(crate) fn cur(&self, offset: usize) -> &GlyphInfo {
        &self.info[self.idx + offset]
    }
    pub(crate) fn cur_mut(&mut self, offset: usize) -> &mut GlyphInfo {
        let i = self.idx + offset;
        &mut self.info[i]
    }

    pub(crate) fn out_info(&self, i: usize) -> &GlyphInfo {
        if self.separate {
            &self.out[i]
        } else {
            &self.info[i]
        }
    }
    pub(crate) fn out_info_mut(&mut self, i: usize) -> &mut GlyphInfo {
        if self.separate {
            &mut self.out[i]
        } else {
            &mut self.info[i]
        }
    }
    fn set_out(&mut self, i: usize, info: GlyphInfo) {
        *self.out_info_mut(i) = info;
    }
    pub(crate) fn prev(&self) -> &GlyphInfo {
        self.out_info(self.out_len - 1)
    }

    pub(crate) fn backtrack_len(&self) -> usize {
        if self.have_output {
            self.out_len
        } else {
            self.idx
        }
    }
    pub(crate) fn lookahead_len(&self) -> usize {
        self.len - self.idx
    }

    pub(crate) fn clear_output(&mut self) {
        self.have_output = true;
        self.idx = 0;
        self.out_len = 0;
        self.separate = false;
    }

    /// Makes the output the input.
    pub(crate) fn sync(&mut self) {
        if !self.failed {
            self.next_glyphs(self.len - self.idx);
        }
        if self.separate {
            std::mem::swap(&mut self.info, &mut self.out);
        }
        self.len = if self.failed { 0 } else { self.out_len };
        self.have_output = false;
        self.separate = false;
        self.out_len = 0;
        self.idx = 0;
    }

    pub(crate) fn allocate_lig_id(&mut self) -> u8 {
        loop {
            self.serial = self.serial.wrapping_add(1);
            let id = self.serial & 0x07;
            if id != 0 {
                return id;
            }
        }
    }
}
