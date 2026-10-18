//! Text as pieces: words, spaces, tabs and line breaks, capitalised as the style says, each
//! drawn character knowing the paragraph unit it comes from.

use super::Piece;
use crate::context::Ctx;
use crate::ir::{PlacedGlyph, Pt100, SpanKind, UnitSpan, Units};
use crate::style::{small_cap_size, Caps, TextStyle};
use cdoc_model::BreakKind;

/// Where a text's characters are in the paragraph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// Character `k` is unit `first + k`: a text run starting at unit `first`.
    Chars(u32),
    /// Every character is this one unit: a field's value.
    Atom(u32),
}

impl Source {
    fn unit(self, k: usize) -> u32 {
        match self {
            Self::Chars(first) => first.saturating_add(u32::try_from(k).unwrap_or(u32::MAX)),
            Self::Atom(unit) => unit,
        }
    }

    const fn kind(self) -> SpanKind {
        match self {
            Self::Chars(_) => SpanKind::Each,
            Self::Atom(_) => SpanKind::Same,
        }
    }
}

/// The cached space glyph of a style.
type Space = Option<(Vec<PlacedGlyph>, Pt100)>;

/// `text` as one word piece whose characters are `units`.
fn word_piece(
    ctx: &mut Ctx<'_, '_>,
    text: &str,
    units: Units,
    style: &TextStyle,
    link: Option<&String>,
    out: &mut Vec<Piece>,
) {
    let (text, units, chars, glyphs, width) = ctx.set_units(text, units, style);
    out.push(Piece::Text {
        chars,
        text,
        style: style.clone(),
        width,
        glyphs,
        link: link.cloned(),
        note: None,
        units,
    });
}

/// The piece `ch` is when it ends a word (a break, a tab or a breakable space), pushed to
/// `out`; `false` for a character of a word.
#[inline]
fn separator(
    ctx: &mut Ctx<'_, '_>,
    ch: char,
    unit: u32,
    style: &TextStyle,
    space: &mut Space,
    link: Option<&String>,
    out: &mut Vec<Piece>,
) -> bool {
    match ch {
        '\n' | '\u{2028}' => out.push(Piece::Break {
            kind: BreakKind::Line,
            unit,
        }),
        '\t' => out.push(Piece::Tab {
            style: style.clone(),
            unit,
        }),
        c if c.is_whitespace() && c != '\u{a0}' => {
            let (glyphs, width) = space.get_or_insert_with(|| {
                let (_, glyphs, width) = ctx.set(" ", style);
                (glyphs, width)
            });
            out.push(Piece::Space {
                width: *width,
                glyphs: glyphs.clone(),
                style: style.clone(),
                link: link.cloned(),
                unit,
            });
        }
        _ => return false,
    }
    true
}

#[inline]
fn is_separator(ch: char) -> bool {
    if ch.is_ascii() {
        return matches!(ch, ' ' | '\t' | '\n' | '\x0b' | '\x0c' | '\r');
    }
    ch == '\u{2028}' || (ch.is_whitespace() && ch != '\u{a0}')
}

/// `text`'s pieces in `style`, appended to `out`; returns how many characters `text` holds.
/// Small capitals set each lowercase letter capitalised at the reduced size, so a word changes
/// style (and is split) where its case does.
pub fn text_pieces(
    ctx: &mut Ctx<'_, '_>,
    text: &str,
    source: Source,
    style: &TextStyle,
    link: Option<&String>,
    out: &mut Vec<Piece>,
) -> u32 {
    if style.caps != Caps::AsTyped {
        return capitalised(ctx, text, source, style, link, out);
    }
    let span = |k: usize| {
        Units::One(UnitSpan {
            at: 0,
            unit: source.unit(k),
            kind: source.kind(),
        })
    };
    let mut space = None;
    let mut count = 0;
    // The byte and character index the word being gathered starts at.
    let mut word: Option<(usize, usize)> = None;
    for (k, (at, ch)) in text.char_indices().enumerate() {
        count = k + 1;
        if !is_separator(ch) {
            word.get_or_insert((at, k));
            continue;
        }
        if let Some((from, first)) = word.take() {
            word_piece(ctx, &text[from..at], span(first), style, link, out);
        }
        separator(ctx, ch, source.unit(k), style, &mut space, link, out);
    }
    if let Some((from, first)) = word {
        word_piece(ctx, &text[from..], span(first), style, link, out);
    }
    u32::try_from(count).unwrap_or(u32::MAX)
}

/// A word being gathered character by character: its characters as drawn and their units.
struct Word<'s> {
    style: &'s TextStyle,
    text: String,
    chars: u32,
    units: Units,
    /// The span the next character continues, when it is the next unit.
    next: Option<(u32, SpanKind)>,
    space: Space,
}

impl Word<'_> {
    fn push(&mut self, ch: char, unit: u32, kind: SpanKind) {
        if self.next != Some((unit, kind)) {
            self.units.push(UnitSpan {
                at: self.chars,
                unit,
                kind,
            });
        }
        self.chars += 1;
        let step = u32::from(kind == SpanKind::Each);
        self.next = Some((unit.wrapping_add(step), kind));
        self.text.push(ch);
    }

    fn flush(&mut self, ctx: &mut Ctx<'_, '_>, link: Option<&String>, out: &mut Vec<Piece>) {
        if self.text.is_empty() {
            return;
        }
        let units = std::mem::replace(&mut self.units, Units::None);
        word_piece(ctx, &self.text, units, self.style, link, out);
        self.text.clear();
        self.chars = 0;
        self.next = None;
    }
}

/// [`text_pieces`] for text set in capitals: each character as its capitals, at the reduced
/// size for a small capital.
fn capitalised(
    ctx: &mut Ctx<'_, '_>,
    text: &str,
    source: Source,
    style: &TextStyle,
    link: Option<&String>,
    out: &mut Vec<Piece>,
) -> u32 {
    let small = TextStyle {
        size: small_cap_size(style.size),
        ..style.clone()
    };
    let word = |style| Word {
        style,
        text: String::new(),
        chars: 0,
        units: Units::None,
        next: None,
        space: None,
    };
    let (mut full, mut reduced) = (word(style), word(&small));
    let mut count = 0;
    for (k, ch) in text.chars().enumerate() {
        count = k + 1;
        let unit = source.unit(k);
        let small_letter = style.caps == Caps::Small && ch.is_lowercase();
        let (word, other) = if small_letter {
            (&mut reduced, &mut full)
        } else {
            (&mut full, &mut reduced)
        };
        other.flush(ctx, link, out);
        if is_separator(ch) {
            word.flush(ctx, link, out);
            separator(ctx, ch, unit, word.style, &mut word.space, link, out);
            continue;
        }
        let upper = ch.to_uppercase();
        let kind = if upper.len() == 1 {
            source.kind()
        } else {
            SpanKind::Same
        };
        for u in upper {
            word.push(u, unit, kind);
        }
    }
    full.flush(ctx, link, out);
    reduced.flush(ctx, link, out);
    u32::try_from(count).unwrap_or(u32::MAX)
}
