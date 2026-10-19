//! The Docs rows: ours, measured in the browser by the same protocol as the browser products
//! (`bench/competitors/README.md`), against each of them. A product beats us on a row when its
//! median, its p95, its memory or its share of long scroll frames is lower than ours, or when it
//! measured a cell we did not; either turns the gate red. Only numbers from the same workflow run
//! on the same runner are compared: a pair from two machines is itself a red finding, never a
//! comparison.

use crate::raw::browser_cells;
use crate::schema::{CompetitorResult, CompetitorRun, FrameCount, Provenance};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Ours in the browser, and the browser products, as committed under `bench/competitors/results`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrowserRows {
    /// Ours (`Provenance::RecordedOurs`).
    pub ours: CompetitorRun,
    /// The products (`Provenance::Recorded`), in file order.
    pub products: Vec<CompetitorRun>,
}

/// How a product is ahead of us on one cell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Lag {
    /// Our median is higher.
    Median {
        /// Ours, ns.
        ours: u64,
        /// Theirs, ns.
        theirs: u64,
    },
    /// Our p95 is higher.
    P95 {
        /// Ours, ns.
        ours: u64,
        /// Theirs, ns.
        theirs: u64,
    },
    /// We use more memory.
    Bytes {
        /// Ours.
        ours: u64,
        /// Theirs.
        theirs: u64,
    },
    /// A larger share of our scroll frames is over 16.7 ms.
    LongFrames {
        /// Ours.
        ours: FrameCount,
        /// Theirs.
        theirs: FrameCount,
    },
    /// They measured the cell; we did not.
    Unmeasured,
    /// The two numbers come from different runs or runners, so they are not compared.
    OtherMachine {
        /// Our run.
        ours: String,
        /// Theirs.
        theirs: String,
    },
}

/// One Docs-row finding: on `cell`, `tool` is ahead of us.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Behind {
    /// The protocol cell.
    pub cell: String,
    /// The product.
    pub tool: String,
    /// How.
    pub lag: Lag,
}

impl fmt::Display for Behind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (cell, tool) = (&self.cell, &self.tool);
        match &self.lag {
            Lag::Median { ours, theirs } => {
                write!(
                    f,
                    "{cell} in the browser: {tool} median {theirs} ns < ours {ours} ns"
                )
            }
            Lag::P95 { ours, theirs } => {
                write!(
                    f,
                    "{cell} in the browser: {tool} p95 {theirs} ns < ours {ours} ns"
                )
            }
            Lag::Bytes { ours, theirs } => {
                write!(
                    f,
                    "{cell} in the browser: {tool} {theirs} B < ours {ours} B"
                )
            }
            Lag::LongFrames { ours, theirs } => write!(
                f,
                "{cell} in the browser: {tool} {} of {} frames over 16.7 ms, a smaller share than \
                 ours {} of {}",
                theirs.over, theirs.of, ours.over, ours.of
            ),
            Lag::Unmeasured => write!(
                f,
                "{cell}: {tool} measured it in the browser and we did not"
            ),
            Lag::OtherMachine { ours, theirs } => write!(
                f,
                "{cell}: ours ({ours}) and {tool} ({theirs}) were not measured in the same run, so \
                 they are not compared; record both from one competitors.yml run"
            ),
        }
    }
}

/// The run and runner a browser number was recorded on.
fn origin(p: &Provenance) -> Option<(&str, &str)> {
    match p {
        Provenance::Recorded {
            runner, run_url, ..
        }
        | Provenance::RecordedOurs {
            runner, run_url, ..
        } => Some((run_url, runner)),
        Provenance::ThisRun => None,
    }
}

const fn measured(r: &CompetitorResult) -> bool {
    !matches!(
        r,
        CompetitorResult::NotMeasured { .. } | CompetitorResult::NotComparable { .. }
    )
}

fn compare(ours: &CompetitorResult, theirs: &CompetitorResult) -> Vec<Lag> {
    let mut lags = Vec::new();
    match (ours, theirs) {
        (CompetitorResult::Measured(o), CompetitorResult::Measured(t)) => {
            if o.median_ns > t.median_ns {
                lags.push(Lag::Median {
                    ours: o.median_ns,
                    theirs: t.median_ns,
                });
            }
            if o.p95_ns > t.p95_ns {
                lags.push(Lag::P95 {
                    ours: o.p95_ns,
                    theirs: t.p95_ns,
                });
            }
        }
        (
            CompetitorResult::MeasuredBytes {
                median_bytes: o, ..
            },
            CompetitorResult::MeasuredBytes {
                median_bytes: t, ..
            },
        ) => {
            if o > t {
                lags.push(Lag::Bytes {
                    ours: *o,
                    theirs: *t,
                });
            }
        }
        (CompetitorResult::MeasuredFrames(o), CompetitorResult::MeasuredFrames(t)) => {
            if u128::from(o.over) * u128::from(t.of) > u128::from(t.over) * u128::from(o.of) {
                lags.push(Lag::LongFrames {
                    ours: *o,
                    theirs: *t,
                });
            }
        }
        _ => lags.push(Lag::Unmeasured),
    }
    lags
}

/// Every Docs row a product is ahead of us on; empty when ours leads every measured cell.
#[must_use]
pub fn check(rows: &BrowserRows) -> Vec<Behind> {
    let mut found = Vec::new();
    for cell in browser_cells() {
        for product in &rows.products {
            let Some(theirs) = product.results.get(&cell).filter(|r| measured(r)) else {
                continue;
            };
            let mut push = |lag| {
                found.push(Behind {
                    cell: cell.clone(),
                    tool: product.tool.clone(),
                    lag,
                });
            };
            let Some(ours) = rows.ours.results.get(&cell).filter(|r| measured(r)) else {
                push(Lag::Unmeasured);
                continue;
            };
            let (o, t) = (origin(&rows.ours.provenance), origin(&product.provenance));
            if o.is_none() || o != t {
                let run =
                    |x: Option<(&str, &str)>| x.map_or("not recorded", |(url, _)| url).to_owned();
                push(Lag::OtherMachine {
                    ours: run(o),
                    theirs: run(t),
                });
                continue;
            }
            compare(ours, theirs).into_iter().for_each(push);
        }
    }
    found
}

/// Splits the competitor files by provenance: this run's (the engine rows), and the browser rows.
/// Browser products need ours beside them; two runs of ours are refused.
pub fn split(
    runs: Vec<CompetitorRun>,
) -> Result<(Vec<CompetitorRun>, Option<BrowserRows>), String> {
    let (mut native, mut products, mut ours) = (Vec::new(), Vec::new(), None);
    for run in runs {
        match run.provenance {
            Provenance::ThisRun => native.push(run),
            Provenance::Recorded { .. } => products.push(run),
            Provenance::RecordedOurs { .. } if ours.is_some() => {
                return Err(format!("{}: a second browser run of ours", run.tool));
            }
            Provenance::RecordedOurs { .. } => ours = Some(run),
        }
    }
    match ours {
        Some(ours) => Ok((native, Some(BrowserRows { ours, products }))),
        None if products.is_empty() => Ok((native, None)),
        None => Err("browser products are recorded but ours is not \
                     (bench/competitors/results/ours.json, from the same competitors.yml run)"
            .to_owned()),
    }
}
