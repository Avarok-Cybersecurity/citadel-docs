//! The JSON shapes round-trip, carry integers only, spell the statuses the README documents, and
//! the committed placeholder and plan files parse.

use cdoc_bench::metrics::Family;
use cdoc_bench::plan::{Plan, PlanError};
use cdoc_bench::raw::CellSet;
use cdoc_bench::schema::{
    CompetitorResult, CompetitorRun, Measurement, Provenance, RawRun, Report,
};
use cdoc_bench::stats::TimeSummary;
use std::collections::BTreeMap;

fn sample_report() -> Report {
    let t = TimeSummary {
        median_ns: 5,
        p95_ns: 9,
        min_ns: 1,
        iterations: 3,
    };
    let mut metrics = BTreeMap::new();
    metrics.insert("layout_full/1p".to_owned(), Measurement::Time(t));
    metrics.insert("wasm_size".to_owned(), Measurement::Size { bytes: 7 });
    let mut report = Report {
        machine: "m".to_owned(),
        commit: "c".to_owned(),
        runner_reference_ns: 1,
        metrics,
        competitors: BTreeMap::new(),
        browser: None,
    };
    let mut results = BTreeMap::new();
    results.insert("layout_full/1p".to_owned(), CompetitorResult::Measured(t));
    results.insert(
        "pdf_export/1p".to_owned(),
        CompetitorResult::NotMeasured {
            reason: "browser".to_owned(),
        },
    );
    report.add_competitors(&[CompetitorRun {
        tool: "x".to_owned(),
        version: "1".to_owned(),
        results,
        provenance: Provenance::ThisRun,
    }]);
    report
}

#[test]
fn the_report_round_trips_through_json() {
    let report = sample_report();
    let text = serde_json::to_string(&report).unwrap();
    assert_eq!(serde_json::from_str::<Report>(&text).unwrap(), report);
}

#[test]
fn the_report_holds_no_floats_and_spells_statuses_out() {
    let text = serde_json::to_string(&sample_report()).unwrap();
    assert!(!text.contains('.'), "{text}");
    assert!(text.contains(r#""status":"not measured""#));
    assert!(text.contains(r#""status":"measured","median_ns":5"#));
    assert!(text.contains(r#""wasm_size":{"bytes":7}"#));
}

#[test]
fn a_float_nanosecond_does_not_parse() {
    let text = r#"{"median_ns":1.5,"p95_ns":2,"min_ns":1,"iterations":1}"#;
    assert!(serde_json::from_str::<TimeSummary>(text).is_err());
}

#[test]
fn raw_samples_are_summarised_like_ours() {
    let raw: RawRun = serde_json::from_str(
        r#"{"tool":"t","version":"1","samples":{"docx_open/1p":[3,1,2]},"bytes":{},"frames":{},
            "unmeasured":{"pdf_export/1p":{"status":"not comparable","reason":"html"}},
            "provenance":{"kind":"this run"}}"#,
    )
    .unwrap();
    let run = raw.summarize(CellSet::Any).unwrap();
    assert!(matches!(
        run.results["docx_open/1p"],
        CompetitorResult::Measured(TimeSummary {
            median_ns: 2,
            min_ns: 1,
            p95_ns: 3,
            iterations: 3
        })
    ));
    assert!(matches!(
        run.results["pdf_export/1p"],
        CompetitorResult::NotComparable { .. }
    ));
}

#[test]
fn the_committed_browser_results_fill_every_protocol_cell_with_provenance() {
    let cells: std::collections::BTreeSet<String> =
        cdoc_bench::raw::browser_cells().into_iter().collect();
    for tool in ["onlyoffice"] {
        let path = format!(
            "{}/../../bench/competitors/results/{tool}.json",
            env!("CARGO_MANIFEST_DIR")
        );
        let run: CompetitorRun =
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(
            run.results
                .keys()
                .cloned()
                .collect::<std::collections::BTreeSet<_>>(),
            cells
        );
        assert!(
            matches!(run.provenance, Provenance::Recorded { .. }),
            "{tool}"
        );
        for cell in run.results.values() {
            if let CompetitorResult::NotMeasured { reason } = cell {
                assert!(!reason.trim().is_empty(), "{tool}");
            }
        }
    }
}

#[test]
fn the_committed_plan_parses_and_names_what_it_lacks() {
    let path = format!("{}/../../bench/plan.toml", env!("CARGO_MANIFEST_DIR"));
    let plan = Plan::parse(&std::fs::read_to_string(path).unwrap()).unwrap();
    assert!(plan
        .get(&cdoc_bench::metrics::paged(Family::LayoutKeystroke, 100))
        .is_ok());
    assert_eq!(
        plan.get("nothing"),
        Err(PlanError::Missing("nothing".to_owned()))
    );
    let zero = Plan::parse("[plan.x]\nwarmup = 0\niterations = 0\n");
    assert_eq!(zero, Err(PlanError::Zero("x".to_owned())));
}
