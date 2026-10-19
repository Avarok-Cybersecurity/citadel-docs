//! The gate: `bench/budgets.toml` against a report. Every metric of ours needs a budget, every
//! budget needs a metric, nothing may exceed its budget (plus tolerance) or its owner ceiling,
//! and nothing of ours may be slower than a measured competitor on the same metric.

use crate::runner::Runner;
use crate::schema::{CompetitorResult, Measurement, Report};
use crate::stats::basis_points;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fmt;

/// Which number of a metric a budget limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stat {
    /// The median.
    MedianNs,
    /// The 95th percentile.
    P95Ns,
    /// The size.
    Bytes,
}

impl Stat {
    /// How the table names it.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::MedianNs => "median",
            Self::P95Ns => "p95",
            Self::Bytes => "bytes",
        }
    }
}

/// One metric's budget. No `Default`: each number is written out in `bench/budgets.toml`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Budget {
    /// The number limited.
    pub stat: Stat,
    /// The limit, from a measured run on the CI runner.
    pub limit: u64,
    /// Allowed excess over `limit`, in basis points (2000 = 20 %).
    pub tolerance_bp: u64,
    /// An owner's hard limit, when there is one; checked on its own stat, never stretched.
    pub ceiling: Option<Ceiling>,
}

/// An owner's hard limit on one stat of a metric (for example: keystroke p95 under 8 ms).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ceiling {
    /// The number limited.
    pub stat: Stat,
    /// The most it may be.
    pub value: u64,
}

impl Budget {
    /// The most the stat may be: `limit` plus `tolerance_bp`, and for a time at least `limit`
    /// plus `floor_ns` (the noise floor; sizes are exact and never floored).
    #[must_use]
    pub fn allowed(&self, floor_ns: u64) -> u64 {
        let mut slack = u128::from(self.limit) * u128::from(self.tolerance_bp) / 10_000;
        if self.stat != Stat::Bytes {
            slack = slack.max(u128::from(floor_ns));
        }
        u64::try_from(u128::from(self.limit) + slack).unwrap_or(u64::MAX)
    }
}

/// `bench/budgets.toml`: the runner calibration and every metric's budget.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Budgets {
    /// The reference workload's baseline, the slow-runner factor and the noise floor.
    pub runner: Runner,
    /// Budgets by metric.
    pub budget: BTreeMap<String, Budget>,
}

/// Parses `bench/budgets.toml`.
pub fn parse(text: &str) -> Result<Budgets, toml::de::Error> {
    toml::from_str(text)
}

/// One reason the gate is red.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Finding {
    /// A metric without a budget.
    NoBudget(String),
    /// A budget for a metric the report does not have.
    UnknownMetric(String),
    /// A budget on a stat the metric does not have (bytes on a timing, or the reverse).
    WrongStat(String),
    /// Over budget.
    OverBudget {
        /// The metric.
        metric: String,
        /// What it measured.
        value: u64,
        /// What it may be.
        allowed: u64,
    },
    /// Over an owner's ceiling.
    OverCeiling {
        /// The metric.
        metric: String,
        /// The stat limited.
        stat: Stat,
        /// What it measured.
        value: u64,
        /// The ceiling.
        ceiling: u64,
    },
    /// Slower than a measured competitor (medians).
    SlowerThan {
        /// The metric.
        metric: String,
        /// The competitor.
        tool: String,
        /// Our median.
        ours_ns: u64,
        /// Theirs.
        theirs_ns: u64,
    },
    /// A browser product ahead of ours on a Docs row (`crate::docs_rows`).
    Behind(crate::docs_rows::Behind),
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoBudget(m) => write!(f, "no budget for {m}"),
            Self::UnknownMetric(m) => write!(f, "budget for unknown metric {m}"),
            Self::WrongStat(m) => write!(f, "budget for {m} limits a stat it does not have"),
            Self::OverBudget {
                metric,
                value,
                allowed,
            } => write!(f, "{metric} over budget: {value} > {allowed} allowed"),
            Self::OverCeiling {
                metric,
                stat,
                value,
                ceiling,
            } => write!(
                f,
                "{metric} over the owner's ceiling: {} {value} > {ceiling}",
                stat.label()
            ),
            Self::SlowerThan {
                metric,
                tool,
                ours_ns,
                theirs_ns,
            } => {
                let bp = basis_points(*ours_ns, *theirs_ns).unwrap_or(u64::MAX);
                write!(
                    f,
                    "{metric} slower than {tool}: ours {ours_ns} ns median vs {theirs_ns} ns ({bp} bp)"
                )
            }
            Self::Behind(behind) => write!(f, "{behind}"),
        }
    }
}

fn stat_of(m: &Measurement, stat: Stat) -> Option<u64> {
    match (m, stat) {
        (Measurement::Time(t), Stat::MedianNs) => Some(t.median_ns),
        (Measurement::Time(t), Stat::P95Ns) => Some(t.p95_ns),
        (Measurement::Size { bytes }, Stat::Bytes) => Some(*bytes),
        _ => None,
    }
}

fn check_one(
    metric: &str,
    measured: &Measurement,
    budget: &Budget,
    floor_ns: u64,
    findings: &mut Vec<Finding>,
) {
    let allowed = budget.allowed(floor_ns);
    match stat_of(measured, budget.stat) {
        None => findings.push(Finding::WrongStat(metric.to_owned())),
        Some(value) if value > allowed => findings.push(Finding::OverBudget {
            metric: metric.to_owned(),
            value,
            allowed,
        }),
        Some(_) => {}
    }
    let Some(ceiling) = budget.ceiling else {
        return;
    };
    match stat_of(measured, ceiling.stat) {
        None => findings.push(Finding::WrongStat(metric.to_owned())),
        Some(value) if value > ceiling.value => findings.push(Finding::OverCeiling {
            metric: metric.to_owned(),
            stat: ceiling.stat,
            value,
            ceiling: ceiling.value,
        }),
        Some(_) => {}
    }
}

/// Every reason the report fails the budgets; empty when the gate is green.
#[must_use]
pub fn check(report: &Report, budgets: &Budgets) -> Vec<Finding> {
    let floor_ns = budgets.runner.floor_ns;
    let mut findings = Vec::new();
    for (metric, measured) in &report.metrics {
        match budgets.budget.get(metric) {
            None => findings.push(Finding::NoBudget(metric.clone())),
            Some(budget) => check_one(metric, measured, budget, floor_ns, &mut findings),
        }
        let Some(ours_ns) = measured.median_ns() else {
            continue;
        };
        for entry in report.competitors.get(metric).into_iter().flatten() {
            if let CompetitorResult::Measured(theirs) = &entry.result {
                if ours_ns > theirs.median_ns {
                    findings.push(Finding::SlowerThan {
                        metric: metric.clone(),
                        tool: entry.tool.clone(),
                        ours_ns,
                        theirs_ns: theirs.median_ns,
                    });
                }
            }
        }
    }
    for metric in budgets.budget.keys() {
        if !report.metrics.contains_key(metric) {
            findings.push(Finding::UnknownMetric(metric.clone()));
        }
    }
    findings
}
