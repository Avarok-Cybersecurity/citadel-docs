//! The tables the writer builds before the page bodies: strings in order of first use, and
//! per face the glyph-id → table-index map text runs are written with.

use crate::op::{DisplayList, Op};
use crate::PaintError;
use std::collections::BTreeMap;

pub fn u32_of(value: usize) -> Result<u32, PaintError> {
    u32::try_from(value).map_err(|_| PaintError::TooLarge)
}

/// Strings in order of first use, each stored once.
pub struct Strings<'a> {
    pub order: Vec<&'a str>,
    pub index: BTreeMap<&'a str, u32>,
}

impl<'a> Strings<'a> {
    pub fn collect(list: &'a DisplayList) -> Result<Self, PaintError> {
        let mut strings = Self {
            order: Vec::new(),
            index: BTreeMap::new(),
        };
        for op in list.pages.iter().flat_map(|p| &p.ops) {
            if let Op::Image { content_id: s, .. } | Op::Link { href: s, .. } = op {
                if !strings.index.contains_key(s.as_str()) {
                    let next = u32_of(strings.order.len())?;
                    strings.index.insert(s, next);
                    strings.order.push(s);
                }
            }
        }
        Ok(strings)
    }
}

/// Per face, glyph id → its index in the face entry's glyph table.
pub struct Locals(Vec<Vec<u32>>);

impl Locals {
    pub fn new(list: &DisplayList) -> Self {
        Self(
            list.faces
                .iter()
                .map(|f| {
                    let len = f.glyphs.iter().map(|g| usize::from(g.id) + 1).max();
                    let mut local = vec![u32::MAX; len.unwrap_or(0)];
                    for (k, g) in f.glyphs.iter().enumerate().rev() {
                        local[usize::from(g.id)] = u32::try_from(k).unwrap_or(u32::MAX);
                    }
                    local
                })
                .collect(),
        )
    }

    pub fn index(&self, face: u16, glyph: u16) -> Result<u32, PaintError> {
        self.0
            .get(usize::from(face))
            .ok_or(PaintError::NoFace(face))?
            .get(usize::from(glyph))
            .copied()
            .filter(|i| *i != u32::MAX)
            .ok_or(PaintError::GlyphNotInTable { face, glyph })
    }
}
