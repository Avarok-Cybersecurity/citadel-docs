//! A script's raw file to the shared competitor shape. A metric's unit follows from its family,
//! so a sample filed under the wrong unit is refused, as is a cell filed twice, an empty sample
//! set, and (for the browser protocol, `bench/competitors/README.md`) a cell left out or one the
//! protocol does not have. A refused file stops the run; nothing is guessed.

use crate::metrics::{paged, Family};
use crate::plan::{Plan, PlanError};
use crate::schema::{CompetitorResult, CompetitorRun, Provenance, RawRun};
use crate::stats::{summarize, Iterations};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

/// The browser-only family of `performance.measureUserAgentSpecificMemory()` bytes.
pub const MEMORY: &str = "memory";
/// The browser-only family of per-frame times while scrolling one viewport per frame.
pub const SCROLL: &str = "scroll";
/// The browser-only family counting the scroll's frames over 16.7 ms.
pub const SCROLL_LONG_FRAMES: &str = "scroll_long_frames";
/// The page counts the browser protocol opens.
pub const BROWSER_PAGES: [u32; 2] = [100, 500];
/// The page count the scroll and memory steps run on.
pub const BROWSER_LARGE: u32 = 500;

/// What a metric's numbers count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    /// Integer nanoseconds (`samples`).
    Nanoseconds,
    /// Bytes (`bytes`).
    Bytes,
    /// A count of frames (`frames`).
    Frames,
}

/// The unit of `metric`, from its family.
#[must_use]
pub fn unit_of(metric: &str) -> Unit {
    match metric.split_once('/').map_or(metric, |(family, _)| family) {
        MEMORY => Unit::Bytes,
        SCROLL_LONG_FRAMES => Unit::Frames,
        _ => Unit::Nanoseconds,
    }
}

/// Every cell of the browser protocol; a browser runner fills each, measured or with a reason.
#[must_use]
pub fn browser_cells() -> Vec<String> {
    let mut cells = Vec::new();
    for pages in BROWSER_PAGES {
        for family in [Family::DocxOpen, Family::LayoutKeystroke, Family::DocxToPdf] {
            cells.push(paged(family, pages));
        }
    }
    for family in [SCROLL, SCROLL_LONG_FRAMES, MEMORY] {
        cells.push(format!("{family}/{BROWSER_LARGE}p"));
    }
    cells
}

/// The plan of a browser runner: `tool/cell` for every cell it times (all but the long-frame
/// count, which is read off the scroll's own frames).
pub fn browser_plan(plan: &Plan, tool: &str) -> Result<BTreeMap<String, Iterations>, PlanError> {
    browser_cells()
        .into_iter()
        .filter(|cell| unit_of(cell) != Unit::Frames)
        .map(|cell| Ok((cell.clone(), plan.get(&format!("{tool}/{cell}"))?)))
        .collect()
}

/// What a browser runner reads: the iterations of every cell it times, and per page count the
/// phrase its Find lands on ([`crate::corpus::keystroke_phrase`]) and the paragraphs before the
/// keystroke paragraph ([`crate::corpus::keystroke_paragraph`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BrowserPlan {
    /// Iterations by cell.
    pub cells: BTreeMap<String, Iterations>,
    /// The keystroke phrase by page count (`100p`).
    pub phrases: BTreeMap<String, String>,
    /// The keystroke paragraph's reading-order index by page count.
    pub paragraphs: BTreeMap<String, usize>,
}

/// Which cells a raw file must hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellSet {
    /// Whatever the runner measures (the native script runners).
    Any,
    /// Exactly [`browser_cells`], from a browser product (`Provenance::Recorded`).
    Browser,
    /// Exactly [`browser_cells`], from ours in the browser (`Provenance::RecordedOurs`, both
    /// commits full ids).
    BrowserOurs,
}

impl CellSet {
    /// `any`, `browser` or `browser-ours`.
    pub fn parse(text: &str) -> Result<Self, String> {
        match text {
            "any" => Ok(Self::Any),
            "browser" => Ok(Self::Browser),
            "browser-ours" => Ok(Self::BrowserOurs),
            other => Err(format!(
                "cell set {other:?}: expected any, browser or browser-ours"
            )),
        }
    }

    /// Whether `p` is the provenance this set requires.
    fn admits(self, p: &Provenance) -> bool {
        let full = |c: &str| c.len() == 40 && c.bytes().all(|b| b.is_ascii_hexdigit());
        match (self, p) {
            (Self::Any, _) | (Self::Browser, Provenance::Recorded { .. }) => true,
            (
                Self::BrowserOurs,
                Provenance::RecordedOurs {
                    ui_commit,
                    engine_commit,
                    ..
                },
            ) => full(ui_commit) && full(engine_commit),
            _ => false,
        }
    }
}

