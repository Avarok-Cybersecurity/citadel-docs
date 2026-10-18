//! The Docs rows: ours in the browser against the browser products. Summarize takes ours only
//! with its commits and a product only as a product; the gate is red when a product measured on
//! the same run is ahead on median, p95, bytes or the share of long frames, or measured a cell
//! ours did not, and a pair from two runs is never compared.

use cdoc_bench::docs_rows::{check, split, BrowserRows, Lag};
use cdoc_bench::raw::{browser_cells, unit_of, CellSet, RawError, Unit};
use cdoc_bench::schema::{CompetitorRun, FrameCount, RawRun};
use serde_json::{json, Value};

const UI: &str = "b6677f90397f4b06c44c4acc0818950ab1b5f06c";
const ENGINE: &str = "f8656c69029bffd071d679f496b9671c573df18d";

fn provenance(ours: bool, run_url: &str) -> Value {
    let mut p = json!({"kind": "recorded", "runner": "r", "browser": "b", "commit": "c",
                       "date": "2026-10-18", "run_url": run_url});
    if ours {
        p["kind"] = json!("recorded ours");
        p["ui_commit"] = json!(UI);
        p["engine_commit"] = json!(ENGINE);
    }
    p
}

/// Every protocol cell measured: times `ns` each, memory `bytes`, `over` of 100 frames long.
fn run(tool: &str, ours: bool, ns: u64, bytes: u64, over: u64) -> Value {
    let mut samples = serde_json::Map::new();
    for cell in browser_cells() {
        if unit_of(&cell) == Unit::Nanoseconds {
            samples.insert(cell, json!([ns, ns, ns]));
        }
    }
    json!({
        "tool": tool, "version": "1", "samples": samples,
        "bytes": {"memory/500p": [bytes]},
        "frames": {"scroll_long_frames/500p": {"over": over, "of": 100}},
        "unmeasured": {}, "provenance": provenance(ours, "u"),
    })
}

fn summarised(value: &Value, cells: CellSet) -> Result<CompetitorRun, RawError> {
    serde_json::from_value::<RawRun>(value.clone())
        .unwrap()
        .summarize(cells)
}

fn rows(ours: &Value, theirs: &Value) -> BrowserRows {
    BrowserRows {
        ours: summarised(ours, CellSet::BrowserOurs).unwrap(),
        products: vec![summarised(theirs, CellSet::Browser).unwrap()],
    }
}

#[test]
fn ours_is_summarised_only_with_both_commits_and_never_as_a_product() {
    let ours = run("Citadel Docs", true, 1, 1, 0);
    assert!(summarised(&ours, CellSet::BrowserOurs).is_ok());
    assert!(matches!(
        summarised(&ours, CellSet::Browser),
        Err(RawError::Provenance(_))
    ));
    for field in ["ui_commit", "engine_commit"] {
        let mut v = ours.clone();
        v["provenance"][field] = json!("main");
        assert_eq!(
            summarised(&v, CellSet::BrowserOurs),
            Err(RawError::Provenance("Citadel Docs".to_owned())),
            "{field}"
        );
    }
    let product = run("ONLYOFFICE Docs", false, 1, 1, 0);
    assert!(matches!(
        summarised(&product, CellSet::BrowserOurs),
        Err(RawError::Provenance(_))
    ));
    assert_eq!(CellSet::parse("browser-ours"), Ok(CellSet::BrowserOurs));
}

#[test]
fn ours_ahead_on_every_cell_is_green() {
    assert_eq!(
        check(&rows(
            &run("ours", true, 1, 10, 1),
            &run("ONLYOFFICE Docs", false, 2, 20, 2)
        )),
        vec![]
    );
}

#[test]
fn a_median_above_theirs_is_red_on_every_timed_cell() {
    let found = check(&rows(
        &run("ours", true, 3, 10, 1),
        &run("ONLYOFFICE Docs", false, 2, 20, 2),
    ));
    let timed = browser_cells()
        .into_iter()
        .filter(|c| unit_of(c) == Unit::Nanoseconds)
        .count();
    assert_eq!(
        found
            .iter()
            .filter(|b| matches!(b.lag, Lag::Median { ours: 3, theirs: 2 }))
            .count(),
        timed
    );
    assert_eq!(
        found
            .iter()
            .filter(|b| matches!(b.lag, Lag::P95 { .. }))
            .count(),
        timed
    );
}

#[test]
fn a_p95_above_theirs_alone_is_red() {
    let mut ours = run("ours", true, 1, 10, 1);
    ours["samples"]["docx_open/100p"] =
        json!(vec![1; 9].into_iter().chain([9]).collect::<Vec<u64>>());
    let found = check(&rows(&ours, &run("ONLYOFFICE Docs", false, 2, 20, 2)));
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(
        (found[0].cell.as_str(), &found[0].lag),
        ("docx_open/100p", &Lag::P95 { ours: 9, theirs: 2 })
    );
}

