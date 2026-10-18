//! Caret stops along a glyph run.

use cdoc_layout::{GlyphRun, Pt100};

/// The pen before each character of `run` and after its last, from the glyphs' own advances
/// (what the painter draws them with). A cluster's characters share its width evenly, so a
/// ligature has a caret stop inside it; characters before the first cluster take no width.
pub(super) fn pens(run: &GlyphRun) -> Vec<Pt100> {
    let mut out = Vec::with_capacity(run.text.len() + 1);
    out.push(run.x);
    let (mut pen, mut done) = (run.x, 0);
    let mut glyphs = run.glyphs.iter().peekable();
    while let Some(first) = glyphs.next() {
        let start = (first.cluster as usize).min(run.text.len());
        let mut width = first.advance;
        while let Some(g) = glyphs.next_if(|g| g.cluster == first.cluster) {
            width += g.advance;
        }
        let end = glyphs
            .peek()
            .map_or(run.text.len(), |g| (g.cluster as usize).min(run.text.len()));
        let before = run.text.get(done..start).map_or(0, |t| t.chars().count());
        out.extend(std::iter::repeat_n(pen, before));
        let parts = run.text.get(start..end).map_or(0, |t| t.chars().count());
        let parts = i64::try_from(parts).unwrap_or(i64::MAX);
        for k in 1..=parts {
            let share = i64::from(width) * k / parts;
            out.push(pen + Pt100::try_from(share).unwrap_or(width));
        }
        pen += width;
        done = end.max(start);
    }
    let rest = run.text.get(done..).map_or(0, |t| t.chars().count());
    out.extend(std::iter::repeat_n(pen, rest));
    out
}

#[cfg(test)]
mod tests {
    use super::pens;
    use cdoc_layout::{FaceId, GlyphRun, PlacedGlyph};
    use cdoc_model::Color;

    fn run(text: &str, glyphs: &[(u32, i32)]) -> GlyphRun {
        GlyphRun {
            text: text.to_owned(),
            glyphs: glyphs
                .iter()
                .map(|&(cluster, advance)| PlacedGlyph {
                    id: 1,
                    advance,
                    x_offset: 0,
                    y_offset: 0,
                    cluster,
                })
                .collect(),
            x: 100,
            y: 0,
            size: 1100,
            face: FaceId(0),
            colour: Color::rgb(0, 0, 0),
        }
    }

    #[test]
    fn a_ligatures_characters_share_its_width_and_a_cluster_of_glyphs_is_one_stop() {
        // "fi" as one ligature glyph, then "o"; then "é" as a base and a mark, then "x".
        assert_eq!(
            pens(&run("fio", &[(0, 1000), (2, 500)])),
            [100, 600, 1100, 1600]
        );
        assert_eq!(
            pens(&run("éx", &[(0, 600), (0, 0), (2, 400)])),
            [100, 700, 1100]
        );
    }
}
