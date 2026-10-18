//! What the layout draws in each face: the glyph ids (which become the font's CIDs, so the
//! content stream speaks the layout's own glyph ids) and the text each stands for, which is
//! the face's `ToUnicode` map.

use cdoc_layout::{GlyphRun, Layout};
use std::collections::{BTreeMap, BTreeSet};

/// One face's glyphs and their text.
#[derive(Debug, Default)]
pub struct Used {
    /// Every glyph id drawn, with `.notdef`.
    pub glyphs: BTreeSet<u16>,
    /// The text of each glyph that starts a cluster, from the first run that draws it.
    pub unicode: BTreeMap<u16, String>,
}

/// The glyphs and text of every face the layout's runs draw, by face id.
pub fn gather(layout: &Layout) -> BTreeMap<u16, Used> {
    let mut out: BTreeMap<u16, Used> = BTreeMap::new();
    for run in layout.pages.iter().flat_map(cdoc_layout::Page::runs) {
        let used = out.entry(run.face.0).or_default();
        used.glyphs.insert(0);
        for (index, glyph) in run.glyphs.iter().enumerate() {
            used.glyphs.insert(glyph.id);
            let text = run.cluster_text(index);
            if !text.is_empty() {
                used.unicode
                    .entry(glyph.id)
                    .or_insert_with(|| text.to_owned());
            }
        }
    }
    out
}

/// What a reader extracts from `run` through `unicode`: each glyph's mapped text in order.
pub fn extracted(unicode: &BTreeMap<u16, String>, run: &GlyphRun) -> String {
    run.glyphs
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
