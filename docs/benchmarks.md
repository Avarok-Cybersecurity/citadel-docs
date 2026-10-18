# Benchmarks

**The owner's rule: we will be the fastest on the market.** Performance is a release gate: a
release ships only when the published benchmarks beat every named competitor on the same machine
and corpus. The `bench` workflow enforces the part of that rule that can run on a CI runner, on
every pull request and every push to `main`.

## What is measured

`crates/cdoc-bench` defines every metric once (`metrics::Family`) and the corpus they run on.
Times are integer nanoseconds, sizes integer bytes; ratios are integer basis points. Each timed
metric runs `warmup` untimed iterations and then `iterations` timed ones (`bench/plan.toml`), and
reports min, median and p95 (nearest rank).

| Metric | What one iteration does |
|---|---|
| `docx_open/Np` | `.docx` bytes to model (`cdoc_docx::read_docx`) |
| `docx_save/Np` | model to `.docx` bytes |
| `md_open/Np` | Markdown text to model |
| `layout_full/Np` | a fresh layout of the whole document |
| `layout_keystroke/Np` | incremental relayout after a one-character edit in the middle body paragraph, warm session (owner budget: p95 < 8 ms on 100 pages, the `ceiling` of its budget) |
| `keystroke_select_all/500p` | the editor's result record (formatting mixed over the selection, its capped text, its rectangles on a three-page viewport) and its encoding, the whole 500-page body selected (ceiling: p95 < 8 ms) |
| `pdf_export/Np` | laid-out pages to PDF bytes (layout excluded) |
| `docx_to_pdf/Np` | `.docx` bytes to PDF bytes: open, lay out, export |
| `paint_encode/Np`, `paint_decode/Np` | layout to the binary display list; the list decoded with every op and glyph visited |
| `crdt_from_model/Np`, `crdt_snapshot_encode/Np`, `crdt_snapshot_decode/Np`, `crdt_keystroke/Np` | the model into the CRDT; its snapshot out and in; one character typed mid-document |
| `crdt_type/10k_chars` | 10 000 characters typed into a fresh CRDT document |
| `crdt_diff_bytes/after_1_keystroke_in_1MB_doc`, `crdt_diff/after_1_keystroke_in_1MB_doc` | the update a peer in sync needs after one keystroke in a 1 MB document built by random edits (many tombstones): its bytes, and the time to type the key and encode it |
| `crdt_snapshot_bytes/100k_churn`, `crdt_snapshot_bytes/100k_churn_after_gc` | snapshot bytes after 100 000 short inserts and deletes, before and after `Doc::gc` with every peer caught up |
| `crdt_diff_bytes/after_1_block_field_change` | the update a peer in sync needs after one property (alignment) of a paragraph with four set is changed |
| `shape_latin/10k_chars` | 10 000 Latin characters shaped on Carlito with Word's default features |
| `para_break/2000_words` | Knuth-Plass breaking of a 2 000-word hyphenated paragraph |
| `wasm_size` | bytes of `cdoc_wasm.wasm`, `--release`, `wasm32-unknown-unknown` |

`N` is 1, 10, 100 and 500 (`layout_keystroke` from 10). The corpus (`cdoc_bench::corpus`) is
procedural from a constant seed through `SplitMix64`: headings, prose, bulleted and numbered
lists and tables, calibrated so `generate(n)` lays out to about `n` pages. The 1- and 10-page
fixtures (`.json` model, `.docx`, `.md`) are committed under `crates/cdoc-bench/corpus/`; the job
checks they are byte-for-byte what the generator makes, and generates 100 and 500 pages at bench
time. A test pins the 10-page fingerprint; changing the corpus is deliberate or it is a failure.

Competitors and how each is measured: [`bench/competitors/README.md`](../bench/competitors/README.md).

## How to read the table

The job posts one comment on the PR (updated in place) and uploads `bench-results.json` and
`bench-results.md`. One row per metric of ours:

