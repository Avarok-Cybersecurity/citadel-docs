// Adapted from rMazing crates/cc-export/src/layout/wrap.rs
// Copyright (c) Thomas Braun. Relicensed MIT OR Apache-2.0 by the copyright holder, 2026-10-04.

//! Word-wrap by measurement.
//!
//! On word boundaries, in the face and size each piece is set in, to the width the line has.
//! A word wider than the line on its own (a URL, routinely) is broken by character rather than
//! left to run past the margin, where it is simply gone from the page. Nothing is lost in the
//! breaking: the pieces concatenate back to the word.

use crate::fonts::Fonts;
use crate::ir::Pt100;
use crate::pieces::Piece;
use cdoc_model::BreakKind;

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
    /// The pieces.
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

fn width_of(piece: &Piece) -> Pt100 {
    match piece {
        Piece::Text { width, .. }
        | Piece::Space { width, .. }
        | Piece::Widget { width, .. }
        | Piece::Field { width, .. } => *width,
        Piece::Tab { .. } | Piece::Break(_) => 0,
    }
}

struct Builder<'f, 'a> {
    fonts: &'f Fonts<'a>,
    measure: Measure,
    lines: Vec<Line>,
    line: Line,
}

impl Builder<'_, '_> {
    fn fresh(&self, first: bool) -> Line {
        let (start, available) = if first {
            self.measure.first
        } else {
            self.measure.rest
        };
        Line {
            start,
            available: available.max(1),
            items: Vec::new(),
            width: 0,
            hard_end: false,
            break_after: None,
        }
    }

    fn x(&self) -> Pt100 {
        self.line.items.last().map_or(0, |i| i.x + i.width)
    }

    fn finish(&mut self, hard_end: bool, break_after: Option<BreakKind>) {
        let next = self.fresh(false);
        let mut done = std::mem::replace(&mut self.line, next);
        done.hard_end = hard_end;
        done.break_after = break_after;
        self.lines.push(done);
    }

    fn push(&mut self, piece: Piece, width: Pt100) {
        let x = self.x();
        if !matches!(piece, Piece::Space { .. }) {
            self.line.width = x + width;
        }
        self.line.items.push(Item { x, width, piece });
    }

    fn has_content(&self) -> bool {
        self.line.width > 0 || self.line.items.iter().any(|i| !is_space(&i.piece))
    }

    /// A word, which may be several differently styled pieces: it moves to the next line
    /// whole unless it is wider than a line by itself.
    fn word(&mut self, pieces: Vec<Piece>) {
        let total: Pt100 = pieces.iter().map(width_of).sum();
        if total > 0 && self.x() + total > self.line.available && self.has_content() {
            self.finish(false, None);
        }
        for piece in pieces {
            self.word_piece(piece);
        }
    }

    fn word_piece(&mut self, piece: Piece) {
        let width = width_of(&piece);
        if self.x() + width > self.line.available && self.has_content() {
            self.finish(false, None);
        }
        if width <= self.line.available - self.x() {
            self.push(piece, width);
            return;
        }
        let Piece::Text {
            text,
            style,
            link,
            note,
            ..
        } = piece
        else {
            // A box wider than the line: it gets a line of its own and overhangs.
            self.push(piece, width);
            return;
        };
        let mut taken = String::new();
        let mut taken_width = 0;
        for ch in text.chars() {
            let advance = self.fonts.advance(style.face, style.size, ch);
            if self.x() + taken_width + advance > self.line.available && !taken.is_empty() {
                let part = std::mem::take(&mut taken);
                let piece = Piece::Text {
                    text: part,
                    style: style.clone(),
                    width: taken_width,
                    link: link.clone(),
                    note,
                };
                self.push(piece, taken_width);
                self.finish(false, None);
                taken_width = 0;
            }
            taken.push(ch);
            taken_width += advance;
        }
        let piece = Piece::Text {
            text: taken,
            style,
            width: taken_width,
            link,
            note,
        };
        self.push(piece, taken_width);
    }
}

fn is_space(piece: &Piece) -> bool {
    matches!(piece, Piece::Space { .. })
}

/// Breaks pieces into lines. There is always at least one line, so an empty paragraph still
/// occupies its place.
#[must_use]
pub fn wrap(pieces: Vec<Piece>, fonts: &Fonts<'_>, measure: Measure) -> Vec<Line> {
    let mut b = Builder {
        fonts,
        measure,
        lines: Vec::new(),
        line: Line {
            start: 0,
            available: 0,
            items: Vec::new(),
            width: 0,
            hard_end: false,
            break_after: None,
        },
    };
    b.line = b.fresh(true);
    let mut word: Vec<Piece> = Vec::new();
    for piece in pieces {
        if matches!(
            piece,
            Piece::Text { .. } | Piece::Widget { .. } | Piece::Field { .. }
        ) {
            word.push(piece);
            continue;
        }
        b.word(std::mem::take(&mut word));
        match piece {
            Piece::Break(BreakKind::Line) => b.finish(true, None),
            Piece::Break(kind) => b.finish(true, Some(kind)),
            Piece::Space { .. } => {
                if !b.line.items.is_empty() || b.lines.is_empty() {
                    let width = width_of(&piece);
                    b.push(piece, width);
                }
            }
            Piece::Tab { .. } => {
                let absolute = b.line.start + b.x();
                let tab = measure.tab.max(1);
                let stop = (absolute.div_euclid(tab) + 1) * tab;
                if stop - b.line.start > b.line.available && b.has_content() {
                    b.finish(false, None);
                }
                let width = (stop - b.line.start - b.x()).max(0);
                b.push(piece, width);
            }
            word_piece => b.word(vec![word_piece]),
        }
    }
    b.word(word);
    b.finish(true, None);
    b.lines
}
