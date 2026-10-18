//! TeX's badness, fitness classes and demerits, in integers exactly as `tex.web` computes them
//! (§108, §851 to §859), so a line scores the same on every target.

use crate::measure::{Breakpoint, Sums};
use crate::params::Params;

/// The badness ceiling: a line this bad or worse is "infinitely bad".
pub const INF_BAD: i64 = 10_000;

/// One in [`RATIO_ONE`] is how adjustment ratios are expressed (TeX's `glue_set` scale).
pub const RATIO_ONE: i32 = 65_536;

const AWFUL: i64 = 100_000_000;

/// TeX's fitness classes, ordered as `tex.web` numbers them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Fitness {
    /// Badness above 99 from stretching.
    VeryLoose = 0,
    /// Badness 13 to 99 from stretching.
    Loose = 1,
    /// Badness 12 or less.
    Decent = 2,
    /// Badness above 12 from shrinking.
    Tight = 3,
}

impl Fitness {
    /// Every class, in `tex.web`'s order.
    pub const ALL: [Fitness; 4] = [
        Fitness::VeryLoose,
        Fitness::Loose,
        Fitness::Decent,
        Fitness::Tight,
    ];

    fn distance(self, other: Fitness) -> i32 {
        (self as i32 - other as i32).abs()
    }
}

/// TeX's `badness(t, s)`: about `100 (t/s)^3`, capped at [`INF_BAD`].
#[must_use]
pub fn badness(t: i64, s: i64) -> i64 {
    if t == 0 {
        return 0;
    }
    if s <= 0 {
        return INF_BAD;
    }
    let r = if t <= 7_230_584 {
        t * 297 / s
    } else if s >= 1_663_497 {
        t / (s / 297)
    } else {
        t
    };
    if r > 1290 {
        INF_BAD
    } else {
        (r * r * r + 0x2_0000) / 0x4_0000
    }
}

/// How a line of the given totals fits a width.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fit {
    /// Badness; [`INF_BAD`]` + 1` when it cannot shrink enough (overfull).
    pub badness: i64,
    /// Fitness class.
    pub fitness: Fitness,
    /// What the glue-set ratio is computed from, only when a line is kept (the breakers try
    /// many more lines than they keep).
    setting: Setting,
}

/// A fit's glue setting before the division: stretched by `t` over `s`, shrunk by `t` over
/// `s`, or set exactly at a fixed ratio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Setting {
    Stretch(i64, i64),
    Shrink(i64, i64),
    Fixed(i32),
}

impl Fit {
    /// The ratio finite glue is set at, in 1/[`RATIO_ONE`]: positive stretches, negative
    /// shrinks (never below -1 unless overfull, when it is exactly -1).
    #[must_use]
    pub fn ratio(&self) -> i32 {
        match self.setting {
            Setting::Stretch(t, s) => ratio(t, s),
            Setting::Shrink(t, s) => -ratio(t, s),
            Setting::Fixed(r) => r,
        }
    }
}

fn ratio(t: i64, s: i64) -> i32 {
    if s <= 0 {
        return 0;
    }
    // Nearest, halves away from zero; `t` is never negative here. Real lengths keep the
    // products well inside 64 bits; 128 is the exact fallback for any that do not.
    let wide = || {
        let r = (2 * i128::from(t) * i128::from(RATIO_ONE) + i128::from(s)) / (2 * i128::from(s));
        i32::try_from(r).unwrap_or(if r < 0 { i32::MIN } else { i32::MAX })
    };
    let narrow = t
        .checked_mul(2 * i64::from(RATIO_ONE))
        .and_then(|n| n.checked_add(s))
        .zip(s.checked_mul(2))
        .map(|(n, d)| n / d);
    narrow.map_or_else(wide, |r| {
        i32::try_from(r).unwrap_or(if r < 0 { i32::MIN } else { i32::MAX })
    })
}

/// Fits a line of totals `sums` (plus `background` finite stretch, TeX's emergency stretch)
/// to `width`.
#[must_use]
pub fn fit(sums: &Sums, width: i32, background: i64) -> Fit {
    let shortfall = i64::from(width) - sums.width;
    if shortfall > 0 {
        if sums.infinite() {
            return Fit {
                badness: 0,
                fitness: Fitness::Decent,
                setting: Setting::Fixed(0),
            };
        }
        let stretch = sums.stretch[0] + background;
        let setting = Setting::Stretch(shortfall, sums.stretch[0]);
        if shortfall > 7_230_584 && stretch < 1_663_497 {
            return Fit {
                badness: INF_BAD,
                fitness: Fitness::VeryLoose,
                setting,
            };
        }
        let badness = badness(shortfall, stretch);
        let fitness = match badness {
            b if b > 99 => Fitness::VeryLoose,
            b if b > 12 => Fitness::Loose,
            _ => Fitness::Decent,
        };
        return Fit {
            badness,
            fitness,
            setting,
        };
    }
    let over = -shortfall;
    if over > sums.shrink {
        return Fit {
            badness: INF_BAD + 1,
            fitness: Fitness::Tight,
            setting: Setting::Fixed(-RATIO_ONE),
        };
    }
    let badness = badness(over, sums.shrink);
    Fit {
        badness,
        fitness: if badness > 12 {
            Fitness::Tight
        } else {
            Fitness::Decent
        },
        setting: Setting::Shrink(over, sums.shrink),
    }
}

/// What the line before a candidate break ended like.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Previous {
    /// Its fitness class.
    pub fitness: Fitness,
    /// Whether it ended in a hyphen.
    pub hyphenated: bool,
}

/// TeX's demerits for a line of badness `badness` ending at `at`, after a line like `prev`;
/// `last` is whether `at` ends the paragraph (where `\finalhyphendemerits` applies).
#[must_use]
pub fn demerits(
    params: &Params,
    badness: i64,
    fitness: Fitness,
    at: &Breakpoint,
    prev: Previous,
    last: bool,
) -> i64 {
    let base = i64::from(params.line_penalty) + badness;
    let mut d = if base.abs() >= 10_000 {
        AWFUL
    } else {
        base * base
    };
    let pi = i64::from(at.penalty);
    if pi > 0 {
        d += pi * pi;
    } else if !at.forced() {
        d -= pi * pi;
    }
    if prev.hyphenated && (at.flagged || last) {
        d = d.saturating_add(if last {
            params.final_hyphen_demerits
        } else {
            params.double_hyphen_demerits
        });
    }
    if fitness.distance(prev.fitness) > 1 {
        d = d.saturating_add(params.adj_demerits);
    }
    d
}
