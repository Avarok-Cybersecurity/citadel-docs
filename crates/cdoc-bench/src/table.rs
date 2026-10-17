//! The markdown report: one row per metric of ours with its budget and every competitor's cell,
//! then the gate's findings. A competitor that was not measured says so; no cell is blank.

use crate::budget::{Budget, Finding, Stat};
use crate::schema::{CompetitorEntry, CompetitorResult, Measurement, Report};
use crate::stats::basis_points;
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// The marker the sticky PR comment is found by.
pub const COMMENT_MARKER: &str = "<!-- cdoc-bench-report -->";

/// Nanoseconds for a human: `850 ns`, `12.3 µs`, `4.56 ms`, `1.234 s` (integer arithmetic).
#[must_use]
pub fn human_ns(ns: u64) -> String {
    match ns {
        0..1_000 => format!("{ns} ns"),
        1_000..1_000_000 => format!("{}.{} µs", ns / 1_000, ns % 1_000 / 100),
        1_000_000..1_000_000_000 => format!("{}.{:02} ms", ns / 1_000_000, ns % 1_000_000 / 10_000),
        _ => format!(
            "{}.{:03} s",
            ns / 1_000_000_000,
            ns % 1_000_000_000 / 1_000_000
        ),
    }
}

fn human(value: u64, stat: Stat) -> String {
    match stat {
        Stat::Bytes => format!("{value} B"),
        Stat::MedianNs | Stat::P95Ns => human_ns(value),
    }
}

fn competitor_cell(entry: &CompetitorEntry, ours_ns: Option<u64>) -> String {
    match &entry.result {
        CompetitorResult::Measured(t) => {
            let ratio = ours_ns
                .and_then(|o| basis_points(o, t.median_ns))
                .map_or_else(String::new, |bp| format!(", ours {bp} bp"));
            format!("{} {}{ratio}", entry.tool, human_ns(t.median_ns))
        }
        CompetitorResult::NotMeasured { .. } => format!("{}: not measured", entry.tool),
        CompetitorResult::NotComparable { .. } => format!("{}: not comparable", entry.tool),
    }
}

/// Family first, then the size as a number, so `10p` follows `1p` and precedes `100p`.
fn reading_order(name: &str) -> (&str, u64) {
    let (family, size) = name.split_once('/').unwrap_or((name, ""));
    let digits: String = size.chars().take_while(char::is_ascii_digit).collect();
    (family, digits.parse().unwrap_or(0))
}

fn mentions(finding: &Finding, metric: &str) -> bool {
    match finding {
        Finding::NoBudget(m) | Finding::UnknownMetric(m) | Finding::WrongStat(m) => m == metric,
        Finding::OverBudget { metric: m, .. } | Finding::SlowerThan { metric: m, .. } => {
            m == metric
        }
    }
}

fn ours_row(
    out: &mut String,
    name: &str,
    m: &Measurement,
    budget: Option<&Budget>,
    report: &Report,
    findings: &[Finding],
) {
    let (median, p95) = match m {
        Measurement::Time(t) => (human_ns(t.median_ns), human_ns(t.p95_ns)),
        Measurement::Size { bytes } => (format!("{bytes} B"), "-".to_owned()),
    };
    let budget = budget.map_or_else(
        || "none".to_owned(),
        |b| format!("{} <= {}", b.stat.label(), human(b.allowed(), b.stat)),
    );
    let cells: Vec<String> = report
        .competitors
        .get(name)
        .into_iter()
        .flatten()
        .map(|e| competitor_cell(e, m.median_ns()))
        .collect();
    let competitors = if cells.is_empty() {
        "-".to_owned()
    } else {
        cells.join("; ")
    };
    let verdict = if findings.iter().any(|f| mentions(f, name)) {
        "**RED**"
    } else {
        "ok"
    };
    let _ = writeln!(
        out,
        "| `{name}` | {median} | {p95} | {budget} | {competitors} | {verdict} |"
    );
}

/// The whole report as markdown, headed by [`COMMENT_MARKER`].
#[must_use]
pub fn render(report: &Report, budgets: &BTreeMap<String, Budget>, findings: &[Finding]) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "{COMMENT_MARKER}\n## cdoc-bench\n");
    let _ = writeln!(
        out,
        "Machine: {}. Commit: `{}`. Integers in, rounded for reading; ratios are ours over theirs \
         in basis points (10000 = equal, lower is faster).\n",
        report.machine, report.commit
    );
    let _ = writeln!(
        out,
        "| Metric | Ours median | Ours p95 | Budget | Competitors | Gate |\n|---|---|---|---|---|---|"
    );
    let mut rows: Vec<_> = report.metrics.iter().collect();
    rows.sort_by_key(|(name, _)| reading_order(name));
    for (name, m) in rows {
        ours_row(&mut out, name, m, budgets.get(name), report, findings);
    }
    let theirs_only: Vec<_> = report
        .competitors
        .iter()
        .filter(|(name, _)| !report.metrics.contains_key(*name))
        .collect();
    if !theirs_only.is_empty() {
        let _ = writeln!(out, "\n### Competitor cells with no metric of ours\n");
        let _ = writeln!(out, "| Metric | Competitors |\n|---|---|");
        for (name, entries) in theirs_only {
            let cells: Vec<String> = entries.iter().map(|e| competitor_cell(e, None)).collect();
            let _ = writeln!(out, "| `{name}` | {} |", cells.join("; "));
        }
    }
    let _ = writeln!(out, "\n### Gate\n");
    if findings.is_empty() {
        let _ = writeln!(
            out,
            "Green: every metric within budget and ahead of every measured competitor."
        );
    }
    for f in findings {
        let _ = writeln!(out, "- RED {f}");
    }
    out
}
