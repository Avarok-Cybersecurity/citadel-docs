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
