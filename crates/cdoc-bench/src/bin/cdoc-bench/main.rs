//! `cdoc-bench`: the harness's I/O edge. See `docs/benchmarks.md`.
//!
//! ```text
//! cdoc-bench corpus      --out DIR --pages 1,10,100,500
//! cdoc-bench run         --corpus DIR --pages .. --plan bench/plan.toml
//!                        --wasm ARTEFACT=FILE [--wasm ARTEFACT=FILE ..]
//!                        --machine TEXT --commit SHA --out ours.json
//! cdoc-bench competitors --corpus DIR --pages .. --plan bench/plan.toml --out DIR
//! cdoc-bench plan        --plan bench/plan.toml --key tool/metric      (prints "warmup iterations")
//! cdoc-bench summarize   --raw raw.json --cells any|browser --out competitor.json
//!                        (a script's samples; `browser` requires every browser-protocol cell)
//! cdoc-bench browser-plan --plan bench/plan.toml --tool T --corpus DIR --out plan.json
//! cdoc-bench report      --results ours.json --competitors DIR [--competitors DIR ..]
//!                        --budgets bench/budgets.toml --out bench-results.json --markdown FILE
//! ```
//!
//! `report` exits 1 when the gate is red, after writing both files.

mod args;
mod io;

use args::Args;
use cdoc_bench::metrics::{run_ours, Ctx};
use cdoc_bench::plan::Plan;
use cdoc_bench::raw::{self, BrowserPlan, CellSet};
use cdoc_bench::runner::{self, Gate};
use cdoc_bench::schema::{CompetitorRun, RawRun, Report};
use cdoc_bench::{budget, corpus, table};
use std::path::Path;
use std::process::ExitCode;

fn corpus_cmd(a: &Args) -> Result<(), String> {
    let out = Path::new(a.one("out")?);
    for pages in a.pages()? {
        let doc = corpus::generate(pages);
        let json = corpus::to_json(&doc).map_err(|e| e.to_string())?;
        io::write(&io::fixture_path(out, pages, "json"), json.as_bytes())?;
        let media = cdoc_docx::MemoryMedia::with_prefix("img");
        let (docx, _) = cdoc_docx::write_docx(&doc, &media, cdoc_docx::WriteOptions::document())
            .map_err(|e| e.to_string())?;
        io::write(&io::fixture_path(out, pages, "docx"), &docx)?;
        let (md, _) = cdoc_md::document_to_md(&doc);
        io::write(&io::fixture_path(out, pages, "md"), md.as_bytes())?;
        println!(
            "{pages}p: fingerprint {:016x}",
            corpus::fingerprint(&doc).map_err(|e| e.to_string())?
        );
    }
    Ok(())
}

fn plan_of(a: &Args) -> Result<Plan, String> {
    Plan::parse(&io::read_text(Path::new(a.one("plan")?))?).map_err(|e| e.to_string())
}

fn run_cmd(a: &Args) -> Result<(), String> {
    let plan = plan_of(a)?;
    let fixtures = io::load_fixtures(Path::new(a.one("corpus")?), &a.pages()?)?;
    let mut wasm_bytes = std::collections::BTreeMap::new();
    for spec in a.many("wasm")? {
        let (name, file) = spec
            .split_once('=')
            .ok_or_else(|| format!("--wasm {spec:?}: expected ARTEFACT=FILE"))?;
        let bytes = std::fs::metadata(file)
            .map_err(|e| format!("{file}: {e}"))?
            .len();
        if wasm_bytes.insert(name.to_owned(), bytes).is_some() {
            return Err(format!("--wasm {name} given more than once"));
        }
    }
    let clock = io::Monotonic::start();
    let ctx = Ctx {
        clock: &clock,
        plan: &plan,
    };
    let reference = ctx
        .time(runner::REFERENCE, || {
            std::hint::black_box(runner::reference_workload(runner::REFERENCE_ROUNDS));
            Ok(())
        })
        .map_err(|e| e.to_string())?;
    let metrics = run_ours(&ctx, &fixtures, &wasm_bytes).map_err(|e| e.to_string())?;
    let report = Report {
        machine: a.one("machine")?.to_owned(),
        commit: a.one("commit")?.to_owned(),
        runner_reference_ns: reference.median_ns,
        metrics,
        competitors: std::collections::BTreeMap::new(),
    };
    io::write_json(Path::new(a.one("out")?), &report)
}

