//! The JSON shapes: `bench-results.json` and each competitor runner's file. Integers only (no
//! floats cross this boundary): nanoseconds, bytes, iteration counts.

use crate::stats::TimeSummary;
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
    /// A size measured on the same runner and corpus (memory).
    #[serde(rename = "measured bytes")]
    MeasuredBytes {
        /// The middle sample, bytes.
        median_bytes: u64,
        /// How many samples.
        iterations: u32,
    },
    /// A count of frames (scroll frames over 16.7 ms).
    #[serde(rename = "measured frames")]
    MeasuredFrames(FrameCount),
}

/// How many of a scroll's frames took longer than one 60 Hz frame (16.7 ms).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrameCount {
    /// Frames over 16.7 ms.
    pub over: u64,
    /// Every frame measured.
    pub of: u64,
}

/// Where a competitor file's numbers come from. No default: a file that does not say is refused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum Provenance {
    /// Measured in the same job as ours, on the same runner (the in-process and script
    /// competitors of `bench.yml`).
    #[serde(rename = "this run")]
    ThisRun,
    /// Ours in the browser (`competitors.yml`, the same job as the browser products), committed:
    /// where it ran, and exactly what was measured, the UI commit built and the engine commit its
    /// vendored cdoc-wasm came from.
    #[serde(rename = "recorded ours")]
    RecordedOurs {
        /// The runner, as it describes itself.
        runner: String,
        /// The browser.
        browser: String,
        /// The commit of this repository that ran it.
        commit: String,
        /// The day it ran, `YYYY-MM-DD` (UTC).
        date: String,
        /// The workflow run.
        run_url: String,
        /// The citadel-workspace-ui commit built.
        ui_commit: String,
        /// The citadel-docs commit of the cdoc-wasm that UI commit vendors.
        engine_commit: String,
    },
    /// Measured by another job (`competitors.yml`) and committed.
    #[serde(rename = "recorded")]
    Recorded {
        /// The runner, as it describes itself (image, CPU model, vCPUs).
        runner: String,
        /// The browser that drove the product.
        browser: String,
        /// The commit of this repository that ran it.
        commit: String,
        /// The day it ran, `YYYY-MM-DD` (UTC).
        date: String,
        /// The workflow run.
        run_url: String,
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
    /// Where the numbers come from.
    pub provenance: Provenance,
}

/// What an out-of-process runner (a script) writes: raw samples, summarised by
/// [`RawRun::summarize`](crate::raw) so every competitor's statistics are computed the way ours
/// are. Each map holds one unit; [`crate::raw::unit_of`] says which a metric belongs in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawRun {
    /// The competitor's name.
    pub tool: String,
    /// The version measured.
    pub version: String,
    /// Timed samples in nanoseconds by metric name.
    pub samples: BTreeMap<String, Vec<u64>>,
    /// Sizes in bytes by metric name.
    pub bytes: BTreeMap<String, Vec<u64>>,
    /// Frame counts by metric name.
    pub frames: BTreeMap<String, FrameCount>,
    /// Cells it does not measure, each with its reason.
    pub unmeasured: BTreeMap<String, CompetitorResult>,
    /// Where the numbers come from.
    pub provenance: Provenance,
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
    /// Where it comes from.
    pub provenance: Provenance,
}

/// `bench-results.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    /// The machine, as the runner describes itself.
    pub machine: String,
    /// The commit measured.
    pub commit: String,
    /// The runner's reference workload, median nanoseconds, timed before the metrics
    /// ([`crate::runner`]).
    pub runner_reference_ns: u64,
    /// Our metrics by name.
    pub metrics: BTreeMap<String, Measurement>,
    /// Competitor results by metric name, measured in this run (the engine rows).
    pub competitors: BTreeMap<String, Vec<CompetitorEntry>>,
    /// The Docs rows: ours in the browser and the browser products (`competitors.yml`); `null`
    /// in `run`'s output, filled by `report`.
    pub browser: Option<crate::docs_rows::BrowserRows>,
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
                        provenance: run.provenance.clone(),
                    });
            }
        }
    }
}
