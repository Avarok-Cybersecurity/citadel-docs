//! Running totals over the items, so any candidate line is measured in constant time.

use crate::error::BreakError;
use crate::item::{HyphenKind, Item, Order, INFINITE_PENALTY};
use crate::params::Params;

/// Width, stretch per order and shrink, summed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Sums {
    pub width: i64,
    pub stretch: [i64; 4],
    pub shrink: i64,
}

impl Sums {
    const ZERO: Sums = Sums {
        width: 0,
        stretch: [0; 4],
        shrink: 0,
    };

    /// Whether there is stretch of an infinite order.
    pub fn infinite(&self) -> bool {
        Order::ALL[1..].iter().any(|o| self.stretch[o.index()] != 0)
    }

    fn minus(&self, other: &Sums) -> Sums {
        let mut stretch = [0; 4];
        for (k, s) in stretch.iter_mut().enumerate() {
            *s = self.stretch[k] - other.stretch[k];
        }
        Sums {
            width: self.width - other.width,
            stretch,
            shrink: self.shrink - other.shrink,
        }
    }
}

/// A legal breakpoint: what breaking there costs and adds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Breakpoint {
    /// The penalty; `<= -INFINITE_PENALTY` is forced.
    pub penalty: i32,
    /// Width typeset at the end of the line (a hyphen).
    pub width: i32,
    /// A hyphen, for the consecutive-hyphen demerits.
    pub flagged: bool,
}

impl Breakpoint {
    pub fn forced(&self) -> bool {
        self.penalty <= -INFINITE_PENALTY
    }
}

/// The items with their running totals.
pub(crate) struct Measured<'a, T> {
    pub items: &'a [Item<T>],
    prefix: Vec<Sums>,
    next_start: Vec<usize>,
}

fn discardable<T>(item: &Item<T>) -> bool {
    match item {
        Item::Box { .. } => false,
        Item::Penalty { penalty, .. } => *penalty > -INFINITE_PENALTY,
        Item::Glue { .. } | Item::Hyphen { .. } => true,
    }
}

impl<'a, T> Measured<'a, T> {
    /// Validates the items (non-negative flex, a forced break at the end) and sums them.
    pub fn new(items: &'a [Item<T>]) -> Result<Self, BreakError> {
        match items.last() {
            Some(Item::Penalty { penalty, .. }) if *penalty <= -INFINITE_PENALTY => {}
            _ => return Err(BreakError::Unterminated),
        }
        let mut prefix = Vec::with_capacity(items.len() + 1);
        let mut sum = Sums::ZERO;
        prefix.push(sum);
        for (index, item) in items.iter().enumerate() {
            match item {
                Item::Box { width, .. } => sum.width += i64::from(*width),
                Item::Glue {
                    width,
                    stretch,
                    shrink,
                } => {
                    if stretch.amount < 0 || *shrink < 0 {
                        return Err(BreakError::NegativeFlex { index });
                    }
                    sum.width += i64::from(*width);
                    sum.stretch[stretch.order.index()] += i64::from(stretch.amount);
                    sum.shrink += i64::from(*shrink);
                }
                Item::Penalty { .. } | Item::Hyphen { .. } => {}
            }
            prefix.push(sum);
        }
        let n = items.len();
        let mut next_start = vec![n; n];
        for i in (0..n.saturating_sub(1)).rev() {
            next_start[i] = if discardable(&items[i + 1]) {
                next_start[i + 1]
            } else {
                i + 1
            };
        }
        Ok(Self {
            items,
            prefix,
            next_start,
        })
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// The first item of the line after a break at `at` (`None`: the paragraph's start).
    pub fn line_start(&self, after: Option<usize>) -> usize {
        after.map_or(0, |a| self.next_start[a])
    }

    /// The breakpoint at item `i`, if it is one in a pass that does (or does not) consider
    /// pattern hyphens.
    pub fn breakpoint(&self, i: usize, params: &Params, patterns: bool) -> Option<Breakpoint> {
        match &self.items[i] {
            Item::Glue { .. } => (i > 0 && self.items[i - 1].is_box()).then_some(Breakpoint {
                penalty: 0,
                width: 0,
                flagged: false,
            }),
            Item::Penalty {
                width,
                penalty,
                flagged,
            } => (*penalty < INFINITE_PENALTY).then_some(Breakpoint {
                penalty: (*penalty).max(-INFINITE_PENALTY),
                width: *width,
                flagged: *flagged,
            }),
            Item::Hyphen { width, kind } => {
                let penalty = match kind {
                    HyphenKind::Pattern if !patterns => return None,
                    HyphenKind::Pattern | HyphenKind::Soft => params.hyphen_penalty,
                    HyphenKind::Explicit => params.ex_hyphen_penalty,
                };
                (penalty < INFINITE_PENALTY).then_some(Breakpoint {
                    penalty: penalty.max(-INFINITE_PENALTY),
                    width: *width,
                    flagged: true,
                })
            }
            Item::Box { .. } => None,
        }
    }

    /// The totals of the line from item `start` up to the break at `end` (which is not part of
    /// the line, except for a breakpoint's width).
    pub fn line(&self, start: usize, end: usize, break_width: i32) -> Sums {
        let start = start.min(end);
        let mut sums = self.prefix[end].minus(&self.prefix[start]);
        sums.width += i64::from(break_width);
        sums
    }
}
