//! Pieces as the line breaker's items: a word is a box, or several boxes with a hyphen
//! between them where it may break (a pattern point when the paragraph is hyphenated, after a
//! soft hyphen, after an explicit hyphen); a space is glue; a tab is a box with a break
//! allowed on either side; a forced break ends its line with `fil` glue so that line is not
//! stretched. A word wider than any line is cut into line-wide boxes, cluster by cluster.

use super::Setup;
use crate::context::{BreakerChoice, Ctx};
use crate::fonts::SOFT_HYPHEN;
use crate::ir::{PlacedGlyph, Pt100};
use crate::pieces::Piece;
use cdoc_para::{HyphenKind, Item, Stretch, INFINITE_PENALTY};
use std::collections::BTreeMap;

/// What a box stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Seg {
    /// A whole piece.
    Whole(usize),
    /// Glyphs `start..end` of a text piece.
    Part(usize, usize, usize),
    /// A tab piece.
    Tab(usize),
    /// A space piece hung after a line's last word: no width to the breaker, so it can
    /// neither be broken at nor push the line past its measure.
    Hung(usize),
}

/// The items, the piece behind each glue and forced break, and the hyphen each hyphenated
/// piece adds at a line end.
pub struct Items {
    /// The breaker's input.
    pub list: Vec<Item<Seg>>,
    /// Per item: the space piece of a glue, the break piece of a forced break.
    pub owner: Vec<Option<usize>>,
    /// Per text piece with hyphen items: the hyphen's glyphs and width.
    pub hyphens: BTreeMap<usize, (Vec<PlacedGlyph>, Pt100)>,
}

impl Items {
    fn push(&mut self, item: Item<Seg>, owner: Option<usize>) {
        self.list.push(item);
        self.owner.push(owner);
    }

    /// A forced break. The spaces before it hang (TeX's `\par` drops them; Word keeps them,
    /// and the caret typed past them must move).
    fn forced(&mut self, owner: Option<usize>) {
        let mut at = self.list.len();
        while at > 0 && matches!(self.list[at - 1], Item::Glue { .. }) {
            at -= 1;
            if let Some(space) = self.owner[at] {
                self.list[at] = Item::boxed(0, Seg::Hung(space));
                self.owner[at] = None;
            }
        }
        self.push(Item::penalty(INFINITE_PENALTY), None);
        let fil = Item::Glue {
            width: 0,
            stretch: Stretch::fil(1),
            shrink: 0,
        };
        self.push(fil, None);
        self.push(Item::penalty(-INFINITE_PENALTY), owner);
    }
}

fn is_explicit_hyphen(c: char) -> bool {
    matches!(c, '-' | '\u{2010}')
}

/// Where `text` may break, as `(byte offset, kind)`, ascending, before glyph-cluster checks.
fn points(
    ctx: &mut Ctx<'_, '_>,
    text: &str,
    patterns: Option<cdoc_para::Language>,
    found: &mut Vec<usize>,
) -> Vec<(usize, HyphenKind)> {
    let mut out = Vec::new();
    for (at, c) in text.char_indices() {
        let after = at + c.len_utf8();
        if after < text.len() && at > 0 {
            if c == SOFT_HYPHEN {
                out.push((after, HyphenKind::Soft));
            } else if is_explicit_hyphen(c) {
                out.push((after, HyphenKind::Explicit));
            }
        }
    }
    if let Some(hyphenator) = patterns.and_then(|l| ctx.fonts.hyphenator(l)) {
        ctx.hyphenators.points(hyphenator, text, found);
        out.extend(found.iter().map(|p| (*p, HyphenKind::Pattern)));
        out.sort_by_key(|(at, kind)| (*at, *kind == HyphenKind::Pattern));
        out.dedup_by_key(|(at, _)| *at);
    }
    out
}

/// The first glyph of the cluster starting exactly at byte `at`, if one does.
fn glyph_at(glyphs: &[PlacedGlyph], at: usize) -> Option<usize> {
    let index = glyphs.partition_point(|g| (g.cluster as usize) < at);
    (index > 0 && glyphs.get(index).is_some_and(|g| g.cluster as usize == at)).then_some(index)
}

