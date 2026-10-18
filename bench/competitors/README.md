# Competitor runners

Every competitor is measured on the same GitHub-hosted `ubuntu-latest` runner, on the same corpus
(`cdoc-bench corpus`), with the iterations `bench/plan.toml` gives it (key `tool/metric`), and
reported in the same JSON shape as ours. Scripts write raw nanosecond samples and
`cdoc-bench summarize` computes min, median and p95 exactly as it does for ours.

| Competitor | Licence | Pinned | Runner | Measures | Not comparable |
|---|---|---|---|---|---|
| docx-rs | MIT | `=0.4.22` (Cargo, feature `competitors`) | in-process | `docx_open` | layout, PDF export (it has neither) |
| rustybuzz | MIT | `=0.20.1` (Cargo, feature `competitors`) | in-process | `shape_latin`, same text, face and features (`cdoc_shape::HARFBUZZ_DEFAULTS_NOT_IMPLEMENTED` off) | - |
| yrs | MIT | `=0.21.3` (Cargo, feature `competitors`) | in-process | `crdt_type` (plain typing, one transaction per insert) | the rich-document CRDT metrics (yrs has no document model) |
| mammoth | BSD-2-Clause | `1.13.0` (`mammoth/package-lock.json`) | `mammoth/run.sh`, Node 22.11.0 | `docx_open` (docx to HTML, its only open path) | layout, PDF export |
| LibreOffice, cold | MPL-2.0 (external tool, apt) | `4:24.2.*` (`bench.yml`) | `libreoffice/run.sh` | `docx_to_pdf`: one `soffice --headless --convert-to pdf` process per run | - |
| LibreOffice, warm | as above | as above | `libreoffice/uno_bench.py` over UNO, one running `soffice --invisible` | `docx_open` (load, hidden) and `docx_to_pdf` (load + `writer_pdf_Export`) | `pdf_export` alone (its export lays out; compare `docx_to_pdf`) |

A cell a competitor cannot fill says why: `"status": "not comparable"` with a reason. The report
never leaves a blank, and a blank is never read as a win.

## Ours, ONLYOFFICE, Word, Google Docs: the browser runners

These run in a browser, so they are measured by the protocol below in `competitors.yml` (weekly,
on demand, and on PRs that change the runners), not in `bench.yml`. Their results live in
`results/<tool>.json`, one file per product, each naming its provenance (runner, Chromium build,
commit, date, workflow run); `bench.yml` reads that directory as it is committed. A result enters
the gate only by a reviewed commit of `competitors.yml`'s `competitor-results` artifact, which is
laid out as `results/` (a workflow-made PR would run no CI). Do not put a number in `results/`
that was measured any other way than the protocol below.

| Product | Licence | Pinned | Runner | Status |
|---|---|---|---|---|
| Citadel Docs, ours | ours | the UI commit in `browser/ours/ui-pin` (its vendored cdoc-wasm names its citadel-docs commit, `vendor/cdoc/PROVENANCE.json`); the harness build's packages in `browser/ours/ui.package-lock.json` | `browser/ours/prepare.sh` checks the UI out and installs it, `browser/ours/run.sh` builds its bench harness page (`scripts/lib/bench-harness`, the shipped editor opening a file, not in the app) and serves it cross-origin isolated on the runner's loopback | measured, memory included; its file is `results/ours.json` and fills the report's Docs rows |
| ONLYOFFICE Docs | AGPL-3.0 (external tool, Docker) | `onlyoffice/documentserver:10.0.0.1@sha256:ca17889f…` (`onlyoffice/run.sh`) | `onlyoffice/run.sh`: the Document Server in Docker on the runner, JWT disabled (`JWT_ENABLED=false`, everything on the runner), `host.html` (the `DocsAPI.DocEditor` embed) served by `server.ts` | measured, except memory (not cross-origin isolated) |
| Microsoft Word (web) | proprietary service | the service as it is that day (no version exposed; provenance dates it) | `word/run.sh` | needs a test account: secrets `BENCH_WORD_EMAIL`, `BENCH_WORD_PASSWORD`; without them every cell is `not measured`, reason "no test account configured". The adapter has never run: expect selector fixes on its first run |
| Google Docs | proprietary service | as above | `google-docs/run.sh` | needs a test account: secrets `BENCH_GDOCS_EMAIL`, `BENCH_GDOCS_PASSWORD`; as above. Google may refuse a scripted sign-in |

