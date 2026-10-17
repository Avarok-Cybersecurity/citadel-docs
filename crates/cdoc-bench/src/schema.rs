//! The JSON shapes: `bench-results.json` and each competitor runner's file. Integers only (no
//! floats cross this boundary): nanoseconds, bytes, iteration counts.

use crate::stats::{summarize, StatsError, TimeSummary};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// One metric of ours: timed, or a size in bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Measurement {
    /// A timed metric.
    Time(TimeSummary),
    /// A size.
    Size {
        /// Bytes.
        bytes: u64,
    },
}

impl Measurement {
    /// The median in nanoseconds, for a timed metric.
    #[must_use]
    pub const fn median_ns(&self) -> Option<u64> {
        match self {
            Self::Time(t) => Some(t.median_ns),
            Self::Size { .. } => None,
        }
    }
}

/// A competitor's result on one metric. Never a blank: a cell either holds a measurement or
/// says why it does not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status")]
pub enum CompetitorResult {
    /// Measured on the same runner and corpus.
    #[serde(rename = "measured")]
    Measured(TimeSummary),
    /// Not measured yet (for example, it needs a browser); never read as a win.
    #[serde(rename = "not measured")]
    NotMeasured {
        /// Why, and what would measure it.
        reason: String,
    },
    /// The competitor has no equivalent operation.
    #[serde(rename = "not comparable")]
    NotComparable {
        /// Why.
        reason: String,
    },
}

/// What one competitor runner writes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompetitorRun {
    /// The competitor's name as the table shows it.
    pub tool: String,
    /// The pinned version measured.
    pub version: String,
    /// Results by metric name.
    pub results: BTreeMap<String, CompetitorResult>,
}

/// What an out-of-process runner (a script) writes: raw samples, summarised by
/// [`RawRun::summarize`] so every competitor's statistics are computed the way ours are.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawRun {
    /// The competitor's name.
    pub tool: String,
    /// The version measured.
    pub version: String,
    /// Timed samples in nanoseconds by metric name.
    pub samples: BTreeMap<String, Vec<u64>>,
    /// Cells it does not measure, each with its reason.
    pub unmeasured: BTreeMap<String, CompetitorResult>,
}

impl RawRun {
    /// The run in the shared competitor shape.
    pub fn summarize(&self) -> Result<CompetitorRun, StatsError> {
        let mut results = self.unmeasured.clone();
        for (metric, samples) in &self.samples {
            results.insert(
                metric.clone(),
                CompetitorResult::Measured(summarize(samples)?),
            );
        }
        Ok(CompetitorRun {
            tool: self.tool.clone(),
            version: self.version.clone(),
            results,
        })
    }
}

/// One competitor's cell for a metric in the report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompetitorEntry {
    /// The competitor.
    pub tool: String,
    /// Its version.
    pub version: String,
    /// The result.
    pub result: CompetitorResult,
}

/// `bench-results.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    /// The machine, as the runner describes itself.
    pub machine: String,
    /// The commit measured.
    pub commit: String,
    /// Our metrics by name.
    pub metrics: BTreeMap<String, Measurement>,
    /// Competitor results by metric name.
    pub competitors: BTreeMap<String, Vec<CompetitorEntry>>,
}

impl Report {
    /// Files every competitor run's results under their metrics, keeping the runs' order.
    pub fn add_competitors(&mut self, runs: &[CompetitorRun]) {
        for run in runs {
            for (metric, result) in &run.results {
                self.competitors
                    .entry(metric.clone())
                    .or_default()
                    .push(CompetitorEntry {
                        tool: run.tool.clone(),
                        version: run.version.clone(),
                        result: result.clone(),
                    });
            }
        }
    }
}
