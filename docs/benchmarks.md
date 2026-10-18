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
| `wasm_size/<artefact>` | bytes of one shipped `cdoc-wasm` artefact (`full`, `editor`, `convert`): release, `wasm32-unknown-unknown`, wasm-bindgen, `wasm-opt -Oz` (`scripts/build-wasm.sh`) |

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
  A cell measured by `competitors.yml` rather than this job says `(recorded DATE, BROWSER)`.
- **Gate**: `ok`, or **RED** with the reasons listed under the table.

The gate is red when a metric of ours has no budget, a budget names a metric that does not exist,
a metric exceeds its budget, or our median is above a **measured** competitor's on the same
metric. Word, Google Docs and ONLYOFFICE are measured by the wave 3 browser protocol in
`competitors.yml` and committed under `bench/competitors/results/`; Word and Google Docs stay
`not measured` until their test accounts exist, and until then the release gate cannot be
declared met for them. Ours does not run the browser protocol yet, so a browser competitor's
cell is compared with ours measured natively (the engine alone): red there means slower even
so, green proves nothing like for like.

## Budgets

`bench/budgets.toml` holds one entry per metric: `stat`, `limit`, `tolerance_bp` and optionally
`ceiling = { stat, value }`. Limits come from the runner's artefacts with 20 % tolerance
(`tolerance_bp = 2000`), taking the largest value seen across the CPU models the GitHub pool
assigns (they differ by up to 2x; the slowest sets the limit). Timings are budgeted on the median,
which is stable on a shared runner; the p95 tail is not, so a p95 is only ever an owner's ceiling. A metric added to the harness fails the gate until it has a budget.
Raise a budget only with the reason in the commit message; lower it when a speed-up lands.

Since #20 every run is a `GLYPH_RUN` carrying per-glyph id, cluster, advance and offsets (five
varints a glyph, where `TEXT_RUN` had one table index), so the display-list metrics roughly
doubled and their budgets were raised. A lazy path that skips clusters while painting would save
at most one varint of five in one of the two passes: opening a page must still read every
cluster to check it is ordered and on a character boundary. The follow-up worth measuring is a
column layout per run (ids, advances, offsets, clusters as separate arrays, offsets usually all
zero and run-length coded) so that both the check and the paint pass touch fewer bytes.

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

## WASM sizes

What each part of `cdoc-wasm` costs, built with only that feature (`--no-default-features
--features <f>`), shipped as the UI ships it (wasm-bindgen 0.2.129 `--target web`, binaryen 123
`wasm-opt -Oz`), `opt-level = "s"`, bytes. Sizes are deterministic, so local numbers are the
runner's. The per-feature rows were measured on main 733cb62; the artefact rows on main f8656c6, after
#20 linked the shaper, Knuth-Plass and hyphenation into the core (the budgets'; macOS, the CI
runner differs by under 0.05 %). #20 added 650 KB to every artefact, 440 KB (brotli 160 KB) of it the five
hyphenation pattern files compiled in; they are now fetched assets
(`crates/cdoc-para/patterns/manifest.json`, ARCHITECTURE.md), which took the editor from 4 160 265
to 3 727 270 bytes. The core (model, ProseMirror mapping, schema, list labels, serde JSON at the boundary)
is in every build. The per-feature rows predate #20, so their core lacks `cdoc-shape` and
`cdoc-para`, which cdoc-layout (and so every artefact) now links.

| build | raw cargo | `wasm-opt -Oz` | gzip -9 | brotli -q 11 | over core (opt) |
|---|---:|---:|---:|---:|---:|
| core alone | 1 802 276 | 815 936 | 262 704 | 191 779 | |
| + `paint` (display list, glyphs) | 2 312 122 | 1 112 799 | 396 848 | 296 150 | +296 863 |
| + `formats` (docx/dotx, odt/ott, md, txt) | 4 140 675 | 2 010 152 | 717 841 | 521 513 | +1 194 216 |
| + `edit` (editor, CRDT, the paint it links, and the formats its file I/O needs) | 6 715 437 | 3 384 690 | 1 192 299 | 844 624 | +2 568 754 |
| + `pdf` (PDF, SVG preview, and the paint it links) | 3 888 070 | 2 036 367 | 761 043 | 572 837 | +1 220 431 |
| **`editor`** artefact (paint + edit) | 7 294 946 | **3 727 270** | 1 326 016 | 937 508 | |
| **`pdf`** artefact | 4 215 175 | **2 238 989** | 849 494 | 634 483 | |
| **`full`** artefact (default, what the UI vendors today) | 8 913 417 | **4 654 547** | 1 680 876 | 1 193 679 | |

