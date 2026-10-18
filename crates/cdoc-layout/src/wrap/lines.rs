//! The breaker's lines back as pieces at positions: words whole or in fragments (with the
//! hyphen a line broken inside a word ends in), spaces at the width the line sets them to
//! (their natural width, or `cdoc-para`'s exact justification), tabs at their widths.

use super::items::{Items, Seg};
use super::{Item as Placed, Line, Setup};
use crate::ir::{PlacedGlyph, Pt100};
use crate::pieces::Piece;
use cdoc_model::BreakKind;
use cdoc_para::{justify, HyphenKind, Item, INFINITE_PENALTY};

/// Glyphs `start..end` of a text piece, as a piece of their own.
fn fragment(piece: &Piece, start: usize, end: usize) -> Piece {
    let Piece::Text {
        text,
        style,
        glyphs,
        link,
        note,
        ..
    } = piece
    else {
        return piece.clone();
    };
    let from = glyphs[start].cluster as usize;
    let to = glyphs.get(end).map_or(text.len(), |g| g.cluster as usize);
    let base = u32::try_from(from).unwrap_or(0);
    let glyphs: Vec<PlacedGlyph> = glyphs[start..end]
        .iter()
        .map(|g| PlacedGlyph {
            cluster: g.cluster - base,
            ..*g
        })
        .collect();
    Piece::Text {
        text: text[from..to].to_owned(),
        style: style.clone(),
        width: glyphs.iter().map(|g| g.advance).sum(),
        glyphs,
        link: link.clone(),
        note: *note,
    }
}

/// A space set `width` wide: the extra goes on its last glyph's advance.
fn space(piece: Piece, width: Pt100) -> Piece {
    let Piece::Space {
        style,
        width: natural,
        mut glyphs,
        link,
    } = piece
    else {
        return piece;
    };
    if let Some(last) = glyphs.last_mut() {
        last.advance += width - natural;
    }
    Piece::Space {
        style,
        width,
        glyphs,
        link,
    }
}

/// One broken line, set. Whole pieces and spaces are taken out of `pieces` (each is on one
/// line only); a text piece broken across lines stays, for its other fragments.
pub fn line(
    pieces: &mut [Option<Piece>],
    items: &Items,
    line: &cdoc_para::Line,
    number: usize,
    setup: &Setup,
) -> Line {
    let (start, available) = if number == 0 {
        (setup.starts.0, setup.widths.0)
    } else {
        (setup.starts.1, setup.widths.1)
    };
    let end = &items.list[line.end];
    let hard_end = matches!(end, Item::Penalty { penalty, .. } if *penalty <= -INFINITE_PENALTY);
    let break_after = items.owner[line.end].and_then(|p| match pieces[p] {
        Some(Piece::Break(BreakKind::Line)) | None => None,
        Some(Piece::Break(kind)) => Some(kind),
        Some(_) => None,
    });
    let set = setup.justified && !hard_end;
    let glue = set.then(|| justify(&items.list, line, available.max(1)).glue);
    let mut glue = glue.iter().flatten();
    let mut out = Line {
        start,
        available: available.max(1),
        items: Vec::with_capacity(line.end - line.start + 1),
        width: 0,
        hard_end,
        break_after,
    };
    let mut x = 0;
    let mut place = |piece: Piece, width: Pt100, out: &mut Line| {
        if !matches!(piece, Piece::Space { .. }) {
            out.width = x + width;
        }
        out.items.push(Placed { x, width, piece });
        x += width;
    };
    for k in line.start..line.end {
        match &items.list[k] {
            Item::Box { data, width } => {
                let piece = match *data {
                    Seg::Whole(p) | Seg::Tab(p) => pieces[p].take(),
                    Seg::Part(p, from, to) => pieces[p].as_ref().map(|w| fragment(w, from, to)),
                };
                if let Some(piece) = piece {
                    place(piece, *width, &mut out);
                }
            }
            Item::Glue { width, .. } => {
                let set = glue.next().copied().unwrap_or(*width);
                if let Some(piece) = items.owner[k].and_then(|p| pieces[p].take()) {
                    place(space(piece, set), set, &mut out);
                }
            }
            Item::Penalty { .. } | Item::Hyphen { .. } => {}
        }
    }
    let inserted = matches!(end, Item::Hyphen { kind, .. } if *kind != HyphenKind::Explicit);
    if let (true, Item::Hyphen { width, .. }, Some(Seg::Part(p, _, _))) =
        (inserted, end, last_part(items, line))
    {
        if let (Some((glyphs, _)), Some(Piece::Text { style, link, .. })) =
            (items.hyphens.get(&p), &pieces[p])
        {
            let hyphen = Piece::Text {
                text: "-".to_owned(),
                style: style.clone(),
                width: *width,
                glyphs: glyphs.clone(),
                link: link.clone(),
                note: None,
            };
            place(hyphen, *width, &mut out);
        }
    }
    while matches!(out.items.last(), Some(i) if matches!(i.piece, Piece::Space { .. })) {
        out.items.pop();
    }
    out
}

fn last_part(items: &Items, line: &cdoc_para::Line) -> Option<Seg> {
    items.list[line.start..line.end]
        .iter()
        .rev()
        .find_map(|i| match i {
            Item::Box { data, .. } => Some(*data),
            _ => None,
        })
}