- **Ours median / p95**: rounded for reading; the JSON holds the integers.
- **Budget**: the stat limited and the most it may be (`limit` plus `tolerance_bp`), and the
  owner's ceiling where there is one (checked on its own stat, never stretched).
- **Competitors**: each competitor's median and ours over theirs in basis points (10000 = equal,
  below 10000 = we are faster), or `not comparable` / `not measured`. A blank is never a win.
- **Gate**: `ok`, or **RED** with the reasons listed under the table.

The gate is red when a metric of ours has no budget, a budget names a metric that does not exist,
a metric exceeds its budget, or our median is above a **measured** competitor's on the same
metric. Word, Google Docs and ONLYOFFICE are `not measured` until the wave 3 browser protocol
runs; until then the release gate cannot be declared met for them.

## Budgets

`bench/budgets.toml` holds one entry per metric: `stat`, `limit`, `tolerance_bp` and optionally
`ceiling = { stat, value }`. Limits come from the runner's artefacts with 20 % tolerance
(`tolerance_bp = 2000`), taking the largest value seen across the CPU models the GitHub pool
assigns (they differ by up to 2x; the slowest sets the limit). Timings are budgeted on the median,
which is stable on a shared runner; the p95 tail is not, so a p95 is only ever an owner's ceiling. A metric added to the harness fails the gate until it has a budget.
Raise a budget only with the reason in the commit message; lower it when a speed-up lands.

### Slow runners and the noise floor

`[runner]` in `bench/budgets.toml` keeps an unrelated PR from going red on a slow or noisy runner
while the budgets stay regression guards (`crates/cdoc-bench/src/runner.rs`):

- **Calibration.** Before ours, `cdoc-bench run` times a fixed integer workload that touches no
  code of ours (`runner_reference` in the plan) and records its median in the report. When it is
  slower than `reference_ns` by more than `slow_factor_bp` (12500 = 1.25 x), the table's header
  says **Slow runner — budgets not gated**, each budget overrun is listed as "slow runner, not
  gated" (in the table, under Gate, and in the log), and the run is green on them. Competitor
  comparisons (the release gate), owner ceilings, and missing or unknown budgets still gate: they
  are judged on this machine against itself. `reference_ns` is the workload's median on a normal
  runner; update it, with the reason, if the runner pool changes.
- **Noise floor.** A timed budget allows at least `floor_ns` of slack above its limit, whatever
  its tolerance, so a 300 ns median does not fail on 20 ns. With 20 % tolerance this changes only
  limits below `floor_ns / 0.2`; a real regression (2 x) still fails, and sizes are exact and
  never floored.

## Running locally (smoke only)

Local numbers are for checking that the harness runs; they are not comparable with the runner's
and never set a budget.

```sh
export CARGO_TARGET_DIR=...   # your own target dir
cargo build --release -p cdoc-bench --features competitors
cargo build --release -p cdoc-wasm --target wasm32-unknown-unknown
bin=$CARGO_TARGET_DIR/release/cdoc-bench
$bin corpus --out /tmp/corpus --pages 1,10,100,500
$bin run --corpus /tmp/corpus --pages 1,10,100,500 --plan bench/plan.toml \
  --wasm $CARGO_TARGET_DIR/wasm32-unknown-unknown/release/cdoc_wasm.wasm \
  --machine local --commit local --out /tmp/ours.json
$bin competitors --corpus /tmp/corpus --pages 1,10,100,500 --plan bench/plan.toml --out /tmp/comp
$bin report --results /tmp/ours.json --competitors /tmp/comp \
  --competitors bench/competitors/placeholders --budgets bench/budgets.toml \
  --out /tmp/bench-results.json --markdown /tmp/bench-results.md
```

mammoth needs Node (`bench/competitors/mammoth/run.sh`); LibreOffice needs `soffice` and
`python3-uno` (`bench/competitors/libreoffice/run.sh`). The per-crate criterion benches
(`cargo bench -p cdoc-shape`, `-p cdoc-layout`, `-p cdoc-para`, `-p cdoc-paint`, `-p cdoc-crdt`)
remain for profiling one crate; the harness is the one that gates.
