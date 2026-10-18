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

## Word, Google Docs, ONLYOFFICE: not measured yet

None of them runs headless on a Linux runner, so none has a number. Their cells are typed
placeholders in `placeholders/*.json` (`"status": "not measured"`). Do not replace a placeholder
with a number measured any other way than the protocol below.

### Wave 3 protocol (Playwright, in a browser)

Same machine for every product, one product at a time, Chromium stable at a pinned version,
1920 × 1080 window, a fresh profile per product, the network quiet (no other tabs), each step
repeated with the iterations `bench/plan.toml` names for `tool/metric`, results written as a
`RawRun` (`tool`, `version`, `samples`, `unmeasured`) and summarised with `cdoc-bench summarize`.
Signed-in accounts are test accounts; no document leaves the corpus.

1. **Open** (`docx_open/100p`, `docx_open/500p`): upload the corpus `.docx` through the product's
   own import, start the clock at the file input's `change`, stop at the first animation frame
   after the last page's text is in the DOM (or, for canvas renderers, after the page count shows
   the full count). `performance.now()` both ends, in the page.
2. **Keystroke to paint** (`layout_keystroke/100p`, `/500p`): caret in the middle body paragraph
   (the one `corpus::keystroke_target` picks), type one character; start at `keydown`
   (`event.timeStamp`), stop at the second `requestAnimationFrame` callback after it (the frame
   that painted it). 200 keystrokes, alternating type and delete.
3. **Scroll** (`scroll/500p`): from page 1, `window.scrollBy` one viewport per frame to the end;
   report the per-frame time (`requestAnimationFrame` deltas) median and p95, and the count of
   frames over 16.7 ms.
4. **Export** (`docx_to_pdf/100p`, `/500p`): the product's own PDF export, from the command to the
   download's `finished` event (Playwright `download.path()` resolves).
5. **Memory** (`memory/500p`, bytes, not nanoseconds): after the 500-page open settles,
   `performance.measureUserAgentSpecificMemory()` (needs cross-origin isolation; where the
   product is not isolated, record `"not measured"` with that reason, never an estimate).

Ours runs through the same protocol in the same browser in wave 3, so those rows compare like
with like; the native rows above stay as they are.