#[cfg(feature = "competitors")]
fn competitors_cmd(a: &Args) -> Result<(), String> {
    use cdoc_bench::metrics::competitors;
    let plan = plan_of(a)?;
    let fixtures = io::load_fixtures(Path::new(a.one("corpus")?), &a.pages()?)?;
    let clock = io::Monotonic::start();
    let ctx = Ctx {
        clock: &clock,
        plan: &plan,
    };
    let out = Path::new(a.one("out")?);
    let docx_rs = competitors::docx_rs(&ctx, &fixtures).map_err(|e| e.to_string())?;
    io::write_json(&out.join("docx-rs.json"), &docx_rs)?;
    let rustybuzz = competitors::rustybuzz(&ctx).map_err(|e| e.to_string())?;
    io::write_json(&out.join("rustybuzz.json"), &rustybuzz)?;
    let yrs = competitors::yrs(&ctx, &fixtures).map_err(|e| e.to_string())?;
    io::write_json(&out.join("yrs.json"), &yrs)
}

#[cfg(not(feature = "competitors"))]
fn competitors_cmd(_: &Args) -> Result<(), String> {
    Err("built without the `competitors` feature".to_owned())
}

fn summarize_cmd(a: &Args) -> Result<(), String> {
    let raw: RawRun = io::read_json(Path::new(a.one("raw")?))?;
    let cells = CellSet::parse(a.one("cells")?)?;
    let run = raw
        .summarize(cells)
        .map_err(|e| format!("{}: {e}", raw.tool))?;
    io::write_json(Path::new(a.one("out")?), &run)
}

fn browser_plan_cmd(a: &Args) -> Result<(), String> {
    let cells = raw::browser_plan(&plan_of(a)?, a.one("tool")?).map_err(|e| e.to_string())?;
    let dir = Path::new(a.one("corpus")?);
    let mut phrases = std::collections::BTreeMap::new();
    for pages in raw::BROWSER_PAGES {
        let doc = io::read_json(&io::fixture_path(dir, pages, "json"))?;
        let phrase = corpus::keystroke_phrase(&doc)
            .ok_or_else(|| format!("{pages}p: no tail of the keystroke paragraph is unique"))?;
        phrases.insert(format!("{pages}p"), phrase);
    }
    io::write_json(Path::new(a.one("out")?), &BrowserPlan { cells, phrases })
}

fn plan_cmd(a: &Args) -> Result<(), String> {
    let it = plan_of(a)?.get(a.one("key")?).map_err(|e| e.to_string())?;
    println!("{} {}", it.warmup, it.iterations);
    Ok(())
}

/// Writes the merged report and the markdown; `Ok(false)` when the gate is red.
fn report_cmd(a: &Args) -> Result<bool, String> {
    let mut report: Report = io::read_json(Path::new(a.one("results")?))?;
    let mut runs: Vec<CompetitorRun> = Vec::new();
    for dir in a.many("competitors")? {
        for file in io::json_files(Path::new(dir))? {
            runs.push(io::read_json(&file)?);
        }
    }
    report.add_competitors(&runs);
    let budgets = budget::parse(&io::read_text(Path::new(a.one("budgets")?))?)
        .map_err(|e| format!("bench/budgets.toml: {e}"))?;
    let verdict = budgets.runner.verdict(report.runner_reference_ns);
    let gate = Gate::new(verdict, budget::check(&report, &budgets));
    let markdown = table::render(&report, &budgets, &gate);
    io::write_json(Path::new(a.one("out")?), &report)?;
    io::write(Path::new(a.one("markdown")?), markdown.as_bytes())?;
    println!("{markdown}");
    eprintln!("runner: {verdict:?}");
    for f in &gate.red {
        eprintln!("RED {f}");
    }
    for f in &gate.waived {
        eprintln!("slow runner, not gated: {f}");
    }
    Ok(gate.is_green())
}

fn main() -> ExitCode {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let Some((command, rest)) = raw.split_first() else {
        eprintln!("usage: cdoc-bench corpus|run|competitors|plan|summarize|browser-plan|report --flag value ..");
        return ExitCode::from(2);
    };
    let outcome = Args::parse(rest).and_then(|a| match command.as_str() {
        "corpus" => corpus_cmd(&a).map(|()| true),
        "run" => run_cmd(&a).map(|()| true),
        "competitors" => competitors_cmd(&a).map(|()| true),
        "plan" => plan_cmd(&a).map(|()| true),
        "summarize" => summarize_cmd(&a).map(|()| true),
        "browser-plan" => browser_plan_cmd(&a).map(|()| true),
        "report" => report_cmd(&a),
        other => Err(format!("unknown command {other:?}")),
    });
    match outcome {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("cdoc-bench {command}: {e}");
            ExitCode::from(2)
        }
    }
}