The driver (`browser/`, TypeScript, strict) is Playwright `1.63.0` with the Chromium build that
release pins (`package-lock.json`), headed under Xvfb. The accounts are test accounts the owner
creates; nothing signs in anywhere but the runner, and no document leaves the corpus.

### Measuring a product

`browser/protocol.ts` is the protocol and knows no product; a product is an `Adapter`
(`browser/types.ts`) registered in `browser/main.ts`, plus `tool/cell` entries in
`bench/plan.toml` (`cdoc-bench browser-plan` refuses a missing one). The adapter provides:

- `paint`: `protocol` (the protocol's stops as written), or `instrumented`, for a page that also
  reports its paints: each open and keystroke then stops at the later of the protocol's frame and
  the frame after the paint, so instrumenting never makes a product look faster (ours);
- `probe`: the URL fragment of the frame that holds the document and takes keystrokes, and how
  that frame shows its page count: a label with a pattern capturing current page and total, or a
  selector matching one element per laid-out page (a DOM renderer can point this at its pages);
- `open(page, docx)`: the product's own import, returning the start; through a file input the
  probe stamps its `change` (`starts`), as step 1 says;
- `placeCaret(page, frame, target)`: the product's own Find on `target.phrase` (from `cdoc-bench
  browser-plan`, the tail of `corpus::keystroke_target`), then one step right; a product without
  Find may use `target.paragraph` (`corpus::keystroke_paragraph`, the paragraphs before it in
  reading order) with its own paragraph moves, and must check where the caret landed;
- `installScroll(frame)`: `__cdocScrollStep` (one viewport; true at the end) and `__cdocScrollTop`.
  For a page that scrolls its window this is `window.scrollBy(0, innerHeight)`, as written;
- `openExport` / `exportCommand`: the menus up to the PDF command, and the command;
- `version(page)`.

The `Product` says whether it is ours (`kind`): ours' raw file carries `"recorded ours"`
provenance with the UI and engine commits (`summarize --cells browser-ours` refuses it without
them, and refuses ours' provenance on a product). For memory the probe checks
`crossOriginIsolated` in the document frame and measures only when it is true; ours' page is
served with COOP `same-origin` and COEP `require-corp`.

### Wave 3 protocol (Playwright, in a browser)

Same machine for every product, one product at a time, Chromium stable at a pinned version,
1920 × 1080 window, a fresh profile per product, the network quiet (no other tabs), each step
repeated with the iterations `bench/plan.toml` names for `tool/metric`, results written as a
`RawRun` (`tool`, `version`, `samples` in nanoseconds, `bytes`, `frames`, `unmeasured`,
`provenance`) and summarised with `cdoc-bench summarize --cells browser`, which refuses a sample
under the wrong unit, an empty sample set, an unmeasured cell without its reason, a cell filed
twice, and a run missing any of the cells below.
Signed-in accounts are test accounts; no document leaves the corpus.

1. **Open** (`docx_open/100p`, `docx_open/500p`): upload the corpus `.docx` through the product's
   own import, start the clock at the file input's `change`, stop at the first animation frame
   after the last page's text is in the DOM (or, for canvas renderers, after the page count shows
   the full count). `performance.now()` both ends, in the page.
2. **Keystroke to paint** (`layout_keystroke/100p`, `/500p`): caret in the middle body paragraph
   (the one `corpus::keystroke_target` picks), type one character; start at `keydown`
   (`event.timeStamp`), stop at the second `requestAnimationFrame` callback after it (the frame
   that painted it). 200 keystrokes, alternating type and delete, one every 100 ms.
3. **Scroll** (`scroll/500p`): from page 1, `window.scrollBy` one viewport per frame to the end;
   report the per-frame time (`requestAnimationFrame` deltas) median and p95 (`scroll/500p`), and
   the count of frames over 16.7 ms (`scroll_long_frames/500p`).
4. **Export** (`docx_to_pdf/100p`, `/500p`): the product's own PDF export, from the command to the
   download's `finished` event (Playwright `download.path()` resolves).
5. **Memory** (`memory/500p`, bytes, not nanoseconds): after the 500-page open settles,
   `performance.measureUserAgentSpecificMemory()` (needs cross-origin isolation; where the
   product is not isolated, record `"not measured"` with that reason, never an estimate).

Ours runs through the same protocol in the same browser, in the same job, so the Docs rows
compare like with like; the native rows above stay as they are, against ours measured natively
(the report's Engine table). The report compares ours with a product only when both come from
the same run: commit `ours.json` with the products' files from one `competitors-results`
artifact.

A new UI pin regenerates the lock: `node dist/browser/ours/ui-deps.js UI/package.json
OUT/package.json`, then `npm install --package-lock-only --ignore-scripts` in `OUT`, and copy its
`package-lock.json` to `browser/ours/ui.package-lock.json`.

### Where a product cannot follow the text literally

Every deviation, and nothing else:

- **Open, ONLYOFFICE: no file input.** The embed opens a document by URL, so its import (the
  Document Server's download and conversion, then the editor's load) starts at
  `new DocsAPI.DocEditor(...)`; the clock starts in the host page just before it. A fresh document
  key per open, so nothing comes from its conversion cache.
- **Open, canvas renderers: "the full count".** The count a product settles on is its own (its
  pagination is not ours), so an untimed open first waits until the count has not changed for
  10 s; that count is the full count, and each timed open stops at the first animation frame
  after the count shows it. Start and stop sit in different frames (host page, editor iframe), so
  both are `performance.timeOrigin + performance.now()`.
- **Keystroke: where the caret goes.** The caret is put in the target paragraph with the
  product's own Find (the paragraph's last words that occur once, from `cdoc-bench browser-plan`),
  then one step right: the paragraph's end, where ours types.
- **Scroll, ONLYOFFICE: the window does not scroll.** The editor scrolls its canvas, and its
  internals are minified out of reach, so one viewport per frame is one `mousewheel` event to its
  own wheel handler, with `wheelDeltaY` chosen so that its scaling (`-45 × wheelDeltaY / 120` px)
  scrolls the height of `#id_main_view`; the end is the last page among the pages its API
  reports drawn (`GetCurrentVisiblePages`, `getCountPages`); its label follows the caret.
- **Timer resolution.** A page that is not cross-origin isolated gets `performance.now()` and
  `event.timeStamp` coarsened (100 µs in Chromium); its keystroke samples are that coarse.
- **Export start.** The command is a click Playwright performs, so the start is Node's
  `process.hrtime` just before it, the stop `download.path()` resolving, as written.
- **Stalled opens.** An open that shows no page count within 60 s is abandoned and redone (at
  most two per size), as above; the protocol text has no rule for an open that never loads.
- **Browser.** Headed Chromium under Xvfb, so the GPU is software on every product alike.
- **Ours: the stops.** The editor paints in a Web Worker, so the page count can show, and the
  second frame after a keydown can come, before the pages are painted. The harness page reports
  the frame after the first page paint (open) and the frame after each edit's paint (keystroke);
  each of ours' samples is the later of that and the protocol's stop. The protocol's own stops
  are kept in the artifact's `ours-notes.json` (`<size>/protocol-stops`).
- **Ours: no Find.** The caret goes to the target paragraph by the editor's paragraph moves
  (Ctrl+Home, then Ctrl+Down `paragraph + 1` times, then Left: the paragraph's end, as Find and
  one step right leave it for the others). Before timing, a marker is typed there, the document is
  exported as text through the page's "Download as…", the phrase plus marker must occur exactly
  once, and the marker is deleted (checked by a second export).
- **Ours: scroll.** The page view is an element that scrolls (`overflow: auto`), so one viewport
  per frame is `scrollBy(0, clientHeight)` on it, as the text says for a window; the end is its
  bottom. The editor paints pages in a worker, so a frame's time does not show whether every page
  in view was painted in it.
- **Ours: timer resolution.** The page is cross-origin isolated, so its clocks are not coarsened
  to 100 µs as the products' are.
- **Ours: order.** Ours runs first in the job, before the ONLYOFFICE container is pulled.

Reading the browser rows: keystroke to paint stops at the second animation frame after the
`keydown`, so at 60 Hz it cannot read below roughly one frame (16.7 ms) for any product, ours
included; ours measured natively (no browser, no frames) is not comparable to it. ONLYOFFICE's
page count grows in steps after a first page (`*-notes.json` in the artifact records how), and
about one open in seven never finished loading the editor (its script requests aborted, no page
count ever shown). An open showing no page count 60 s after its start is closed, noted in the
artifact's `*-notes.json` (`abandoned`) and redone; more than two per corpus size fails the run.
A stalled open is never recorded as a time.