The CRDT has no export of its own (it is reached through the editor), so it is not separable by
feature; by symbol in the full build before `wasm-opt`, `cdoc-crdt`'s own functions are 4.5 % of
the code (the generic code it instantiates is counted under `serde` and `alloc`). Before the
editor gained file I/O (`EditorHandle.export` / `fromDocument`), the editor without formats was
2 239 207 bytes; keeping the formats in it costs 1.18 MB and keeps opening a `.docx` to one
instance and one parse, the path the headline `docx_open` comparison measures. The split takes
the PDF writer out of the artefact the canvas loads: 3.73 MB, 467 KB under the UI's 4 MiB precache
cap, where the full binary (4.65 MB) is not.

Measured and not landed:

- `opt-level = "z"`: full 3 839 189 bytes (-11.2 %; brotli 1 017 243), editor without formats
  1 957 824 (-12.6 %), but the bench went red on 30 runtime metrics over budget (among them
  `crdt_keystroke/1p` 331 > 267 ns allowed, `crdt_type/10k_chars` 1.8x, `layout_full/100p`
  74.6 > 63.1 ms, `paint_encode/100p` 5.70 > 4.47 ms; PR #25). Speed wins.
- `panic = "abort"`: already the `wasm32-unknown-unknown` default; nothing to gain.
- `wasm-opt` passes on the editor artefact (pre-file-I/O build, z): `--converge` -0.04 %, a
  second `-Oz` -0.03 %, stripping producers and target features -0.01 %.
- A size-oriented allocator would be a new dependency and is not allowed.

## Snapshot size

`crdt_snapshot_bytes/100k_churn` is a document that took 100 000 random short inserts and
deletes (`benches/common/workload.rs`) and was never collected. Release 1 ships with collection
off, so this is the size an offline-tolerant document grows to.

| Snapshot | Bytes |
|---|---|
| Format 2, snapshot layout 1 (absolute ids) | 1 538 768: 806 836 for 74 924 items, 731 248 for 50 115 delete operations |
| Format 2, snapshot layout 2 (positions, gaps) | 915 534 |

**Do not chase 0.8 MB.** That was format 1's size, and format 1 did not record who deleted
what: its deletions were implied by the tombstones, with no clock. Format 2 records each
deletion as an operation of its deleter. A loaded replica has to send those operations to a
peer that lacks them, so a snapshot without collection keeps every one. Each of the 50 115
deletes falls at a random place among about 75 000 tombstone pieces. Naming one costs about
log2(75 000), roughly 16 bits, however it is coded. That puts the deletes alone at about 100 to
120 KB, and the items have their own floor: their origins and clocks are random on this
workload. Getting much below layout 2 needs an entropy coder (a new dependency or a custom
one) or dropping the delete clocks, which is a semantic change. Neither is planned.

Layout 2 also writes Word's 14 built-in styles as one marker operation when a document still
holds them exactly as `from_model` wrote them. The 1-page snapshot drops from 7 810 to
4 897 bytes. Loading it still creates the 14 registers, which is where the 1-page decode
time goes, so the decode budget does not move.

## Running locally (smoke only)

Local numbers are for checking that the harness runs; they are not comparable with the runner's
and never set a budget.

```sh
export CARGO_TARGET_DIR=...   # your own target dir
cargo build --release -p cdoc-bench --features competitors
scripts/build-wasm.sh /tmp/wasm   # needs wasm-bindgen (Cargo.lock's version), wasm-opt, brotli
bin=$CARGO_TARGET_DIR/release/cdoc-bench
$bin corpus --out /tmp/corpus --pages 1,10,100,500
$bin run --corpus /tmp/corpus --pages 1,10,100,500 --plan bench/plan.toml \
  --wasm full=/tmp/wasm/cdoc_wasm_full_bg.wasm --wasm editor=/tmp/wasm/cdoc_wasm_editor_bg.wasm \
  --wasm convert=/tmp/wasm/cdoc_wasm_convert_bg.wasm \
  --machine local --commit local --out /tmp/ours.json
$bin competitors --corpus /tmp/corpus --pages 1,10,100,500 --plan bench/plan.toml --out /tmp/comp
$bin report --results /tmp/ours.json --competitors /tmp/comp \
  --competitors bench/competitors/results --budgets bench/budgets.toml \
  --out /tmp/bench-results.json --markdown /tmp/bench-results.md
```

mammoth needs Node (`bench/competitors/mammoth/run.sh`); LibreOffice needs `soffice` and
`python3-uno` (`bench/competitors/libreoffice/run.sh`). The per-crate criterion benches
(`cargo bench -p cdoc-shape`, `-p cdoc-layout`, `-p cdoc-para`, `-p cdoc-paint`, `-p cdoc-crdt`)
remain for profiling one crate; the harness is the one that gates.
