//! The paragraph as TeX sees it: boxes, glue and penalties.
//!
//! Widths are `i32` in the caller's unit (hundredths of a point in Citadel Docs). The breaker
//! never converts them, so any unit works as long as it is the same for every item and for
//! the line widths.

/// A penalty at or above this is never a break; at or below its negation the break is forced.
pub const INFINITE_PENALTY: i32 = 10_000;

/// The order of a glue's stretchability, as TeX's `fil`, `fill` and `filll`.
///
/// When a line contains stretch of an infinite order, only the glue of the highest such order
/// stretches and the line's badness is zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Order {
    /// Finite stretch, measured in the caller's unit.
    Normal,
    /// First-order infinite stretch (`fil`).
    Fil,
    /// Second-order infinite stretch (`fill`).
    Fill,
    /// Third-order infinite stretch (`filll`).
    Filll,
}

impl Order {
    /// Every order, lowest first.
    pub const ALL: [Order; 4] = [Order::Normal, Order::Fil, Order::Fill, Order::Filll];

    /// Position in [`Order::ALL`].
    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Order::Normal => 0,
            Order::Fil => 1,
            Order::Fill => 2,
            Order::Filll => 3,
        }
    }
}

/// How far a glue may stretch, and of which order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Stretch {
    /// The amount, in the caller's unit for [`Order::Normal`], in units of the order otherwise.
    pub amount: i32,
    /// The order.
    pub order: Order,
}

impl Stretch {
    /// Finite stretch.
    #[must_use]
    pub const fn finite(amount: i32) -> Self {
        Self {
            amount,
            order: Order::Normal,
        }
    }

    /// `fil` stretch, as `\parfillskip`'s `0pt plus 1fil`.
    #[must_use]
    pub const fn fil(amount: i32) -> Self {
        Self {
            amount,
            order: Order::Fil,
        }
    }
}

/// Where a hyphen break came from. It decides the penalty ([`crate::Params`] holds the
/// values, so they are not repeated on every item) and the pass the break is considered in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HyphenKind {
    /// Found by the hyphenator: `hyphen_penalty`, and only from the second pass on, as TeX
    /// hyphenates only when the first pass fails.
    Pattern,
    /// A discretionary hyphen the author placed (`\-`, U+00AD): `hyphen_penalty`, every pass.
    Soft,
    /// After a hyphen already in the text, which needs no glyph added: `ex_hyphen_penalty`,
    /// every pass.
    Explicit,
}

/// One element of a paragraph.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Item<T> {
    /// Material that is never broken or resized: a word, a word fragment, an image.
    Box {
        /// Width.
        width: i32,
        /// The caller's payload, carried through untouched.
        data: T,
    },
    /// Space that can stretch or shrink, and a legal break when it follows a box.
    Glue {
        /// Natural width.
        width: i32,
        /// Stretchability. Negative amounts are rejected.
        stretch: Stretch,
        /// Shrinkability; always finite (TeX refuses infinite shrink in a paragraph).
        /// Negative amounts are rejected.
        shrink: i32,
    },
    /// A possible break with a cost. `width` is typeset only when the line breaks here.
    Penalty {
        /// Width added at the end of a line broken here.
        width: i32,
        /// Cost; `>= INFINITE_PENALTY` forbids, `<= -INFINITE_PENALTY` forces the break.
        penalty: i32,
        /// Counts as a hyphen for the consecutive-hyphen demerits.
        flagged: bool,
    },
    /// A hyphenation point: TeX's discretionary as a flagged penalty whose width is the hyphen
    /// glyph's, typeset only when the line breaks here.
    Hyphen {
        /// Width of the hyphen glyph (zero after an explicit hyphen).
        width: i32,
        /// Origin of the break, which picks the penalty and the passes.
        kind: HyphenKind,
    },
}

impl<T> Item<T> {
    /// A word or other unbreakable material.
    #[must_use]
    pub const fn boxed(width: i32, data: T) -> Self {
        Item::Box { width, data }
    }

    /// Finite glue.
    #[must_use]
    pub const fn glue(width: i32, stretch: i32, shrink: i32) -> Self {
        Item::Glue {
            width,
            stretch: Stretch::finite(stretch),
            shrink,
        }
    }

    /// An unflagged penalty with no width.
    #[must_use]
    pub const fn penalty(penalty: i32) -> Self {
        Item::Penalty {
            width: 0,
            penalty,
            flagged: false,
        }
    }

    /// Whether this is a box.
    #[must_use]
    pub const fn is_box(&self) -> bool {
        matches!(self, Item::Box { .. })
    }
}

/// Ends a paragraph the way TeX's `\par` does: drops trailing glue, then appends
/// `\penalty10000 \hskip\parfillskip \penalty-10000` with `\parfillskip = 0pt plus 1fil`.
/// The breakers require the items to end in a forced break; this is the usual way to get one.
pub fn end_paragraph<T>(items: &mut Vec<Item<T>>) {
    while matches!(items.last(), Some(Item::Glue { .. })) {
        items.pop();
    }
    items.push(Item::penalty(INFINITE_PENALTY));
    items.push(Item::Glue {
        width: 0,
        stretch: Stretch::fil(1),
        shrink: 0,
    });
    items.push(Item::penalty(-INFINITE_PENALTY));
}
