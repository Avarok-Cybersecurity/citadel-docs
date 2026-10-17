//! Characters to glyphs and back: cluster spans, caret positions (inside ligatures from the
//! font's carets or by even division) and hit testing.

use super::ShapedRun;
use crate::face::Face;
use crate::unicode::Props;
use std::ops::Range;

/// A cluster: the characters and the glyphs that stand for each other.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClusterSpan {
    /// Byte range in the text.
    pub text: Range<usize>,
    /// Index range in [`ShapedRun::glyphs`].
    pub glyphs: Range<usize>,
}

/// Byte offsets inside `text` where a caret may stand (grapheme starts).
fn caret_stops(text: &str, range: Range<usize>) -> impl Iterator<Item = usize> + '_ {
    text.get(range.clone())
        .unwrap_or("")
        .char_indices()
        .filter(|&(_, c)| !Props::of(u32::from(c)).is_continuation())
        .map(move |(i, _)| range.start + i)
}

impl ShapedRun {
    /// The run's clusters in order. `text` is the string the run was shaped from.
    #[must_use]
    pub fn clusters(&self, text: &str) -> Vec<ClusterSpan> {
        let mut spans: Vec<ClusterSpan> = Vec::new();
        for (i, g) in self.glyphs.iter().enumerate() {
            let start = g.cluster as usize;
            match spans.last_mut() {
                Some(last) if last.text.start == start => last.glyphs.end = i + 1,
                _ => {
                    if let Some(last) = spans.last_mut() {
                        last.text.end = start;
                    }
                    spans.push(ClusterSpan {
                        text: start..text.len(),
                        glyphs: i..i + 1,
                    });
                }
            }
        }
        spans
    }

    /// The first glyph of the cluster holding the character at byte `at`.
    #[must_use]
    pub fn glyph_for_byte(&self, text: &str, at: usize) -> Option<usize> {
        self.clusters(text)
            .into_iter()
            .find(|s| s.text.contains(&at))
            .map(|s| s.glyphs.start)
    }

    /// The bytes of `text` glyph `index` stands for (its whole cluster).
    #[must_use]
    pub fn text_for_glyph(&self, text: &str, index: usize) -> Option<Range<usize>> {
        self.clusters(text)
            .into_iter()
            .find(|s| s.glyphs.contains(&index))
            .map(|s| s.text)
    }

    /// Every caret position of the run: `(byte offset, x in font units)`, left to right, the
    /// end of the run included. Inside a one-glyph cluster of several graphemes (a ligature)
    /// the font's GDEF carets are used when it has enough, else the advance is divided evenly.
    #[must_use]
    pub fn carets(&self, face: &Face<'_>, text: &str) -> Vec<(usize, i64)> {
        let mut out = Vec::new();
        let mut x = 0_i64;
        for span in self.clusters(text) {
            let width: i64 = self.glyphs[span.glyphs.clone()]
                .iter()
                .map(|g| i64::from(g.x_advance))
                .sum();
            let stops: Vec<usize> = caret_stops(text, span.text.clone()).collect();
            let n = i64::try_from(stops.len().max(1)).unwrap_or(1);
            let font = (span.glyphs.len() == 1)
                .then(|| face.ligature_carets(self.glyphs[span.glyphs.start].id))
                .flatten()
                .filter(|c| c.len() + 1 >= stops.len());
            for (k, &at) in stops.iter().enumerate() {
                let inside = match (&font, k) {
                    (_, 0) => 0,
                    (Some(c), k) => i64::from(c[k - 1]),
                    (None, k) => width * i64::try_from(k).unwrap_or(0) / n,
                };
                out.push((at, x + inside));
            }
            x += width;
        }
        out.push((text.len(), x));
        out
    }

    /// The caret position nearest to `x` (font units from the run's start), as a byte offset;
    /// the earlier one on a tie.
    #[must_use]
    pub fn hit_test(&self, face: &Face<'_>, text: &str, x: i64) -> usize {
        self.carets(face, text)
            .into_iter()
            .min_by_key(|&(at, cx)| ((cx - x).abs(), at))
            .map_or(0, |(at, _)| at)
    }
}