#[test]
fn more_memory_or_a_larger_share_of_long_frames_is_red() {
    let found = check(&rows(
        &run("ours", true, 1, 30, 1),
        &run("ONLYOFFICE Docs", false, 2, 20, 2),
    ));
    assert_eq!(
        found.iter().map(|b| &b.lag).collect::<Vec<_>>(),
        vec![&Lag::Bytes {
            ours: 30,
            theirs: 20
        }]
    );
    let mut ours = run("ours", true, 1, 10, 3);
    ours["frames"]["scroll_long_frames/500p"] = json!({"over": 3, "of": 100});
    let mut theirs = run("ONLYOFFICE Docs", false, 2, 20, 0);
    // 5 of 200 is 2.5 %, under ours' 3 %, though more frames.
    theirs["frames"]["scroll_long_frames/500p"] = json!({"over": 5, "of": 200});
    let found = check(&rows(&ours, &theirs));
    let lag = Lag::LongFrames {
        ours: FrameCount { over: 3, of: 100 },
        theirs: FrameCount { over: 5, of: 200 },
    };
    assert_eq!(found.iter().map(|b| &b.lag).collect::<Vec<_>>(), vec![&lag]);
}

#[test]
fn a_cell_they_measured_and_ours_did_not_is_red_and_theirs_unmeasured_is_skipped() {
    let mut ours = run("ours", true, 1, 10, 1);
    ours["bytes"] = json!({});
    ours["unmeasured"] = json!({"memory/500p": {"status": "not measured", "reason": "x"}});
    let found = check(&rows(&ours, &run("ONLYOFFICE Docs", false, 2, 20, 2)));
    assert_eq!(
        found
            .iter()
            .map(|b| (b.cell.as_str(), &b.lag))
            .collect::<Vec<_>>(),
        vec![("memory/500p", &Lag::Unmeasured)]
    );
    let mut theirs = run("ONLYOFFICE Docs", false, 2, 20, 2);
    theirs["bytes"] = json!({});
    theirs["unmeasured"] = json!({"memory/500p": {"status": "not measured", "reason": "x"}});
    assert_eq!(check(&rows(&run("ours", true, 1, 99, 1), &theirs)), vec![]);
}

#[test]
fn numbers_from_two_runs_are_not_compared_and_turn_red() {
    let mut theirs = run("ONLYOFFICE Docs", false, 2, 20, 2);
    theirs["provenance"]["run_url"] = json!("another run");
    let found = check(&rows(&run("ours", true, 1, 10, 1), &theirs));
    assert_eq!(found.len(), browser_cells().len());
    assert!(found
        .iter()
        .all(|b| matches!(&b.lag, Lag::OtherMachine { theirs, .. } if theirs == "another run")));
}

#[test]
fn products_need_ours_beside_them_and_ours_comes_once() {
    let product = summarised(&run("ONLYOFFICE Docs", false, 2, 20, 2), CellSet::Browser).unwrap();
    let ours = summarised(&run("ours", true, 1, 10, 1), CellSet::BrowserOurs).unwrap();
    assert!(split(vec![product.clone()]).is_err());
    assert!(split(vec![ours.clone(), product.clone(), ours.clone()]).is_err());
    let (native, browser) = split(vec![product, ours]).unwrap();
    assert_eq!(native, Vec::<CompetitorRun>::new());
    assert_eq!(browser.unwrap().products.len(), 1);
    assert!(matches!(split(vec![]), Ok((_, None))));
}

#[test]
fn the_docs_table_names_each_cell_and_marks_the_red_ones() {
    use cdoc_bench::budget::Finding;
    use cdoc_bench::runner::{Gate, Verdict};
    let mut ours = run("Citadel Docs", true, 1, 10, 1);
    ours["samples"]["docx_open/500p"] = json!([5, 5, 5]);
    let rows = rows(&ours, &run("ONLYOFFICE Docs", false, 2, 20, 2));
    let findings = check(&rows).into_iter().map(Finding::Behind).collect();
    let gate = Gate::new(Verdict::Gated { measured_ns: 1 }, findings);
    let mut out = String::new();
    cdoc_bench::docs_table::render(&mut out, &rows, &gate);
    assert!(
        out.contains("| Cell | Ours median | Ours p95 | ONLYOFFICE Docs | Gate |"),
        "{out}"
    );
    let line = |cell: &str| {
        out.lines()
            .find(|l| l.starts_with(&format!("| `{cell}`")))
            .unwrap()
            .to_owned()
    };
    assert!(line("docx_open/500p").ends_with("**RED** |"), "{out}");
    assert!(line("docx_open/100p").ends_with("| ok |"), "{out}");
    assert!(
        line("scroll_long_frames/500p").contains("2 of 100 (2.0 %)"),
        "{out}"
    );
    assert!(
        out.contains(&format!("UI `{UI}`, engine `{ENGINE}`")),
        "{out}"
    );
}