/// Why a raw file was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RawError {
    /// A sample set with nothing in it.
    #[error("{0}: no samples")]
    Empty(String),
    /// A metric filed under another unit than its own.
    #[error("{metric}: filed as {found:?}, its unit is {expected:?}")]
    WrongUnit {
        /// The metric.
        metric: String,
        /// Where it was filed.
        found: Unit,
        /// Where it belongs.
        expected: Unit,
    },
    /// A metric filed in two places.
    #[error("{0}: filed twice")]
    Twice(String),
    /// A protocol cell neither measured nor explained.
    #[error("{0}: missing; measure it or say why it is not measured")]
    Missing(String),
    /// A cell the protocol does not have.
    #[error("{0}: not a cell of the browser protocol")]
    Unknown(String),
    /// An unmeasured cell with no reason, or a measurement filed as unmeasured: a blank.
    #[error("{0}: an unmeasured cell needs its reason")]
    Blank(String),
    /// More long frames than frames.
    #[error("{0}: more frames over 16.7 ms than frames")]
    Frames(String),
    /// A provenance the cell set does not take: a browser product as ours or ours as a product,
    /// or ours without the UI and engine commits it measured.
    #[error("{0}: its provenance does not say it was measured as this cell set requires")]
    Provenance(String),
}

fn file(
    seen: &mut BTreeSet<String>,
    metric: &str,
    found: Option<Unit>,
    empty: bool,
) -> Result<(), RawError> {
    if !seen.insert(metric.to_owned()) {
        return Err(RawError::Twice(metric.to_owned()));
    }
    let expected = unit_of(metric);
    match found {
        Some(found) if found != expected => Err(RawError::WrongUnit {
            metric: metric.to_owned(),
            found,
            expected,
        }),
        _ if empty => Err(RawError::Empty(metric.to_owned())),
        _ => Ok(()),
    }
}

impl RawRun {
    /// The run in the shared competitor shape, its statistics computed as ours are.
    pub fn summarize(&self, cells: CellSet) -> Result<CompetitorRun, RawError> {
        if !cells.admits(&self.provenance) {
            return Err(RawError::Provenance(self.tool.clone()));
        }
        let mut seen = BTreeSet::new();
        let mut results = BTreeMap::new();
        for (metric, samples) in &self.samples {
            file(
                &mut seen,
                metric,
                Some(Unit::Nanoseconds),
                samples.is_empty(),
            )?;
            let t = summarize(samples).map_err(|_| RawError::Empty(metric.clone()))?;
            results.insert(metric.clone(), CompetitorResult::Measured(t));
        }
        for (metric, samples) in &self.bytes {
            file(&mut seen, metric, Some(Unit::Bytes), samples.is_empty())?;
            let t = summarize(samples).map_err(|_| RawError::Empty(metric.clone()))?;
            let (median_bytes, iterations) = (t.median_ns, t.iterations);
            let cell = CompetitorResult::MeasuredBytes {
                median_bytes,
                iterations,
            };
            results.insert(metric.clone(), cell);
        }
        for (metric, count) in &self.frames {
            file(&mut seen, metric, Some(Unit::Frames), count.of == 0)?;
            if count.over > count.of {
                return Err(RawError::Frames(metric.clone()));
            }
            results.insert(metric.clone(), CompetitorResult::MeasuredFrames(*count));
        }
        for (metric, cell) in &self.unmeasured {
            file(&mut seen, metric, None, false)?;
            match cell {
                CompetitorResult::NotMeasured { reason }
                | CompetitorResult::NotComparable { reason }
                    if !reason.trim().is_empty() => {}
                _ => return Err(RawError::Blank(metric.clone())),
            }
            results.insert(metric.clone(), cell.clone());
        }
        if cells != CellSet::Any {
            let wanted: BTreeSet<String> = browser_cells().into_iter().collect();
            if let Some(extra) = seen.difference(&wanted).next() {
                return Err(RawError::Unknown(extra.clone()));
            }
            if let Some(missing) = wanted.difference(&seen).next() {
                return Err(RawError::Missing(missing.clone()));
            }
        }
        Ok(CompetitorRun {
            tool: self.tool.clone(),
            version: self.version.clone(),
            results,
            provenance: self.provenance.clone(),
        })
    }
}
