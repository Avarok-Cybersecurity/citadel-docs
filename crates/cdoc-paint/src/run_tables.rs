//! The tables a run's glyphs are predicted from, built by the writer before any body: each
//! face's glyph id → glyph-table index, its char table (the index most first glyphs of a
//! cluster starting with that character use), and the strikes (face and size) with each
//! glyph's base advance.

use crate::chars::{char_at, CharTable};
use crate::codec::Writer;
use crate::encode_tables::u32_of;
use crate::op::{DisplayList, FaceEntry, Op};
use crate::strike::Strike;
use crate::PaintError;
use std::collections::{BTreeMap, HashMap};

/// No glyph-table index for a glyph id.
const ABSENT: u32 = u32::MAX;

/// Glyphs a face votes on before a run whose characters have all been voted on is skipped:
/// past this, the predictions are settled and the writer stops reading every glyph twice.
const SETTLED: usize = 16_384;

/// A majority vote (Boyer-Moore) per character: the index most of its glyphs use.
#[derive(Default)]
struct Votes {
    ascii: Vec<(u32, u32)>,
    other: BTreeMap<u32, (u32, u32)>,
    /// ASCII characters voted on, as bits.
    seen: u128,
    voted: usize,
}

impl Votes {
    /// Whether `text`'s run should vote: always until the face has settled, then only when it
    /// has a character no vote has seen.
    fn wants(&self, text: &str) -> bool {
        self.voted < SETTLED
            || !text.bytes().all(|b| b < 0x80 && self.seen & (1 << b) != 0)
                && text.chars().any(|c| match u8::try_from(c) {
                    Ok(b) if b < 0x80 => self.seen & (1 << b) == 0,
                    _ => !self.other.contains_key(&u32::from(c)),
                })
    }

    fn vote(&mut self, code_point: u32, index: u32) {
        self.voted += 1;
        if code_point < 0x80 {
            self.seen |= 1 << code_point;
        }
        if self.ascii.is_empty() {
            self.ascii = vec![(ABSENT, 0); 128];
        }
        let slot = match self.ascii.get_mut(code_point as usize) {
            Some(slot) => slot,
            None => self.other.entry(code_point).or_insert((ABSENT, 0)),
        };
        if slot.1 == 0 {
            *slot = (index, 1);
        } else if slot.0 == index {
            slot.1 += 1;
        } else {
            slot.1 -= 1;
        }
    }

    fn table(&self) -> CharTable {
        let mut table = CharTable::default();
        let ascii = (0_u32..).zip(self.ascii.iter().copied());
        for (cp, (index, _)) in ascii.chain(self.other.iter().map(|(c, v)| (*c, *v))) {
            if index != ABSENT {
                table.push(cp, index);
            }
        }
        table
    }
}

/// Everything a run is predicted from.
pub struct RunTables {
    index: Vec<Vec<u32>>,
    pub chars: Vec<CharTable>,
    pub strikes: Vec<Strike>,
    strike_of: HashMap<(u16, i32), u32>,
}

/// The floor of `advance · size / units_per_em`: the advance a glyph set alone would have,
/// rounded down (a shaped advance is this or one more, unless kerned or justified).
fn base(advance: u16, size: i32, units_per_em: u16) -> i32 {
    let exact = (i64::from(advance) * i64::from(size)).div_euclid(i64::from(units_per_em.max(1)));
    i32::try_from(exact).unwrap_or(0)
}

impl RunTables {
    pub fn collect(list: &DisplayList) -> Result<Self, PaintError> {
        let mut tables = Self {
            index: list.faces.iter().map(index_of).collect(),
            chars: Vec::new(),
            strikes: Vec::new(),
            strike_of: HashMap::new(),
        };
        let mut votes: Vec<Votes> = list.faces.iter().map(|_| Votes::default()).collect();
        for op in list.pages.iter().flat_map(|p| &p.ops) {
            let Op::GlyphRun { head, text, glyphs } = op else {
                continue;
            };
            let face = head.face.0;
            let entry = list
                .faces
                .get(usize::from(face))
                .ok_or(PaintError::NoFace(face))?;
            if let std::collections::hash_map::Entry::Vacant(slot) =
                tables.strike_of.entry((face, head.size))
            {
                slot.insert(u32_of(tables.strikes.len())?);
                tables.strikes.push(Strike {
                    face,
                    size: head.size,
                    bases: (entry.glyphs.iter())
                        .map(|g| base(g.advance, head.size, entry.units_per_em))
                        .collect(),
                });
            }
            let votes = &mut votes[usize::from(face)];
            if !votes.wants(text) {
                continue;
            }
            let mut previous = None;
            for g in glyphs {
                let index = tables.index(face, g.id)?;
                let at = usize::try_from(g.cluster).unwrap_or(usize::MAX);
                if previous != Some(g.cluster) {
                    if let Some((cp, _)) = char_at(text, at) {
                        votes.vote(cp, index);
                    }
                }
                previous = Some(g.cluster);
            }
        }
        tables.chars = votes.iter().map(Votes::table).collect();
        Ok(tables)
    }

    /// The glyph-table index of `id` in `face`; a run may draw only glyphs its face lists.
    pub fn index(&self, face: u16, id: u16) -> Result<u32, PaintError> {
        match self.index[usize::from(face)].get(usize::from(id)) {
            Some(&index) if index != ABSENT => Ok(index),
            _ => Err(PaintError::NoGlyph { face, id }),
        }
    }

    /// The strike of a run's face and size (every run's is collected).
    pub fn strike(&self, face: u16, size: i32) -> u32 {
        self.strike_of[&(face, size)]
    }

    /// One face's char table: `uv` count, then (`uv` gap, `uv` index) in code-point order.
    pub fn write_chars(&self, w: &mut Writer, face: usize) -> Result<(), PaintError> {
        let Some(table) = self.chars.get(face) else {
            w.uv(0);
            return Ok(());
        };
        w.uv(u32_of(table.entries().count())?);
        let mut next = 0;
        for (cp, index) in table.entries() {
            w.uv(cp - next);
            w.uv(index);
            next = cp + 1;
        }
        Ok(())
    }

    /// The strike table: `uv` count, then per strike `uv` face, `zz` size, its face's bases.
    pub fn write_strikes(&self, w: &mut Writer) -> Result<(), PaintError> {
        w.uv(u32_of(self.strikes.len())?);
        for s in &self.strikes {
            w.uv(u32::from(s.face));
            w.zz(s.size);
            w.zzs(&s.bases);
        }
        Ok(())
    }
}

fn index_of(face: &FaceEntry) -> Vec<u32> {
    let len = face.glyphs.iter().map(|g| usize::from(g.id) + 1).max();
    let mut index = vec![ABSENT; len.unwrap_or(0)];
    for (k, g) in (0_u32..).zip(&face.glyphs) {
        if index[usize::from(g.id)] == ABSENT {
            index[usize::from(g.id)] = k;
        }
    }
    index
}
