//! An independent oracle: TeX's demerits written out again from `tex.web` §851 to §859, and the
//! optimum found by trying every pair of breakpoints (no active list). Shared by the optimality
//! property and the benchmark baseline. Uniform line width, one pass, looseness 0.

#![allow(dead_code)]

use cdoc_para::{badness, HyphenKind, Item, Order, Params, INFINITE_PENALTY, INF_BAD};

/// (penalty, width, flagged) of the breakpoint at `i`, if it is one.
pub fn breakpoint<T>(items: &[Item<T>], i: usize, p: &Params) -> Option<(i64, i64, bool)> {
    match &items[i] {
        Item::Glue { .. } => (i > 0 && items[i - 1].is_box()).then_some((0, 0, false)),
        Item::Penalty {
            width,
            penalty,
            flagged,
        } => (*penalty < INFINITE_PENALTY).then_some((
            i64::from((*penalty).max(-INFINITE_PENALTY)),
            i64::from(*width),
            *flagged,
        )),
        Item::Hyphen { width, kind } => {
            let pen = match kind {
                HyphenKind::Explicit => p.ex_hyphen_penalty,
                HyphenKind::Pattern | HyphenKind::Soft => p.hyphen_penalty,
            };
            (pen < INFINITE_PENALTY).then_some((i64::from(pen), i64::from(*width), true))
        }
        Item::Box { .. } => None,
    }
}

/// The first item of the line after a break at `b`: past glue, unforced penalties, hyphens.
pub fn start_after<T>(items: &[Item<T>], b: usize) -> usize {
    let mut j = b + 1;
    while j < items.len() {
        match &items[j] {
            Item::Box { .. } => break,
            Item::Penalty { penalty, .. } if *penalty <= -INFINITE_PENALTY => break,
            _ => j += 1,
        }
    }
    j
}

/// Running sums (width, finite stretch, shrink, infinite stretch) so a line measures in O(1).
pub struct Prefix(Vec<[i64; 4]>);

impl Prefix {
    pub fn new<T>(items: &[Item<T>]) -> Self {
        let mut acc = [0i64; 4];
        let mut out = vec![acc];
        for item in items {
            match item {
                Item::Box { width, .. } => acc[0] += i64::from(*width),
                Item::Glue {
                    width,
                    stretch,
                    shrink,
                } => {
                    acc[0] += i64::from(*width);
                    acc[2] += i64::from(*shrink);
                    let k = if stretch.order == Order::Normal { 1 } else { 3 };
                    acc[k] += i64::from(stretch.amount);
                }
                _ => {}
            }
            out.push(acc);
        }
        Self(out)
    }

    /// (badness, fitness 0..=3) of the line `[from, to)` plus `extra` width, at width `w`.
    pub fn fit(&self, from: usize, to: usize, extra: i64, w: i64) -> (i64, i32) {
        let (a, b) = (self.0[from.min(to)], self.0[to]);
        let (nat, st, sh, inf) = (b[0] - a[0] + extra, b[1] - a[1], b[2] - a[2], b[3] != a[3]);
        let short = w - nat;
        if short > 0 {
            if inf {
                return (0, 2);
            }
            if short > 7_230_584 && st < 1_663_497 {
                return (INF_BAD, 0);
            }
            let b = badness(short, st);
            (
                b,
                if b > 99 {
                    0
                } else if b > 12 {
                    1
                } else {
                    2
                },
            )
        } else if -short > sh {
            (INF_BAD + 1, 3)
        } else {
            let b = badness(-short, sh);
            (b, if b > 12 { 3 } else { 2 })
        }
    }
}

/// TeX's demerits for one line.
pub fn line_demerits(
    p: &Params,
    b: i64,
    pen: i64,
    flagged: bool,
    last: bool,
    fit: i32,
    prev: (i32, bool),
) -> i64 {
    let l = i64::from(p.line_penalty) + b;
    let mut d = if l.abs() >= 10_000 {
        100_000_000
    } else {
        l * l
    };
    if pen > 0 {
        d += pen * pen;
    } else if pen > -i64::from(INFINITE_PENALTY) {
        d -= pen * pen;
    }
    if prev.1 && (flagged || last) {
        d += if last {
            p.final_hyphen_demerits
        } else {
            p.double_hyphen_demerits
        };
    }
    if (fit - prev.0).abs() > 1 {
        d += p.adj_demerits;
    }
    d
}

/// The demerits of a given list of breaks, or `None` if a line is infeasible at `threshold`.
pub fn chain<T>(
    items: &[Item<T>],
    w: i64,
    p: &Params,
    threshold: i64,
    breaks: &[usize],
) -> Option<i64> {
    let sums = Prefix::new(items);
    let (mut total, mut prev, mut from) = (0i64, (2, false), 0usize);
    for &b in breaks {
        let (pen, extra, flagged) = breakpoint(items, b, p)?;
        let (bad, fit) = sums.fit(from, b, extra, w);
        if bad > threshold {
            return None;
        }
        total += line_demerits(p, bad, pen, flagged, b + 1 == items.len(), fit, prev);
        prev = (fit, flagged);
        from = start_after(items, b);
    }
    Some(total)
}

/// The fewest total demerits over every feasible break sequence, by dynamic programming over
/// (breakpoint, fitness of the line ending there). With `prune`, a start is abandoned after
/// its first overfull line or forced break, as TeX deactivates nodes; without, every pair is
/// tried (the O(n^2) baseline).
pub fn optimum<T>(
    items: &[Item<T>],
    w: i64,
    p: &Params,
    threshold: i64,
    prune: bool,
) -> Option<i64> {
    let n = items.len();
    let sums = Prefix::new(items);
    // best[i][f]: fewest demerits to a break at i whose line has fitness f; slot n = start.
    let mut best = vec![[None::<i64>; 4]; n + 1];
    best[n][2] = Some(0);
    let starts: Vec<usize> = std::iter::once(n)
        .chain((0..n).filter(|&i| breakpoint(items, i, p).is_some()))
        .collect();
    for (k, &i) in starts.iter().enumerate() {
        let from = if i == n { 0 } else { start_after(items, i) };
        let hyph = i != n && breakpoint(items, i, p).is_some_and(|b| b.2);
        for &j in &starts[k + 1..] {
            if i != n && j <= i {
                continue;
            }
            let (pen, extra, flagged) = breakpoint(items, j, p).expect("a breakpoint");
            let (bad, fit) = sums.fit(from, j, extra, w);
            let forced = pen <= -i64::from(INFINITE_PENALTY);
            if bad <= threshold {
                for f in 0..4 {
                    if let Some(t) = best[i][f] {
                        let prev = (i32::try_from(f).unwrap(), hyph);
                        let d = line_demerits(p, bad, pen, flagged, j + 1 == n, fit, prev);
                        let slot = &mut best[j][usize::try_from(fit).unwrap()];
                        *slot = Some(slot.map_or(t + d, |s| s.min(t + d)));
                    }
                }
            }
            if prune && (bad > INF_BAD || forced) {
                break;
            }
        }
    }
    best[n - 1].iter().flatten().copied().min()
}
