//! Which paragraph unit each character of a run of text draws (see [`super::CaretLine`]):
//! spans from a character on, so a run of plain text is one span and costs no allocation.

/// What a span's characters are.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpanKind {
    /// The span's `k`-th character is unit `unit + k`.
    Each,
    /// Every character is unit `unit` (a field's value, a capital standing for one letter, the
    /// missing-glyph marker).
    Same,
    /// No unit (a list label, a footnote's own number, a line-end hyphen).
    Nothing,
}

/// From character `at` of the run on, up to the next span or the run's end.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UnitSpan {
    /// The first character, counted in the run's text.
    pub at: u32,
    /// The unit of that character.
    pub unit: u32,
    /// How the span goes on.
    pub kind: SpanKind,
}

/// A run's characters as units. `None` is a run that draws no unit at all.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Units {
    /// No character is a unit.
    None,
    /// One span from character 0.
    One(UnitSpan),
    /// Several spans, the first from character 0, `at` increasing (boxed, so a run's units
    /// stay two words wide: most runs are one span).
    Many(Box<Vec<UnitSpan>>),
}

impl UnitSpan {
    /// No unit, from the run's start.
    pub const NOTHING: Self = Self {
        at: 0,
        unit: 0,
        kind: SpanKind::Nothing,
    };

    /// Whether `next` says nothing this span does not already say.
    #[inline]
    const fn absorbs(&self, next: &Self) -> bool {
        match (self.kind, next.kind) {
            (SpanKind::Each, SpanKind::Each) => {
                next.unit.wrapping_sub(self.unit) == next.at.wrapping_sub(self.at)
            }
            (SpanKind::Same, SpanKind::Same) => self.unit == next.unit,
            (SpanKind::Nothing, SpanKind::Nothing) => true,
            _ => false,
        }
    }
}

impl Units {
    /// The spans, in order.
    #[must_use]
    #[inline]
    pub fn spans(&self) -> &[UnitSpan] {
        match self {
            Self::None => &[],
            Self::One(span) => std::slice::from_ref(span),
            Self::Many(spans) => spans.as_slice(),
        }
    }

    /// Characters from `span.at` on are as `span` says.
    #[inline]
    pub fn push(&mut self, span: UnitSpan) {
        match self {
            Self::None if span.kind == SpanKind::Nothing => {}
            Self::None if span.at == 0 => *self = Self::One(span),
            Self::None => *self = Self::Many(Box::new(vec![UnitSpan::NOTHING, span])),
            Self::One(last) if last.absorbs(&span) => {}
            Self::One(last) => *self = Self::Many(Box::new(vec![*last, span])),
            Self::Many(spans) => {
                if !spans.last().is_some_and(|l| l.absorbs(&span)) {
                    spans.push(span);
                }
            }
        }
    }

    /// `other`'s spans, for characters from `offset` on.
    pub fn append(&mut self, other: &Self, offset: u32) {
        let spans = other.spans();
        if spans.is_empty() {
            self.push(UnitSpan {
                at: offset,
                ..UnitSpan::NOTHING
            });
        }
        for s in spans {
            self.push(UnitSpan {
                at: s.at + offset,
                ..*s
            });
        }
    }

    /// Characters `from..to` as units of their own, counted from `from`.
    #[must_use]
    pub fn slice(&self, from: u32, to: u32) -> Self {
        let mut out = Self::None;
        let spans = self.spans();
        for (k, s) in spans.iter().enumerate() {
            let end = spans.get(k + 1).map_or(u32::MAX, |n| n.at);
            if end <= from || s.at >= to {
                continue;
            }
            let skip = from.saturating_sub(s.at);
            let unit = match s.kind {
                SpanKind::Each => s.unit + skip,
                SpanKind::Same | SpanKind::Nothing => s.unit,
            };
            out.push(UnitSpan {
                at: s.at.max(from) - from,
                unit,
                kind: s.kind,
            });
        }
        out
    }

    /// The same characters as units of a paragraph whose first unit is gone: every unit one
    /// less, unit 0 none.
    #[must_use]
    pub fn without_first(&self) -> Self {
        let mut out = Self::None;
        for s in self.spans() {
            let gone = s.unit == 0 && s.kind != SpanKind::Each;
            out.push(UnitSpan {
                unit: s.unit.saturating_sub(1),
                kind: if gone { SpanKind::Nothing } else { s.kind },
                ..*s
            });
        }
        out
    }

    /// The unit of each of `count` characters, `None` for those that are none.
    pub fn each(&self, count: u32) -> impl Iterator<Item = Option<u32>> + '_ {
        let spans = self.spans();
        let mut k = 0;
        (0..count).map(move |c| {
            while spans.get(k + 1).is_some_and(|n| n.at <= c) {
                k += 1;
            }
            let s = spans.get(k).filter(|s| s.at <= c)?;
            match s.kind {
                SpanKind::Each => Some(s.unit + (c - s.at)),
                SpanKind::Same => Some(s.unit),
                SpanKind::Nothing => None,
            }
        })
    }

    /// These units of `count` characters, after the `(k, n)`-th character was drawn as `n`
    /// characters.
    #[must_use]
    pub fn redrawn(&self, count: usize, wide: &[(usize, usize)]) -> Self {
        let mut out = Self::None;
        let mut at = 0u32;
        let mut wide = wide.iter().peekable();
        let count = u32::try_from(count).unwrap_or(u32::MAX);
        for (k, unit) in self.each(count).enumerate() {
            let n = match wide.peek() {
                Some(&&(w, n)) if w == k => {
                    wide.next();
                    n
                }
                _ => 1,
            };
            let kind = match (unit, n) {
                (None, _) => SpanKind::Nothing,
                (Some(_), 1) => SpanKind::Each,
                (Some(_), _) => SpanKind::Same,
            };
            out.push(UnitSpan {
                at,
                unit: unit.unwrap_or(0),
                kind,
            });
            at = at.saturating_add(u32::try_from(n).unwrap_or(u32::MAX));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::{SpanKind, UnitSpan, Units};

    const fn span(at: u32, unit: u32, kind: SpanKind) -> UnitSpan {
        UnitSpan { at, unit, kind }
    }

    #[test]
    fn plain_text_stays_one_span_and_slices_and_appends_keep_the_units() {
        let mut u = Units::None;
        for k in 0..5 {
            u.push(span(k, 10 + k, SpanKind::Each));
        }
        assert_eq!(u, Units::One(span(0, 10, SpanKind::Each)));
        u.push(span(5, 20, SpanKind::Same));
        u.push(span(8, 0, SpanKind::Nothing));
        let all: Vec<Option<u32>> = u.each(9).collect();
        assert_eq!(
            all,
            [10, 11, 12, 13, 14, 20, 20, 20]
                .map(Some)
                .into_iter()
                .chain([None])
                .collect::<Vec<_>>()
        );
        let mid = u.slice(3, 7);
        assert_eq!(
            mid.each(4).collect::<Vec<_>>(),
            [Some(13), Some(14), Some(20), Some(20)]
        );
        let mut joined = Units::None;
        joined.append(&Units::None, 0);
        joined.append(&mid, 2);
        assert_eq!(
            joined.each(6).collect::<Vec<_>>(),
            [None, None, Some(13), Some(14), Some(20), Some(20)]
        );
    }
}
