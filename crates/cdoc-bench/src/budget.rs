//! The gate: `bench/budgets.toml` against a report. Every metric of ours needs a budget, every
//! budget needs a metric, nothing may exceed its budget (plus tolerance) or its owner ceiling,
//! and nothing of ours may be slower than a measured competitor on the same metric.

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
    /// An owner's hard limit that no tolerance stretches, when there is one.
    pub ceiling: Option<u64>,
}

impl Budget {
    /// The most the stat may be: `limit` plus `tolerance_bp`, capped by `ceiling`.
    #[must_use]
    pub fn allowed(&self) -> u64 {
        let slack = u128::from(self.limit) * u128::from(self.tolerance_bp) / 10_000;
        let allowed = u64::try_from(u128::from(self.limit) + slack).unwrap_or(u64::MAX);
        self.ceiling.map_or(allowed, |c| allowed.min(c))
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    budget: BTreeMap<String, Budget>,
}

/// Parses `bench/budgets.toml`.
pub fn parse(text: &str) -> Result<BTreeMap<String, Budget>, toml::de::Error> {
    Ok(toml::from_str::<File>(text)?.budget)
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

/// Every reason the report fails the budgets; empty when the gate is green.
#[must_use]
pub fn check(report: &Report, budgets: &BTreeMap<String, Budget>) -> Vec<Finding> {
    let mut findings = Vec::new();
    for (metric, measured) in &report.metrics {
        match budgets.get(metric) {
            None => findings.push(Finding::NoBudget(metric.clone())),
            Some(budget) => match stat_of(measured, budget.stat) {
                None => findings.push(Finding::WrongStat(metric.clone())),
                Some(value) if value > budget.allowed() => findings.push(Finding::OverBudget {
                    metric: metric.clone(),
                    value,
                    allowed: budget.allowed(),
                }),
                Some(_) => {}
            },
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
    for metric in budgets.keys() {
        if !report.metrics.contains_key(metric) {
            findings.push(Finding::UnknownMetric(metric.clone()));
        }
    }
    findings
}
