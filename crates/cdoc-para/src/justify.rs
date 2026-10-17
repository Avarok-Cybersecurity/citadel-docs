//! Setting a broken line's glue: the slack shared out in integers, so the widths add up to the
//! measure exactly whenever the line can reach it.

use crate::breaker::Line;
use crate::item::{Item, Order};

/// A justified line.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Justified {
    /// The width of each glue item in `items[line.start..line.end]`, in order.
    pub glue: Vec<i32>,
    /// Measure minus the set width: zero when the line was justified exactly, positive when
    /// it has too little stretch (underfull), negative when it could not shrink enough
    /// (overfull).
    pub residual: i64,
}

/// Shares `total` among weights in proportion, rounding by cumulative floors so the shares
/// are deterministic and sum to `total` exactly.
fn share(total: i64, weights: &[i64]) -> Vec<i64> {
    let sum: i128 = weights.iter().map(|w| i128::from(*w)).sum();
    let mut out = Vec::with_capacity(weights.len());
    let mut acc: i128 = 0;
    let mut given: i128 = 0;
    for w in weights {
        acc += i128::from(*w);
        let upto = if sum == 0 {
            0
        } else {
            (i128::from(total) * acc).div_euclid(sum)
        };
        out.push(i64::try_from(upto - given).unwrap_or(0));
        given = upto;
    }
    out
}

/// The glue widths that set `line` (from `items`) to `width`. Stretch goes to the glue of the
/// highest order present, in proportion to each glue's stretch; shrink in proportion to each
/// glue's shrink, never beyond it.
#[must_use]
pub fn justify<T>(items: &[Item<T>], line: &Line, width: i32) -> Justified {
    let end = line.end.min(items.len());
    let body = &items[line.start.min(end)..end];
    let mut natural: i64 = match items.get(line.end) {
        Some(Item::Penalty { width, .. } | Item::Hyphen { width, .. }) => i64::from(*width),
        _ => 0,
    };
    let mut glue = Vec::new();
    let mut order = Order::Normal;
    for item in body {
        match item {
            Item::Box { width, .. } => natural += i64::from(*width),
            Item::Glue {
                width,
                stretch,
                shrink,
            } => {
                natural += i64::from(*width);
                if stretch.amount > 0 {
                    order = order.max(stretch.order);
                }
                glue.push((*width, *stretch, *shrink));
            }
            Item::Penalty { .. } | Item::Hyphen { .. } => {}
        }
    }
    let shortfall = i64::from(width) - natural;
    let (weights, amount): (Vec<i64>, i64) = if shortfall > 0 {
        let weights: Vec<i64> = glue
            .iter()
            .map(|(_, s, _)| {
                if s.order == order {
                    i64::from(s.amount)
                } else {
                    0
                }
            })
            .collect();
        let any = weights.iter().any(|w| *w > 0);
        (weights, if any { shortfall } else { 0 })
    } else {
        let weights: Vec<i64> = glue.iter().map(|(_, _, z)| i64::from(*z)).collect();
        let total: i64 = weights.iter().sum();
        (weights, -((-shortfall).min(total)))
    };
    let shares = share(amount, &weights);
    let glue = glue
        .iter()
        .zip(shares)
        .map(|((w, _, _), s)| i32::try_from(i64::from(*w) + s).unwrap_or(*w))
        .collect();
    Justified {
        glue,
        residual: shortfall - amount,
    }
}
