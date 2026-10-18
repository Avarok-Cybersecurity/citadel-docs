//! The report's Docs rows (`crate::docs_rows`) as markdown: ours in the browser beside each
//! browser product, cell by cell, with where and from what each was measured.

use crate::budget::Finding;
use crate::docs_rows::BrowserRows;
use crate::raw::browser_cells;
use crate::runner::Gate;
use crate::schema::{CompetitorResult, CompetitorRun, Provenance};
use crate::stats::basis_points;
use crate::table::human_ns;
use std::fmt::Write as _;

/// A share of frames, in tenths of a percent, integer arithmetic.
fn share(over: u64, of: u64) -> String {
    let permille = u128::from(over) * 1000 / u128::from(of.max(1));
    format!("{}.{} %", permille / 10, permille % 10)
}

fn cell_text(result: Option<&CompetitorResult>, ours_ns: Option<u64>) -> String {
    match result {
        None => "-".to_owned(),
        Some(CompetitorResult::Measured(t)) => {
            let ratio = ours_ns
                .and_then(|o| basis_points(o, t.median_ns))
                .map_or_else(String::new, |bp| format!(", ours {bp} bp"));
            format!("{} / {}{ratio}", human_ns(t.median_ns), human_ns(t.p95_ns))
        }
        Some(CompetitorResult::MeasuredBytes { median_bytes, .. }) => format!("{median_bytes} B"),
        Some(CompetitorResult::MeasuredFrames(f)) => {
            format!("{} of {} ({})", f.over, f.of, share(f.over, f.of))
        }
        Some(CompetitorResult::NotMeasured { .. }) => "not measured".to_owned(),
        Some(CompetitorResult::NotComparable { .. }) => "not comparable".to_owned(),
    }
}

fn ours_text(result: Option<&CompetitorResult>) -> (String, String) {
    match result {
        Some(CompetitorResult::Measured(t)) => (human_ns(t.median_ns), human_ns(t.p95_ns)),
        other => (cell_text(other, None), "-".to_owned()),
    }
}

fn origin(run: &CompetitorRun) -> String {
    match &run.provenance {
        Provenance::RecordedOurs {
            runner,
            browser,
            date,
            run_url,
            ui_commit,
            engine_commit,
            ..
        } => format!(
            "{} {}: UI `{ui_commit}`, engine `{engine_commit}`; {date}, {browser}, {runner}, \
             {run_url}",
            run.tool, run.version
        ),
        Provenance::Recorded {
            runner,
            browser,
            date,
            run_url,
            ..
        } => format!(
            "{} {}: {date}, {browser}, {runner}, {run_url}",
            run.tool, run.version
        ),
        Provenance::ThisRun => format!("{} {}: this run", run.tool, run.version),
    }
}

/// Appends the Docs rows: a table per cell of the browser protocol, then where each came from.
pub fn render(out: &mut String, rows: &BrowserRows, gate: &Gate) {
    let _ = writeln!(
        out,
        "\n### Docs rows (ours in the browser, the protocol of bench/competitors/README.md)\n\n\
         Times are median / p95; frames are those over 16.7 ms. Each row is red when a product \
         measured on the same run is ahead of ours on median, p95, bytes or the share of long \
         frames, or measured a cell ours did not.\n"
    );
    let header: Vec<String> = rows.products.iter().map(|p| p.tool.clone()).collect();
    let _ = writeln!(
        out,
        "| Cell | Ours median | Ours p95 | {} | Gate |\n|---|---|---|{}---|",
        header.join(" | "),
        "---|".repeat(header.len())
    );
    for cell in browser_cells() {
        let ours = rows.ours.results.get(&cell);
        let (median, p95) = ours_text(ours);
        let ours_ns = match ours {
            Some(CompetitorResult::Measured(t)) => Some(t.median_ns),
            _ => None,
        };
        let theirs: Vec<String> = rows
            .products
            .iter()
            .map(|p| cell_text(p.results.get(&cell), ours_ns))
            .collect();
        let red = gate
            .red
            .iter()
            .any(|f| matches!(f, Finding::Behind(b) if b.cell == cell));
        let verdict = if red { "**RED**" } else { "ok" };
        let _ = writeln!(
            out,
            "| `{cell}` | {median} | {p95} | {} | {verdict} |",
            theirs.join(" | ")
        );
    }
    let _ = writeln!(out);
    for run in std::iter::once(&rows.ours).chain(&rows.products) {
        let _ = writeln!(out, "- {}", origin(run));
    }
}
