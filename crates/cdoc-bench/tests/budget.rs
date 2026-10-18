//! The gate: pass, over budget, owner ceiling, no budget, stale budget, wrong stat, and slower
//! than a measured competitor (but never than an unmeasured one).

use cdoc_bench::budget::{check, parse, Finding};
use cdoc_bench::schema::{CompetitorResult, CompetitorRun, Measurement, Provenance, Report};
use cdoc_bench::stats::TimeSummary;
use std::collections::BTreeMap;

fn time(median_ns: u64, p95_ns: u64) -> TimeSummary {
    TimeSummary {
        median_ns,
        p95_ns,
        min_ns: median_ns / 2,
        iterations: 10,
    }
}

fn report() -> Report {
    let mut metrics = BTreeMap::new();
    metrics.insert("docx_open/1p".to_owned(), Measurement::Time(time(100, 120)));
    metrics.insert("wasm_size".to_owned(), Measurement::Size { bytes: 1_000 });
    Report {
        machine: "test".to_owned(),
        commit: "0".to_owned(),
        runner_reference_ns: 1_000,
        metrics,
        competitors: BTreeMap::new(),
        browser: None,
    }
}

const BUDGETS: &str = r#"
[runner]
reference_ns = 1000
slow_factor_bp = 12500
floor_ns = 0

[budget."docx_open/1p"]
stat = "p95_ns"
limit = 100
tolerance_bp = 2000

[budget.wasm_size]
stat = "bytes"
limit = 1000
tolerance_bp = 0
"#;

fn competitor(tool: &str, result: CompetitorResult) -> CompetitorRun {
    let mut results = BTreeMap::new();
    results.insert("docx_open/1p".to_owned(), result);
    CompetitorRun {
        tool: tool.to_owned(),
        version: "1".to_owned(),
        results,
        provenance: Provenance::ThisRun,
    }
}

#[test]
fn within_budget_is_green() {
    assert_eq!(check(&report(), &parse(BUDGETS).unwrap()), vec![]);
}

#[test]
fn over_limit_plus_tolerance_is_red() {
    let budgets = parse(&BUDGETS.replace("limit = 100\n", "limit = 99\n")).unwrap();
    let findings = check(&report(), &budgets);
    assert_eq!(
        findings,
        vec![Finding::OverBudget {
            metric: "docx_open/1p".to_owned(),
            value: 120,
            allowed: 118,
        }]
    );
    assert_eq!(
        findings[0].to_string(),
        "docx_open/1p over budget: 120 > 118 allowed"
    );
}

#[test]
fn an_owner_ceiling_is_checked_on_its_own_stat_and_not_stretched() {
    let ceiling = "tolerance_bp = 2000\nceiling = { stat = \"p95_ns\", value = 110 }\n";
    let text = BUDGETS.replacen("stat = \"p95_ns\"", "stat = \"median_ns\"", 1);
    let findings = check(
        &report(),
        &parse(&text.replacen("tolerance_bp = 2000\n", ceiling, 1)).unwrap(),
    );
    assert_eq!(
        findings[0].to_string(),
        "docx_open/1p over the owner's ceiling: p95 120 > 110"
    );
    assert_eq!(findings.len(), 1);
    let roomy = ceiling.replace("110", "120");
    let text = text.replacen("tolerance_bp = 2000\n", &roomy, 1);
    assert_eq!(check(&report(), &parse(&text).unwrap()), vec![]);
}

#[test]
fn a_metric_without_a_budget_is_red() {
    let text = BUDGETS.split("[budget.wasm_size]").next().unwrap();
    let findings = check(&report(), &parse(text).unwrap());
    assert_eq!(findings, vec![Finding::NoBudget("wasm_size".to_owned())]);
    assert_eq!(findings[0].to_string(), "no budget for wasm_size");
}

#[test]
fn a_budget_without_a_metric_is_red() {
    let text = format!("{BUDGETS}\n[budget.gone]\nstat = \"bytes\"\nlimit = 1\ntolerance_bp = 0\n");
    let findings = check(&report(), &parse(&text).unwrap());
    assert_eq!(findings, vec![Finding::UnknownMetric("gone".to_owned())]);
}

#[test]
fn a_bytes_budget_on_a_timing_is_red() {
    let text = BUDGETS.replacen("stat = \"p95_ns\"", "stat = \"bytes\"", 1);
    let findings = check(&report(), &parse(&text).unwrap());
    assert_eq!(
        findings,
        vec![Finding::WrongStat("docx_open/1p".to_owned())]
    );
}

#[test]
fn a_budget_missing_a_number_does_not_parse() {
    assert!(parse("[budget.x]\nstat = \"bytes\"\nlimit = 1\n").is_err());
    assert!(parse("[budget.x]\nstat = \"bytes\"\nlimit = 1.5\ntolerance_bp = 0\n").is_err());
}

#[test]
fn slower_than_a_measured_competitor_is_red_and_says_by_how_much() {
    let mut r = report();
    r.add_competitors(&[competitor("fast", CompetitorResult::Measured(time(80, 90)))]);
    let findings = check(&r, &parse(BUDGETS).unwrap());
    assert_eq!(
        findings[0].to_string(),
        "docx_open/1p slower than fast: ours 100 ns median vs 80 ns (12500 bp)"
    );
    assert_eq!(findings.len(), 1);
}

#[test]
fn faster_than_a_competitor_or_beside_an_unmeasured_one_is_green() {
    let mut r = report();
    r.add_competitors(&[
        competitor("slow", CompetitorResult::Measured(time(200, 220))),
        competitor(
            "browser",
            CompetitorResult::NotMeasured {
                reason: "wave 3".to_owned(),
            },
        ),
        competitor(
            "html",
            CompetitorResult::NotComparable {
                reason: "no PDF".to_owned(),
            },
        ),
    ]);
    assert_eq!(check(&r, &parse(BUDGETS).unwrap()), vec![]);
}
