//! Timing: warm up, run N iterations against a clock port, keep min, median and p95 as integer
//! nanoseconds. The clock is a port so the statistics stay pure; the harness binary hands in the
//! monotonic clock.

use serde::{Deserialize, Serialize};

/// Monotonic time in nanoseconds since an arbitrary origin.
pub trait Clock {
    /// Now.
    fn now_ns(&self) -> u64;
}

/// How often a metric runs. No `Default`: every metric names its own in `bench/plan.toml`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Iterations {
    /// Untimed runs first, so caches and the allocator settle.
    pub warmup: u32,
    /// Timed runs; at least one.
    pub iterations: u32,
}

/// What a timed metric reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimeSummary {
    /// The middle sample (the lower middle for an even count).
    pub median_ns: u64,
    /// The 95th percentile, nearest rank.
    pub p95_ns: u64,
    /// The fastest sample.
    pub min_ns: u64,
    /// How many samples.
    pub iterations: u32,
}

/// Why samples could not be summarised.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum StatsError {
    /// A summary of nothing.
    #[error("no samples")]
    Empty,
}

/// Min, median and p95 (nearest rank: the smallest sample at or above 95 % of them).
pub fn summarize(samples: &[u64]) -> Result<TimeSummary, StatsError> {
    if samples.is_empty() {
        return Err(StatsError::Empty);
    }
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    let n = sorted.len();
    let rank = (95 * n).div_ceil(100);
    Ok(TimeSummary {
        median_ns: sorted[(n - 1) / 2],
        p95_ns: sorted[rank.max(1) - 1],
        min_ns: sorted[0],
        iterations: u32::try_from(n).unwrap_or(u32::MAX),
    })
}

/// Runs `step` `warmup` times untimed, then `iterations` times, each timed on its own.
pub fn sample<E>(
    clock: &dyn Clock,
    plan: Iterations,
    mut step: impl FnMut() -> Result<(), E>,
) -> Result<TimeSummary, E>
where
    E: From<StatsError>,
{
    for _ in 0..plan.warmup {
        step()?;
    }
    let mut samples = Vec::with_capacity(usize::try_from(plan.iterations).unwrap_or(0));
    for _ in 0..plan.iterations {
        let start = clock.now_ns();
        step()?;
        samples.push(clock.now_ns().saturating_sub(start));
    }
    Ok(summarize(&samples)?)
}

/// `numerator / denominator` in basis points (10 000 = equal), rounded down; `None` when the
/// denominator is zero.
#[must_use]
pub fn basis_points(numerator: u64, denominator: u64) -> Option<u64> {
    (denominator != 0).then(|| {
        let bp = u128::from(numerator) * 10_000 / u128::from(denominator);
        u64::try_from(bp).unwrap_or(u64::MAX)
    })
}
