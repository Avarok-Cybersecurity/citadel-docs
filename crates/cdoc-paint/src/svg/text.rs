//! Text for the SVG painter. SVG wants text (so the preview is selectable and searchable, and
//! draws in the `@font-face` the UI loaded), and every glyph run carries the text its glyphs
//! stand for: each cluster's characters are placed where the cluster's first glyph is drawn,
//! and the characters of a several-character cluster (a ligature) share its advance evenly.

use super::{attr_paint, escape};
use crate::decode::FaceView;
use crate::op::RunHead;
use crate::ShapedGlyphs;
use cdoc_layout::ir::to_pt;
use cdoc_layout::Pt100;
use std::fmt::Write as _;

fn family(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_'))
        .collect()
}

fn open(out: &mut String, head: &RunHead, face: &FaceView<'_>, x: &str, y: &str) {
    let _ = write!(
        out,
        "<text x=\"{x}\" y=\"{y}\" font-family=\"'{}'\" font-weight=\"{}\" font-style=\"{}\" \
         font-size=\"{}\" {} xml:space=\"preserve\">",
        family(face.family),
        if face.bold { 700 } else { 400 },
        if face.italic { "italic" } else { "normal" },
        to_pt(head.size),
        attr_paint("fill", head.colour),
    );
}

/// Per character of `text`, where it is drawn: `(x, y)` in page space.
fn positions(head: &RunHead, text: &str, glyphs: ShapedGlyphs<'_>) -> Vec<(Pt100, Pt100)> {
    // Per cluster: its byte start, where its first glyph is drawn, and its whole advance.
    let mut clusters: Vec<(usize, Pt100, Pt100, Pt100)> = Vec::new();
    let mut pen = head.x;
    for g in glyphs {
        let at = usize::try_from(g.cluster).unwrap_or(usize::MAX);
        match clusters.last_mut() {
            Some(c) if c.0 == at => c.3 += g.advance,
            _ => clusters.push((at, pen + g.x_offset, head.y + g.y_offset, g.advance)),
        }
        pen += g.advance;
    }
    let mut out = Vec::with_capacity(text.len());
    for (k, &(start, x, y, advance)) in clusters.iter().enumerate() {
        let end = clusters.get(k + 1).map_or(text.len(), |c| c.0);
        let count = text.get(start..end).map_or(0, |s| s.chars().count());
        let share = Pt100::try_from(count).unwrap_or(1).max(1);
        for i in 0..share.min(Pt100::try_from(count).unwrap_or(0)) {
            out.push((x + advance * i / share, y));
        }
    }
    out
}

/// A shaped run: every character at the position its cluster is drawn at.
pub fn shaped(
    out: &mut String,
    head: &RunHead,
    face: &FaceView<'_>,
    text: &str,
    glyphs: ShapedGlyphs<'_>,
) {
    let (mut xs, mut ys) = (String::new(), String::new());
    for (x, y) in positions(head, text, glyphs) {
        let _ = write!(xs, "{} ", to_pt(x));
        let _ = write!(ys, "{} ", to_pt(y));
    }
    open(out, head, face, xs.trim_end(), ys.trim_end());
    let _ = write!(out, "{}</text>", escape(text));
}
