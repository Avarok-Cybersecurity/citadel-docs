//! Line breaking: a paragraph's pieces as `cdoc-para` items, broken by the breaker the options
//! name ([`BreakerChoice`]), and set back as lines of pieces at positions.
//!
//! Tabs are the one item whose width depends on where it lands: it reaches the next tab stop
//! from its position on its line. Their widths are first estimated as if the paragraph were
//! one line, the paragraph broken, the widths measured on the lines the break produced, and
//! the paragraph broken again until the two agree (a paragraph without tabs is broken once).

mod items;
mod lines;

use crate::context::{BreakerChoice, Ctx};
use crate::ir::Pt100;
use crate::pieces::Piece;
use crate::LayoutError;
use cdoc_model::BreakKind;
use cdoc_para::{Breaker, Breaking, FirstFit, KnuthPlass, LineWidths, Params};

/// How often tab widths are measured again before the last breaking is taken.
const TAB_ROUNDS: usize = 4;

/// A piece at its position on the line, from the line's start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// Offset from the line's start.
    pub x: Pt100,
    /// Width as set (a tab's is the distance to its stop).
    pub width: Pt100,
    /// The piece.
    pub piece: Piece,
}

/// One line, before alignment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    /// Where the line starts, from the paragraph box's left edge.
    pub start: Pt100,
    /// The width it may fill.
    pub available: Pt100,
    /// The pieces, spaces already at the width the line sets them to.
    pub items: Vec<Item>,
    /// Width up to the end of the last piece that is not a space.
    pub width: Pt100,
    /// Ended by a forced break or the end of the paragraph: never justified.
    pub hard_end: bool,
    /// A page or column break after this line.
    pub break_after: Option<BreakKind>,
}

/// Where lines start and how wide they are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Measure {
    /// First line: start and width.
    pub first: (Pt100, Pt100),
    /// Every other line.
    pub rest: (Pt100, Pt100),
    /// Distance between default tab stops, measured from the box's left edge.
    pub tab: Pt100,
}

/// Everything the items and lines are built for.
pub struct Setup {
    /// Line starts: first, rest.
    pub starts: (Pt100, Pt100),
    /// Line widths: first, rest (each at least 1).
    pub widths: (Pt100, Pt100),
    /// Distance between tab stops.
    pub tab: Pt100,
    /// Whether pattern hyphenation is on.
    pub hyphenate: bool,
    /// Whether the paragraph is justified.
    pub justified: bool,
    /// The breaker.
    pub breaker: BreakerChoice,
}

/// The paragraph's words as one breaker sees them.
pub struct Paragraph {
    /// Measure and starts.
    pub measure: Measure,
    /// Whether the paragraph is hyphenated.
    pub hyphenate: bool,
    /// Whether it is justified.
    pub justified: bool,
}

fn break_items(setup: &Setup, items: &items::Items) -> Result<Breaking, LayoutError> {
    let widths = LineWidths::per_line(&[setup.widths.0, setup.widths.1])?;
    Ok(match setup.breaker {
        BreakerChoice::FirstFit => {
            FirstFit.break_lines(&items.list, &widths, &Params::word_like())?
        }
        BreakerChoice::KnuthPlass(params) => {
            KnuthPlass.break_lines(&items.list, &widths, &params)?
        }
    })
}

/// The width each tab reaches its stop with, on the lines `breaking` (or, with none, one
/// line broken only at forced breaks) puts it on.
fn tab_widths(setup: &Setup, items: &items::Items, breaking: Option<&Breaking>) -> Vec<Pt100> {
    let stop = setup.tab.max(1);
    let mut out = Vec::new();
    let mut line = 0;
    let mut x = setup.starts.0;
    for (k, item) in items.list.iter().enumerate() {
        let starts_line = breaking
            .and_then(|b| b.lines.get(line + 1))
            .is_some_and(|l| l.start == k);
        let forced = matches!(item, cdoc_para::Item::Penalty { penalty, .. }
            if *penalty <= -cdoc_para::INFINITE_PENALTY);
        if starts_line {
            line += 1;
            x = setup.starts.1;
        }
        match item {
            cdoc_para::Item::Box {
                data: items::Seg::Tab(_),
                ..
            } => {
                let width = ((x.div_euclid(stop) + 1) * stop - x).max(0);
                out.push(width);
                x += width;
            }
            cdoc_para::Item::Box { width, .. } | cdoc_para::Item::Glue { width, .. } => x += width,
            _ if forced && breaking.is_none() => x = setup.starts.1,
            _ => {}
        }
    }
    out
}

/// Breaks the pieces into lines. There is always at least one line, so an empty paragraph
/// still occupies its place.
pub fn wrap(
    ctx: &mut Ctx<'_, '_>,
    pieces: Vec<Piece>,
    paragraph: &Paragraph,
) -> Result<Vec<Line>, LayoutError> {
    let m = paragraph.measure;
    let setup = Setup {
        starts: (m.first.0, m.rest.0),
        widths: (m.first.1.max(1), m.rest.1.max(1)),
        tab: m.tab,
        hyphenate: paragraph.hyphenate,
        justified: paragraph.justified,
        breaker: ctx.options.breaker,
    };
    let has_tabs = pieces.iter().any(|p| matches!(p, Piece::Tab { .. }));
    let mut tabs = Vec::new();
    if has_tabs {
        let guess = items::items(ctx, &pieces, &setup, &[]);
        tabs = tab_widths(&setup, &guess, None);
    }
    let mut built = items::items(ctx, &pieces, &setup, &tabs);
    let mut breaking = break_items(&setup, &built)?;
    for _ in 0..TAB_ROUNDS {
        if !has_tabs {
            break;
        }
        let measured = tab_widths(&setup, &built, Some(&breaking));
        if measured == tabs {
            break;
        }
        tabs = measured;
        built = items::items(ctx, &pieces, &setup, &tabs);
        breaking = break_items(&setup, &built)?;
    }
    let mut pieces: Vec<Option<Piece>> = pieces.into_iter().map(Some).collect();
    Ok(breaking
        .lines
        .iter()
        .enumerate()
        .map(|(n, l)| lines::line(&mut pieces, &built, l, n, &setup))
        .collect())
}
