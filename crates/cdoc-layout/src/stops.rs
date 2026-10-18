//! Caret stops along a line as its pieces are set: the gaps between a paragraph's units at the
//! pens the glyphs leave them at.

use crate::ir::{CaretStop, Draw, GlyphRun, Page, PlacedGlyph, Pt100, Units};

/// The pen before each character of `text` and after its last, from `x`, by the glyphs' own
/// advances (what the painter draws them with). A cluster's characters share its width evenly,
/// so a ligature has a caret stop inside it; characters before the first cluster take none.
#[must_use]
pub fn pens(text: &str, glyphs: &[PlacedGlyph], x: Pt100) -> Vec<Pt100> {
    let mut out = Vec::with_capacity(text.len() + 1);
    out.push(x);
    let (mut pen, mut done) = (x, 0);
    let mut glyphs = glyphs.iter().peekable();
    while let Some(first) = glyphs.next() {
        let start = (first.cluster as usize).min(text.len());
        let mut width = first.advance;
        while let Some(g) = glyphs.next_if(|g| g.cluster == first.cluster) {
            width += g.advance;
        }
        let end = glyphs
            .peek()
            .map_or(text.len(), |g| (g.cluster as usize).min(text.len()));
        let before = text.get(done..start).map_or(0, |t| t.chars().count());
        out.extend(std::iter::repeat_n(pen, before));
        let parts = text.get(start..end).map_or(0, |t| t.chars().count());
        let parts = i64::try_from(parts).unwrap_or(i64::MAX);
        for k in 1..=parts {
            let share = i64::from(width) * k / parts;
            out.push(pen + Pt100::try_from(share).unwrap_or(width));
        }
        pen += width;
        done = end.max(start);
    }
    let rest = text.get(done..).map_or(0, |t| t.chars().count());
    out.extend(std::iter::repeat_n(pen, rest));
    out
}

/// A line's stops, gathered left to right.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Stops(pub Vec<CaretStop>);

impl Stops {
    /// Gap `gap` at `x`. A gap the line already reached is not stood at again: a stop for the
    /// last gap moves to the right (an object drawn as several pieces, a field's value, ends
    /// where its last piece does) and one for an earlier gap is dropped.
    pub fn push(&mut self, gap: u32, x: Pt100) {
        match self.0.last_mut() {
            Some(last) if last.gap == gap => last.x = last.x.max(x),
            Some(last) if last.gap > gap => {}
            _ => self.0.push(CaretStop { gap, x }),
        }
    }

    /// One unit drawn from `x` to `x + width`.
    pub fn unit(&mut self, unit: u32, x: Pt100, width: Pt100) {
        self.push(unit, x);
        self.push(unit.saturating_add(1), x + width);
    }
}

/// A run's stops, from its clusters and advances and its characters' units: a stop before a
/// unit that does not follow the one before it, and one after every unit, except after a unit
/// followed by one drawn with no width (a combining mark typed as a character of its own), so
/// a caret never parts the two.
#[must_use]
pub fn run_stops(run: &GlyphRun) -> Vec<CaretStop> {
    let pens = pens(&run.text, &run.glyphs, run.x);
    let count = u32::try_from(pens.len() - 1).unwrap_or(u32::MAX);
    let units: Vec<Option<u32>> = run.units.each(count).collect();
    // Groups of characters drawing one unit: (unit, first character, one past the last).
    let mut groups: Vec<(u32, usize, usize)> = Vec::new();
    for (k, unit) in units.iter().enumerate() {
        match (unit, groups.last_mut()) {
            (Some(u), Some(last)) if last.0 == *u && last.2 == k => last.2 = k + 1,
            (Some(u), _) => groups.push((*u, k, k + 1)),
            (None, _) => {}
        }
    }
    let mut out = Stops::default();
    for (n, &(unit, start, end)) in groups.iter().enumerate() {
        let follows = n
            .checked_sub(1)
            .is_some_and(|p| groups[p].2 == start && groups[p].0.wrapping_add(1) == unit);
        if !follows {
            out.push(unit, pens[start]);
        }
        let zero_next = groups.get(n + 1).is_some_and(|&(next, s, e)| {
            s == end && next == unit.wrapping_add(1) && pens[e] == pens[s]
        });
        if !zero_next {
            out.push(unit.saturating_add(1), pens[end]);
        }
    }
    out.0
}

/// Every caret line's stops on `page`, line by line: each line's runs (the page's runs of
/// document text in drawing order, `runs` per line in line order) and the stops it keeps,
/// put in x order. `None` when the page has fewer runs of document text than its lines claim.
#[must_use]
pub fn caret_stops(page: &Page) -> Option<Vec<Vec<CaretStop>>> {
    let mut runs = page.draws.iter().filter_map(|d| match d {
        Draw::Text(run) if run.units != Units::None => Some(run),
        _ => None,
    });
    let mut out = Vec::with_capacity(page.lines.len());
    for line in &page.lines {
        let mut all = line.stops.clone();
        for _ in 0..line.runs {
            all.extend(run_stops(runs.next()?));
        }
        all.sort_by_key(|s| (s.x, s.gap));
        let mut stops = Stops::default();
        for s in all {
            stops.push(s.gap, s.x);
        }
        out.push(stops.0);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::{pens, run_stops, Stops};
    use crate::ir::{CaretStop, FaceId, GlyphRun, PlacedGlyph, SpanKind, UnitSpan, Units};
    use cdoc_model::Color;

    fn glyphs(list: &[(u32, i32)]) -> Vec<PlacedGlyph> {
        list.iter()
            .map(|&(cluster, advance)| PlacedGlyph {
                id: 1,
                advance,
                x_offset: 0,
                y_offset: 0,
                cluster,
            })
            .collect()
    }

    #[test]
    fn a_ligatures_characters_share_its_width_and_a_cluster_of_glyphs_is_one_stop() {
        // "fi" as one ligature glyph, then "o"; then "é" as a base and a mark, then "x".
        assert_eq!(
            pens("fio", &glyphs(&[(0, 1000), (2, 500)]), 100),
            [100, 600, 1100, 1600]
        );
        assert_eq!(
            pens("éx", &glyphs(&[(0, 600), (0, 0), (2, 400)]), 100),
            [100, 700, 1100]
        );
    }

    #[test]
    fn a_field_drawn_as_several_pieces_has_one_stop_on_each_side() {
        let mut stops = Stops::default();
        stops.unit(4, 0, 100);
        stops.unit(4, 100, 50);
        stops.unit(4, 150, 200);
        stops.unit(5, 350, 10);
        let gaps: Vec<(u32, i32)> = stops.0.iter().map(|s| (s.gap, s.x)).collect();
        assert_eq!(gaps, [(4, 0), (5, 350), (6, 360)]);
    }

    #[test]
    fn a_character_drawn_with_no_width_after_another_shares_its_stop() {
        // "e" then a combining acute typed as its own character: one glyph each, the mark's
        // advance zero.
        let run = GlyphRun {
            text: "e\u{301}x".to_owned(),
            glyphs: glyphs(&[(0, 500), (1, 0), (3, 400)]),
            x: 0,
            y: 0,
            size: 1100,
            face: FaceId(0),
            colour: Color::rgb(0, 0, 0),
            units: Units::One(UnitSpan {
                at: 0,
                unit: 0,
                kind: SpanKind::Each,
            }),
        };
        assert_eq!(
            run_stops(&run),
            [
                CaretStop { gap: 0, x: 0 },
                CaretStop { gap: 2, x: 500 },
                CaretStop { gap: 3, x: 900 },
            ]
        );
    }
}
