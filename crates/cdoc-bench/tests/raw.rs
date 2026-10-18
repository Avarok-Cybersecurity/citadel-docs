//! A script's raw file is summarised only when every cell sits under its own unit, once, with
//! samples or a reason; a browser run must hold exactly the protocol's cells. Provenance is
//! required, and the committed plan covers every browser runner.

use cdoc_bench::plan::Plan;
use cdoc_bench::raw::{browser_cells, browser_plan, unit_of, CellSet, RawError, Unit};
use cdoc_bench::schema::{CompetitorResult, FrameCount, RawRun};
use serde_json::{json, Value};

fn raw(value: &Value) -> RawRun {
    serde_json::from_value(value.clone()).unwrap()
}

/// A browser run with every protocol cell filled once, under its own unit.
fn browser_run() -> Value {
    let mut samples = serde_json::Map::new();
    let mut unmeasured = serde_json::Map::new();
    for cell in browser_cells() {
        match unit_of(&cell) {
            Unit::Nanoseconds => {
                samples.insert(cell, json!([3, 1, 2]));
            }
            Unit::Bytes => {
                unmeasured.insert(
                    cell,
                    json!({"status": "not measured", "reason": "not cross-origin isolated"}),
                );
            }
            Unit::Frames => {}
        }
    }
    json!({
        "tool": "ONLYOFFICE Docs", "version": "1", "samples": samples, "bytes": {},
        "frames": {"scroll_long_frames/500p": {"over": 2, "of": 9}},
        "unmeasured": unmeasured,
        "provenance": {"kind": "recorded", "runner": "r", "browser": "b", "commit": "c",
                       "date": "2026-10-17", "run_url": "u"},
    })
}

fn refused(value: &Value, cells: CellSet) -> RawError {
    raw(value).summarize(cells).unwrap_err()
}

#[test]
fn a_full_browser_run_is_summarised() {
    let run = raw(&browser_run()).summarize(CellSet::Browser).unwrap();
    assert_eq!(run.results.len(), browser_cells().len());
    assert_eq!(
        run.results["scroll_long_frames/500p"],
        CompetitorResult::MeasuredFrames(FrameCount { over: 2, of: 9 })
    );
}

#[test]
fn memory_filed_as_nanoseconds_is_refused() {
    let mut v = browser_run();
    v["unmeasured"]
        .as_object_mut()
        .unwrap()
        .remove("memory/500p");
    v["samples"]["memory/500p"] = json!([1]);
    assert!(matches!(
        refused(&v, CellSet::Browser),
        RawError::WrongUnit {
            found: Unit::Nanoseconds,
            expected: Unit::Bytes,
            ..
        }
    ));
}

#[test]
fn a_timing_filed_as_bytes_is_refused() {
    let mut v = browser_run();
    v["samples"]
        .as_object_mut()
        .unwrap()
        .remove("docx_open/100p");
    v["bytes"]["docx_open/100p"] = json!([1]);
    assert!(matches!(
        refused(&v, CellSet::Any),
        RawError::WrongUnit {
            found: Unit::Bytes,
            expected: Unit::Nanoseconds,
            ..
        }
    ));
}

#[test]
fn an_empty_sample_set_is_refused() {
    let mut v = browser_run();
    v["samples"]["docx_open/100p"] = json!([]);
    assert_eq!(
        refused(&v, CellSet::Any),
        RawError::Empty("docx_open/100p".to_owned())
    );
}

#[test]
fn an_unmeasured_cell_without_a_reason_is_a_blank_and_refused() {
    let mut v = browser_run();
    v["unmeasured"]["memory/500p"]["reason"] = json!("  ");
    assert_eq!(
        refused(&v, CellSet::Any),
        RawError::Blank("memory/500p".to_owned())
    );
}

#[test]
fn a_measurement_filed_as_unmeasured_is_refused() {
    let mut v = browser_run();
    v["unmeasured"]["memory/500p"] = json!({"status": "measured bytes", "median_bytes": 1,
                                            "iterations": 1});
    assert_eq!(
        refused(&v, CellSet::Any),
        RawError::Blank("memory/500p".to_owned())
    );
}

#[test]
fn a_cell_filed_twice_is_refused() {
    let mut v = browser_run();
    v["unmeasured"]["docx_open/100p"] = json!({"status": "not measured", "reason": "x"});
    assert_eq!(
        refused(&v, CellSet::Any),
        RawError::Twice("docx_open/100p".to_owned())
    );
}

#[test]
fn a_browser_run_missing_a_cell_is_refused_but_a_native_one_is_not() {
    let mut v = browser_run();
    v["samples"]
        .as_object_mut()
        .unwrap()
        .remove("docx_to_pdf/500p");
    assert_eq!(
        refused(&v, CellSet::Browser),
        RawError::Missing("docx_to_pdf/500p".to_owned())
    );
    assert!(raw(&v).summarize(CellSet::Any).is_ok());
}

#[test]
fn a_browser_run_with_a_cell_the_protocol_lacks_is_refused() {
    let mut v = browser_run();
    v["samples"]["layout_full/100p"] = json!([1]);
    assert_eq!(
        refused(&v, CellSet::Browser),
        RawError::Unknown("layout_full/100p".to_owned())
    );
}

#[test]
fn more_long_frames_than_frames_is_refused() {
    let mut v = browser_run();
    v["frames"]["scroll_long_frames/500p"] = json!({"over": 10, "of": 9});
    assert!(matches!(refused(&v, CellSet::Browser), RawError::Frames(_)));
}

#[test]
fn a_raw_file_without_provenance_does_not_parse() {
    let mut v = browser_run();
    v.as_object_mut().unwrap().remove("provenance");
    assert!(serde_json::from_value::<RawRun>(v).is_err());
}

#[test]
fn an_unknown_cell_set_is_refused() {
    assert!(CellSet::parse("browser").is_ok());
    assert!(CellSet::parse("").is_err());
}

#[test]
fn the_committed_plan_covers_every_browser_runner() {
    let path = format!("{}/../../bench/plan.toml", env!("CARGO_MANIFEST_DIR"));
    let plan = Plan::parse(&std::fs::read_to_string(path).unwrap()).unwrap();
    for tool in ["ours", "onlyoffice"] {
        let cells = browser_plan(&plan, tool).unwrap();
        assert_eq!(cells.len(), browser_cells().len() - 1, "{tool}");
    }
    assert!(browser_plan(&plan, "nobody").is_err());
}

#[test]
fn a_frame_count_of_no_frames_is_refused() {
    let mut v = browser_run();
    v["frames"]["scroll_long_frames/500p"] = json!({"over": 0, "of": 0});
    assert_eq!(
        refused(&v, CellSet::Browser),
        RawError::Empty("scroll_long_frames/500p".to_owned())
    );
}

#[test]
fn a_browser_product_the_benchmark_does_not_compare_is_refused() {
    // Owner decision 2026-10-18: Word (web) and Google Docs are not compared.
    for tool in ["Microsoft Word (web)", "Google Docs"] {
        let mut v = browser_run();
        v["tool"] = json!(tool);
        assert_eq!(
            refused(&v, CellSet::Browser),
            RawError::UnknownProduct(tool.to_owned())
        );
    }
}
