//! The runner calibration and the noise floor: a slow runner waives budget overruns only (visibly),
//! the floor widens only sub-microsecond budgets, and a real regression on a normal runner fails.

use cdoc_bench::budget::{check, parse, Finding};
use cdoc_bench::runner::{reference_workload, Gate, Verdict};
use cdoc_bench::schema::{CompetitorResult, CompetitorRun, Measurement, Report};
use cdoc_bench::stats::TimeSummary;
use cdoc_bench::table::render;
use std::collections::BTreeMap;

const BUDGETS: &str = r#"
[runner]
reference_ns = 1000
slow_factor_bp = 12500
floor_ns = 100

[budget."tiny/1p"]
stat = "median_ns"
limit = 300
tolerance_bp = 2000

[budget."big/1p"]
stat = "median_ns"
limit = 10000
tolerance_bp = 2000

[budget.size]
stat = "bytes"
limit = 10
tolerance_bp = 0
"#;

fn time(median_ns: u64) -> Measurement {
    Measurement::Time(TimeSummary {
        median_ns,
        p95_ns: median_ns,
        min_ns: median_ns,
        iterations: 9,
    })
}

fn report(reference_ns: u64, tiny: u64, big: u64, size: u64) -> Report {
    let mut metrics = BTreeMap::new();
    metrics.insert("tiny/1p".to_owned(), time(tiny));
    metrics.insert("big/1p".to_owned(), time(big));
    metrics.insert("size".to_owned(), Measurement::Size { bytes: size });
    Report {
        machine: "m".to_owned(),
        commit: "c".to_owned(),
        runner_reference_ns: reference_ns,
        metrics,
        competitors: BTreeMap::new(),
    }
}

fn gate(r: &Report) -> Gate {
    let budgets = parse(BUDGETS).unwrap();
    Gate::new(
        budgets.runner.verdict(r.runner_reference_ns),
        check(r, &budgets),
    )
}

#[test]
fn the_floor_absorbs_tens_of_nanoseconds_below_the_threshold() {
    // 300 ns at 20 % allows 360; the 100 ns floor allows 400.
    assert!(gate(&report(1000, 399, 10_000, 10)).is_green());
    assert!(!gate(&report(1000, 401, 10_000, 10)).is_green());
}

#[test]
fn the_floor_changes_nothing_above_the_threshold() {
    // 10 us at 20 % allows 12 000; the floor would allow only 10 100, so 20 % stands.
    assert!(gate(&report(1000, 300, 12_000, 10)).is_green());
    assert!(!gate(&report(1000, 300, 12_001, 10)).is_green());
}

#[test]
fn sizes_are_never_floored() {
    assert!(!gate(&report(1000, 300, 10_000, 11)).is_green());
}

#[test]
fn a_real_regression_on_a_normal_runner_fails() {
    let g = gate(&report(1000, 600, 20_000, 10));
    assert_eq!(g.verdict, Verdict::Gated { measured_ns: 1000 });
    assert_eq!(g.red.len(), 2);
    assert_eq!(g.waived, vec![]);
}

#[test]
fn a_slow_runner_waives_budget_overruns_visibly() {
    // 1251 > 1000 x 1.25.
    let r = report(1251, 600, 20_000, 10);
    let g = gate(&r);
    assert_eq!(g.verdict, Verdict::SlowRunner { measured_ns: 1251 });
    assert!(g.is_green());
    assert_eq!(g.waived.len(), 2);
    let md = render(&r, &parse(BUDGETS).unwrap(), &g);
    assert!(md.contains("Slow runner — budgets not gated"), "{md}");
    assert!(
        md.contains("slow runner, not gated: tiny/1p over budget"),
        "{md}"
    );
}

#[test]
fn at_the_factor_the_runner_still_gates() {
    assert_eq!(
        gate(&report(1250, 300, 10_000, 10)).verdict,
        Verdict::Gated { measured_ns: 1250 }
    );
}

#[test]
fn a_slow_runner_still_gates_competitors_and_missing_budgets() {
    let mut r = report(5000, 300, 10_000, 10);
    r.metrics.insert("unbudgeted".to_owned(), time(1));
    let mut results = BTreeMap::new();
    results.insert(
        "big/1p".to_owned(),
        CompetitorResult::Measured(TimeSummary {
            median_ns: 9_000,
            p95_ns: 9_000,
            min_ns: 9_000,
            iterations: 1,
        }),
    );
    r.add_competitors(&[CompetitorRun {
        tool: "x".to_owned(),
        version: "1".to_owned(),
        results,
    }]);
    let g = gate(&r);
    assert!(matches!(g.verdict, Verdict::SlowRunner { .. }));
    assert!(g
        .red
        .iter()
        .any(|f| matches!(f, Finding::SlowerThan { .. })));
    assert!(g.red.contains(&Finding::NoBudget("unbudgeted".to_owned())));
}

#[test]
fn the_reference_workload_is_deterministic() {
    assert_eq!(reference_workload(1000), reference_workload(1000));
    assert_ne!(reference_workload(1000), reference_workload(1001));
}

#[test]
fn the_committed_budgets_name_the_runner() {
    let path = format!("{}/../../bench/budgets.toml", env!("CARGO_MANIFEST_DIR"));
    let budgets = parse(&std::fs::read_to_string(path).unwrap()).unwrap();
    assert!(budgets.runner.reference_ns > 0 && budgets.runner.slow_factor_bp > 10_000);
}
