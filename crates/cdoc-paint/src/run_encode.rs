//! Writing a `GLYPH_RUN`: its strike, colour and origin, its text, then only the glyphs that
//! differ from their prediction, column by column (see `run_view` for the reading side and
//! `docs/display-list.md` for the bytes).

use crate::chars::{after, char_at, predicted};
use crate::codec::{zigzag, Writer};
use crate::encode_tables::u32_of;
use crate::format::{
    RUN_ADVANCES, RUN_CLUSTERS, RUN_IDS, RUN_PLUS_ONE, RUN_X_OFFSETS, RUN_Y_OFFSETS,
};
use crate::op::{Glyph, RunHead};
use crate::run_tables::RunTables;
use crate::PaintError;

/// One sparse column being written: (`uv` gap, value) pairs.
struct Column {
    w: Writer,
    next: usize,
}

impl Column {
    fn new() -> Self {
        Self {
            w: Writer::with_capacity(64),
            next: 0,
        }
    }

    fn clear(&mut self) {
        self.w.buf.clear();
        self.next = 0;
    }

    #[inline]
    fn entry(&mut self, i: usize, value: u32) -> Result<(), PaintError> {
        let gap = u32_of(i - self.next)?;
        self.next = i + 1;
        if let (Ok(gap @ 0..0x80), Ok(value @ 0..0x80)) = (u8::try_from(gap), u8::try_from(value)) {
            self.w.bytes(&[gap, value]);
        } else {
            self.w.uv(gap);
            self.w.uv(value);
        }
        Ok(())
    }

    fn write(&self, w: &mut Writer) -> Result<(), PaintError> {
        if !self.w.buf.is_empty() {
            w.uv(u32_of(self.w.buf.len())?);
            w.bytes(&self.w.buf);
        }
        Ok(())
    }
}

/// The writer's scratch for runs, reused from one run to the next.
pub struct RunWriter {
    ids: Column,
    clusters: Column,
    plus_one: Vec<u8>,
    advances: Column,
    x_offsets: Column,
    y_offsets: Column,
    /// The last run's face, size and strike: most runs follow one in the same strike.
    last: Option<(u16, i32, u32)>,
}

impl RunWriter {
    pub fn new() -> Self {
        Self {
            ids: Column::new(),
            clusters: Column::new(),
            plus_one: Vec::with_capacity(32),
            advances: Column::new(),
            x_offsets: Column::new(),
            y_offsets: Column::new(),
            last: None,
        }
    }

    /// One run's payload.
    pub fn run(
        &mut self,
        w: &mut Writer,
        head: &RunHead,
        text: &str,
        glyphs: &[Glyph],
        tables: &RunTables,
    ) -> Result<(), PaintError> {
        let face = head.face.0;
        let strike = match self.last {
            Some((f, size, strike)) if f == face && size == head.size => strike,
            _ => tables.strike(face, head.size),
        };
        self.last = Some((face, head.size, strike));
        let bases = &tables.strikes[strike as usize].bases;
        let chars = &tables.chars[usize::from(face)];
        for c in [
            &mut self.ids,
            &mut self.clusters,
            &mut self.advances,
            &mut self.x_offsets,
            &mut self.y_offsets,
        ] {
            c.clear();
        }
        self.plus_one.clear();
        self.plus_one.resize(glyphs.len().div_ceil(8), 0);
        let mut next_cluster: u32 = 0;
        for (i, g) in glyphs.iter().enumerate() {
            if g.cluster != next_cluster {
                let delta = i64::from(g.cluster) - i64::from(next_cluster);
                let delta = i32::try_from(delta).map_err(|_| PaintError::TooLarge)?;
                self.clusters.entry(i, zigzag(delta))?;
            }
            let at = usize::try_from(g.cluster).unwrap_or(usize::MAX);
            let (guess, width) = predicted(chars, char_at(text, at));
            let index = tables.index(face, g.id)?;
            if guess != index {
                self.ids.entry(i, index)?;
            }
            match g.advance.wrapping_sub(bases[index as usize]) {
                0 => {}
                1 => self.plus_one[i >> 3] |= 1 << (i & 7),
                delta => self.advances.entry(i, zigzag(delta))?,
            }
            if g.x_offset != 0 {
                self.x_offsets.entry(i, zigzag(g.x_offset))?;
            }
            if g.y_offset != 0 {
                self.y_offsets.entry(i, zigzag(g.y_offset))?;
            }
            next_cluster = after(g.cluster, width);
        }
        let plus_one = self.plus_one.iter().any(|b| *b != 0);
        let flags = [
            (!self.ids.w.buf.is_empty(), RUN_IDS),
            (!self.clusters.w.buf.is_empty(), RUN_CLUSTERS),
            (plus_one, RUN_PLUS_ONE),
            (!self.advances.w.buf.is_empty(), RUN_ADVANCES),
            (!self.x_offsets.w.buf.is_empty(), RUN_X_OFFSETS),
            (!self.y_offsets.w.buf.is_empty(), RUN_Y_OFFSETS),
        ]
        .iter()
        .fold(0, |f, (set, flag)| if *set { f | flag } else { f });
        w.uv(strike);
        w.bytes(&[head.colour.r, head.colour.g, head.colour.b, head.colour.a]);
        w.zz(head.x);
        w.zz(head.y);
        w.uv(u32_of(text.len())?);
        w.bytes(text.as_bytes());
        w.uv(u32_of(glyphs.len())?);
        w.u8(flags);
        self.ids.write(w)?;
        self.clusters.write(w)?;
        if plus_one {
            w.bytes(&self.plus_one);
        }
        self.advances.write(w)?;
        self.x_offsets.write(w)?;
        self.y_offsets.write(w)
    }
}
