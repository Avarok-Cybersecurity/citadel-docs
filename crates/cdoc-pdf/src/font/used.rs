//! What the display list draws in each face: the glyph ids (which become the font's CIDs, so
//! the content stream speaks the layout's own glyph ids) and the text each stands for, which
//! is the face's `ToUnicode` map.

use cdoc_layout::ir::cluster_text;
use cdoc_paint::Glyph;
use std::collections::{BTreeMap, BTreeSet};

/// One face's glyphs and their text.
#[derive(Debug, Default)]
pub struct Used {
    /// Every glyph id drawn, with `.notdef`.
    pub glyphs: BTreeSet<u16>,
    /// The text of each glyph that starts a cluster, from the first run that draws it.
    pub unicode: BTreeMap<u16, String>,
}

/// The text of glyph `index`'s cluster in a run over `text`.
pub fn cluster<'t>(text: &'t str, glyphs: &[Glyph], index: usize) -> &'t str {
    cluster_text(text, glyphs.len(), |i| glyphs[i].cluster, index)
}

impl Used {
    /// Adds a run of `glyphs` over `text` drawn in this face.
    pub fn add(&mut self, text: &str, glyphs: &[Glyph]) {
        self.glyphs.insert(0);
        for (index, glyph) in glyphs.iter().enumerate() {
            self.glyphs.insert(glyph.id);
            let text = cluster(text, glyphs, index);
            if !text.is_empty() {
                self.unicode
                    .entry(glyph.id)
                    .or_insert_with(|| text.to_owned());
            }
        }
    }
}

/// What a reader extracts from `glyphs` through `unicode`: each glyph's mapped text in order.
pub fn extracted(unicode: &BTreeMap<u16, String>, glyphs: &[Glyph]) -> String {
    glyphs
        .iter()
        .filter_map(|g| unicode.get(&g.id))
        .map(String::as_str)
        .collect()
}

/// The `CIDToGIDMap` stream: big-endian subset glyph id for every CID up to the largest.
pub fn cid_to_gid(pairs: impl Iterator<Item = (u16, u16)>) -> Vec<u8> {
    let pairs: Vec<(u16, u16)> = pairs.collect();
    let last = pairs
        .iter()
        .map(|(cid, _)| usize::from(*cid))
        .max()
        .unwrap_or(0);
    let mut out = vec![0; (last + 1) * 2];
    for (cid, gid) in pairs {
        let at = usize::from(cid) * 2;
        out[at..at + 2].copy_from_slice(&gid.to_be_bytes());
    }
    out
}
