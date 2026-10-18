//! The table the writer builds before the page bodies: strings in order of first use.

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
    fn add(&mut self, s: &'a str) -> Result<(), PaintError> {
        if !self.index.contains_key(s) {
            let next = u32_of(self.order.len())?;
            self.index.insert(s, next);
            self.order.push(s);
        }
        Ok(())
    }

    /// The pages' strings in op order, then the document's: title, language, headings.
    pub fn collect(list: &'a DisplayList) -> Result<Self, PaintError> {
        let mut strings = Self {
            order: Vec::new(),
            index: BTreeMap::new(),
        };
        for op in list.pages.iter().flat_map(|p| &p.ops) {
            match op {
                Op::Image { content_id: s, .. } | Op::Link { href: s, .. } => strings.add(s)?,
                Op::SigField { id, role, .. } => {
                    strings.add(id)?;
                    strings.add(role)?;
                }
                _ => {}
            }
        }
        let doc = &list.document;
        for s in doc.title.iter().chain(&doc.language) {
            strings.add(s)?;
        }
        for entry in &doc.outline {
            strings.add(&entry.title)?;
        }
        Ok(strings)
    }

    /// An optional string as the document table writes it: 0 for none, else index + 1.
    pub fn optional(&self, s: Option<&String>) -> u32 {
        s.map_or(0, |s| self.index[s.as_str()] + 1)
    }
}
