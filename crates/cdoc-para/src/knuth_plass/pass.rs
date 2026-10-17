//! One pass of the optimum-fit algorithm: TeX's `try_break` over every legal breakpoint, with
//! the active list pruned as soon as a line from a node can no longer fit (§829 to §860).

use crate::badness::{demerits, fit, Fitness, Previous, INF_BAD};
use crate::measure::{Breakpoint, Measured};
use crate::params::Params;
use crate::widths::LineWidths;

/// A feasible break that later lines may still start from.
#[derive(Debug, Clone, Copy)]
pub(super) struct Active {
    /// Index into the passive list; `None` is the paragraph's start.
    pub passive: Option<usize>,
    /// The item broken at; `None` is the paragraph's start.
    pub pos: Option<usize>,
    /// Lines before this break, so the zero-based number of the line it starts.
    pub line: usize,
    pub fitness: Fitness,
    pub hyphenated: bool,
    pub total: i64,
}

/// A break that was chosen as the best way to reach some active node.
#[derive(Debug, Clone, Copy)]
pub(super) struct Passive {
    pub pos: usize,
    pub prev: Option<usize>,
}

/// What a pass is allowed to do.
pub(super) struct Pass {
    pub threshold: i64,
    pub patterns: bool,
    pub background: i64,
}

/// The nodes that survive to the paragraph's end, and the break history behind them.
pub(super) struct Outcome {
    pub ends: Vec<Active>,
    pub passive: Vec<Passive>,
}

#[derive(Clone, Copy)]
struct Candidate {
    total: i64,
    from: Active,
}

struct State<'m, 'a, T> {
    m: &'m Measured<'a, T>,
    widths: &'m LineWidths,
    params: &'m Params,
    pass: &'m Pass,
    easy_line: usize,
    active: Vec<Active>,
    next: Vec<Active>,
    passive: Vec<Passive>,
}

impl<T> State<'_, '_, T> {
    fn class(&self, line: usize) -> usize {
        line.min(self.easy_line.saturating_add(1))
    }

    fn try_break(&mut self, at: usize, bp: &Breakpoint) {
        let last = at + 1 == self.m.len();
        let mut next = std::mem::take(&mut self.next);
        next.clear();
        let mut best: [Option<Candidate>; 4] = [None; 4];
        let mut group = None;
        for k in 0..self.active.len() {
            let a = self.active[k];
            let class = self.class(a.line);
            if group != Some(class) {
                self.flush(&mut best, at, bp, &mut next);
                group = Some(class);
            }
            let sums = self.m.line(self.m.line_start(a.pos), at, bp.width);
            let f = fit(&sums, self.widths.width(a.line), self.pass.background);
            if f.badness <= self.pass.threshold {
                let prev = Previous {
                    fitness: a.fitness,
                    hyphenated: a.hyphenated,
                };
                let d = demerits(self.params, f.badness, f.fitness, bp, prev, last);
                let total = a.total.saturating_add(d);
                let slot = &mut best[f.fitness as usize];
                if slot.is_none_or(|c| total <= c.total) {
                    *slot = Some(Candidate { total, from: a });
                }
            }
            if f.badness <= INF_BAD && !bp.forced() {
                next.push(a);
            }
        }
        self.flush(&mut best, at, bp, &mut next);
        self.next = std::mem::replace(&mut self.active, next);
    }

    /// Creates the new active nodes for one line-number class (TeX §835 to §845).
    fn flush(
        &mut self,
        best: &mut [Option<Candidate>; 4],
        at: usize,
        bp: &Breakpoint,
        next: &mut Vec<Active>,
    ) {
        let Some(minimum) = best.iter().flatten().map(|c| c.total).min() else {
            return;
        };
        let limit = minimum.saturating_add(self.params.adj_demerits.abs());
        for fitness in Fitness::ALL {
            if let Some(c) = best[fitness as usize].take() {
                if c.total <= limit {
                    self.passive.push(Passive {
                        pos: at,
                        prev: c.from.passive,
                    });
                    next.push(Active {
                        passive: Some(self.passive.len() - 1),
                        pos: Some(at),
                        line: c.from.line + 1,
                        fitness,
                        hyphenated: bp.flagged,
                        total: c.total,
                    });
                }
            }
        }
    }
}

/// Runs one pass; `None` when no feasible breaks reach the end.
pub(super) fn run<T>(
    m: &Measured<'_, T>,
    widths: &LineWidths,
    params: &Params,
    pass: &Pass,
) -> Option<Outcome> {
    let easy_line = if params.looseness == 0 {
        widths.last_special()
    } else {
        usize::MAX - 1
    };
    let mut state = State {
        m,
        widths,
        params,
        pass,
        easy_line,
        active: vec![Active {
            passive: None,
            pos: None,
            line: 0,
            fitness: Fitness::Decent,
            hyphenated: false,
            total: 0,
        }],
        next: Vec::new(),
        passive: Vec::new(),
    };
    for at in 0..m.len() {
        if let Some(bp) = m.breakpoint(at, params, pass.patterns) {
            state.try_break(at, &bp);
            if state.active.is_empty() {
                return None;
            }
        }
    }
    Some(Outcome {
        ends: state.active,
        passive: state.passive,
    })
}
