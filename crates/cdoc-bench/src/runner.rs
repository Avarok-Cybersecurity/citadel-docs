//! The runner's own speed, so a slow shared runner cannot fail a PR that changed nothing. Before
//! the metrics, `cdoc-bench run` times a fixed pure-CPU workload that no code of ours touches
//! ([`reference_workload`]); `bench/budgets.toml` records its median on a normal runner. When a
//! run's reference is slower than that by more than `slow_factor_bp`, every budget overrun in the
//! run is reported as "slow runner, not gated": visible in the table and the log, never silent.
//! Everything measured against the same machine in the same run stays gated: the competitor
//! comparisons (the release gate), the owner's ceilings, and missing or unknown budgets.
//!
//! The noise floor (`floor_ns`) is the least slack a timed budget allows, so a sub-microsecond
//! median cannot fail on tens of nanoseconds; above `floor_ns / tolerance` it changes nothing.

use crate::budget::Finding;
use crate::prng::SplitMix64;
use serde::Deserialize;
use std::hint::black_box;

/// The plan key of the reference workload.
pub const REFERENCE: &str = "runner_reference";
/// Rounds of the reference workload: a few milliseconds on the runner.
pub const REFERENCE_ROUNDS: u32 = 1_000_000;

/// The reference workload: `SplitMix64` folded with the FNV-1a multiply, `rounds` times. Integer
/// arithmetic only, no memory traffic, nothing of ours: it moves only when the CPU does.
#[must_use]
pub fn reference_workload(rounds: u32) -> u64 {
    let mut rng = SplitMix64::new(black_box(0x5245_4645_5245_4e43));
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for _ in 0..rounds {
        h = (h ^ rng.next_u64()).wrapping_mul(0x0100_0000_01b3);
    }
    black_box(h)
}

/// `[runner]` in `bench/budgets.toml`. No defaults: each number is written out with its reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Runner {
    /// The reference workload's median on a normal runner, nanoseconds.
    pub reference_ns: u64,
    /// How much slower (basis points of `reference_ns`; 12500 = 1.25 x) a run's reference may be
    /// before its budgets stop gating.
    pub slow_factor_bp: u64,
    /// The least slack of a timed budget, nanoseconds.
    pub floor_ns: u64,
}

/// Whether this run's budgets gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// The runner is within `slow_factor_bp` of the baseline.
    Gated {
        /// The run's reference median.
        measured_ns: u64,
    },
    /// The runner is slower than allowed: budget overruns are reported, not gated.
    SlowRunner {
        /// The run's reference median.
        measured_ns: u64,
    },
}

impl Runner {
    /// The verdict for a run whose reference took `measured_ns`.
    #[must_use]
    pub fn verdict(&self, measured_ns: u64) -> Verdict {
        let limit = u128::from(self.reference_ns) * u128::from(self.slow_factor_bp) / 10_000;
        if u128::from(measured_ns) > limit {
            Verdict::SlowRunner { measured_ns }
        } else {
            Verdict::Gated { measured_ns }
        }
    }
}

/// The gate's outcome: what fails the run, and what a slow runner waived.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gate {
    /// The run's verdict.
    pub verdict: Verdict,
    /// Findings that fail the run.
    pub red: Vec<Finding>,
    /// Budget overruns reported but not gated (slow runner only).
    pub waived: Vec<Finding>,
}

impl Gate {
    /// Splits `findings` by `verdict`: on a slow runner only budget overruns are waived.
    #[must_use]
    pub fn new(verdict: Verdict, findings: Vec<Finding>) -> Self {
        let (waived, red) = match verdict {
            Verdict::Gated { .. } => (Vec::new(), findings),
            Verdict::SlowRunner { .. } => findings
                .into_iter()
                .partition(|f| matches!(f, Finding::OverBudget { .. })),
        };
        Self {
            verdict,
            red,
            waived,
        }
    }

    /// Green: nothing fails the run.
    #[must_use]
    pub fn is_green(&self) -> bool {
        self.red.is_empty()
    }
}