fn sum(glyphs: &[PlacedGlyph]) -> Pt100 {
    glyphs.iter().map(|g| g.advance).sum()
}

/// A piece wider than `limit`, as boxes no wider than it (at least one cluster each) with a
/// break allowed between them.
fn chunks(out: &mut Items, index: usize, glyphs: &[PlacedGlyph], limit: Pt100) {
    let mut start = 0;
    let mut width = 0;
    for (g, glyph) in glyphs.iter().enumerate() {
        let boundary = g == 0 || glyphs[g - 1].cluster != glyph.cluster;
        let cluster: Pt100 = glyphs[g..]
            .iter()
            .take_while(|x| x.cluster == glyph.cluster)
            .map(|x| x.advance)
            .sum();
        if boundary && g > start && width + cluster > limit {
            out.push(Item::boxed(width, Seg::Part(index, start, g)), None);
            out.push(Item::penalty(0), None);
            start = g;
            width = 0;
        }
        width += glyph.advance;
    }
    out.push(
        Item::boxed(width, Seg::Part(index, start, glyphs.len())),
        None,
    );
}

fn word(
    ctx: &mut Ctx<'_, '_>,
    out: &mut Items,
    index: usize,
    piece: &Piece,
    setup: &Setup,
    found: &mut Vec<usize>,
) {
    let Piece::Text {
        text,
        style,
        width,
        glyphs,
        ..
    } = piece
    else {
        return;
    };
    let limit = if out.list.iter().any(Item::is_box) {
        setup.widths.1
    } else {
        setup.widths.0
    };
    if *width > limit {
        chunks(out, index, glyphs, limit);
        return;
    }
    let patterns = style.patterns.filter(|_| setup.hyphenate);
    let mut start = 0;
    for (at, kind) in points(ctx, text, patterns, found) {
        let Some(g) = glyph_at(glyphs, at) else {
            continue;
        };
        let hyphen = if kind == HyphenKind::Explicit {
            0
        } else {
            let entry = out.hyphens.entry(index).or_insert_with(|| {
                let (_, glyphs, width) = ctx.set("-", style);
                (glyphs, width)
            });
            entry.1
        };
        out.push(
            Item::boxed(sum(&glyphs[start..g]), Seg::Part(index, start, g)),
            None,
        );
        out.push(
            Item::Hyphen {
                width: hyphen,
                kind,
            },
            None,
        );
        start = g;
    }
    let seg = if start == 0 {
        Seg::Whole(index)
    } else {
        Seg::Part(index, start, glyphs.len())
    };
    out.push(Item::boxed(sum(&glyphs[start..]), seg), None);
}

/// The items for `pieces`, tabs `tabs[k]` wide (the k-th tab in order).
pub fn items(ctx: &mut Ctx<'_, '_>, pieces: &[Piece], setup: &Setup, tabs: &[Pt100]) -> Items {
    let mut out = Items {
        list: Vec::with_capacity(pieces.len() * 2 + 3),
        owner: Vec::with_capacity(pieces.len() * 2 + 3),
        hyphens: BTreeMap::new(),
    };
    let mut found = Vec::new();
    let mut tab = tabs.iter();
    let shrinks = matches!(setup.breaker, BreakerChoice::KnuthPlass(_)) && setup.justified;
    for (index, piece) in pieces.iter().enumerate() {
        match piece {
            Piece::Text { .. } => word(ctx, &mut out, index, piece, setup, &mut found),
            Piece::Widget { width, .. } | Piece::Field { width, .. } => {
                out.push(Item::boxed(*width, Seg::Whole(index)), None);
            }
            Piece::Space { width, .. } => {
                let shrink = if shrinks { width / 3 } else { 0 };
                out.push(Item::glue(*width, width / 2, shrink), Some(index));
            }
            Piece::Tab { .. } => {
                let width = tab.next().copied().unwrap_or(0);
                out.push(Item::penalty(0), None);
                out.push(Item::boxed(width, Seg::Tab(index)), None);
                out.push(Item::penalty(0), None);
            }
            Piece::Break { .. } => out.forced(Some(index)),
        }
    }
    out.forced(None);
    out
}
