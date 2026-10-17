# Architecture

Citadel Docs is a set of pure Rust crates compiled to WASM. "Pure" is literal: a core crate
performs no I/O, reads no clock, draws no randomness and touches no global state. Whatever it
needs from the world (font bytes, image bytes, the time, a fresh id) comes in through a port the
caller implements. This is SBIO: business logic on one side, I/O on the other, with a trait in
between. It is why the cores are testable without mocks and why the same binary runs in a Web
Worker, a CLI and a server.

## Crate graph

```mermaid
graph TD
    model[cdoc-model<br/>the document model, SSOT]
    pm[cdoc-pm<br/>model ↔ ProseMirror JSON<br/>generated schema]
    layout[cdoc-layout<br/>pagination, line breaking → layout IR<br/><i>adapted from rMazing</i>]
    para[cdoc-para<br/>Knuth-Plass breaking, Liang hyphenation<br/>integer only, no deps but thiserror]
    paint[cdoc-paint<br/>IR → binary display list<br/>glyph outlines, SVG painter]
    pdf[cdoc-pdf<br/>IR → PDF, SVG preview, PDF read/annotate]
    docx[cdoc-docx<br/>OOXML read/write]
    md[cdoc-md<br/>CommonMark + GFM in/out]
    txt[cdoc-txt<br/>plain text in/out]
    sign[cdoc-sign<br/>envelopes, ML-DSA + PAdES<br/><i>later</i>]
    wasm[cdoc-wasm<br/>wasm-bindgen facade, Web Worker]
    crdt[cdoc-crdt<br/>rich-text CRDT, binary sync]

    pm --> model
    crdt --> model
    layout --> model
    layout -.-> para
    docx --> model
    md --> model
    md --> layout
    txt --> model
    txt --> layout
    paint --> layout
    paint --> model
    pdf --> layout
    pdf --> paint
    sign --> pdf
    sign --> model
    wasm --> pm
    wasm --> model
    wasm -.-> layout
    wasm -.-> pdf
    wasm -.-> paint
    wasm --> docx
    wasm --> md
    wasm --> txt
    wasm -.-> sign
```

Arrows point at dependencies. `cdoc-model` depends on nothing but `serde` and `thiserror`.
Nothing depends on `cdoc-wasm`.

## cdoc-model: the single source of truth

A `Document` is a tree:

```
Document
├── meta, settings
├── styles: StyleSheet { defaults, styles: [Style] }      ← the cascade
├── numbering: { definitions: [AbstractNumbering], instances: [NumberingInstance] }
├── sections: [Section]
│   ├── properties: page setup, headers {default, first, even}, footers {…}, start, title_page
│   └── blocks: [Block = Paragraph | Table]
│       ├── Paragraph { properties, inlines: [Inline] }
│       │     Inline { content, marks }
│       │       content: Text | Image | Break | Tab | FootnoteRef | Field | FormField | SignatureField
│       │       marks:   link, comments[], revision (insertion | deletion)
│       └── Table { properties, column_widths, rows: [Row { cells: [Cell { blocks }] }] }
├── footnotes: [Footnote { id, blocks }]
└── comments:  [Comment { id, author, date, text, resolved, parent }]
```

Design decisions other crates rely on:

- **Units are Word's integers.** `Length` is hundredths of a point (`Pt100`, the unit the
  layout IR already uses, so model → layout needs no conversion), `FontSize` is half-points,
  `BorderWidth` is eighths of a point, table percentages are fiftieths of a percent. A `.docx`
  value survives a round trip unchanged; no floats cross a serialisation boundary.
- **Every property is `Option`.** `None` inherits through the cascade; `Some(false)` explicitly
  turns an inherited property off. `RunProperties::none()` / `ParagraphProperties::none()` are
  the named empty values; there is no `Default` on configuration-like types.
- **The cascade is a function, not a cache.** `StyleSheet::resolve_run(paragraph_props,
  run_props)` and `resolve_paragraph(props)` walk `based_on` chains (defaults → paragraph
  style → character style → direct) and return the effective properties. Cycles and unknown
  styles are `StyleError`s.
- **Lists are flat.** A paragraph carries `numbering: { num_id, level }`; the definitions live
  in `Document.numbering`. This is Word's model and the only one `.docx` round-trips. Labels
  (`1.`, `1.1`, `•`) are computed, never stored.
- **Range annotations are marks.** Hyperlinks, comment anchors and tracked changes sit on each
  inline, not as containers, so they map one-to-one to ProseMirror marks and overlapping
  comments need no tree surgery. The `.docx` writer groups adjacent inlines with equal marks.
- **Merged cells are spans.** `col_span` / `row_span` on the top-left cell, as ProseMirror and
  HTML do; `.docx`'s `gridSpan` / `vMerge` is the writer's problem.
- **Validation is total.** `Document::validate()` returns *every* defect (`Vec<ValidationError>`),
  so an importer can report them all. A valid document is one every exporter and the layout
  crate can take without defensive code.
- **Walkers are shared.** `Document::blocks()`, `paragraphs()`, `inlines()`,
  `run_properties()` reach headers, footers, footnotes and nested cells, so no crate re-derives
  the containment rules. `word_count()` is built on them.
- **Test support ships with the model.** Behind the `test-support` feature:
  `test_support::strategies` (proptest generators for every type, boxed so deep trees do not
  overflow the stack) and `test_support::fixtures::kitchen_sink()` (valid, uses every variant
  once, including a nested table and all three header kinds). Dependent crates test round trips
  against these rather than writing generators of their own.

## cdoc-pm: the editor's schema, generated

`to_pm` / `from_pm` map the model to ProseMirror JSON losslessly (`from_pm(to_pm(d)) == d`
for every generated document). The mapping rules:

| Model | ProseMirror |
|---|---|
| `Document` | `doc` node; `meta`, `settings`, `styles`, `numbering`, `comments` as attrs |
| `Section` | `section` node; `start`, `page`, `titlePage` attrs; `header`/`footer` children first, then blocks |
| `Paragraph` | `paragraph`; **attrs are exactly the serde fields of `ParagraphProperties`** |
| `Table` / row / cell | `table` (+`columnWidths`), `tableRow`, `tableCell` / `tableHeader` with `colspan`/`rowspan` (prosemirror-tables' spelling) |
| `Text` run | `text` node; `RunProperties` → marks (`bold{on}`, `textStyle{fontFamily,fontSize,color}`, …) |
| other inlines | atoms: `image`, `hardBreak`, `pageBreak`, `columnBreak`, `tab`, `footnoteRef`, `field`, `formField`, `signatureField` |
| `Marks` | `link`, `comment{id}` (repeatable), `insertion`/`deletion{id,author,date}` |
| `Footnote` | `footnotes` > `footnote{id}` at the end of `doc` |

**Schema-sync mechanism.** `cdoc_pm::schema::schema()` builds the schema as data from the same
name constants (`names.rs`) the encoder and decoder use, with attribute lists taken from the
serde field names of fully populated model values (`schema/full.rs`). It is checked in as
`schema/prosemirror-schema.json` (snapshot test), exported by `cdoc-wasm::schema_json()`, and
every encoded document is verified against it by `schema::check` inside the round-trip property
tests. The UI compares its TipTap `editor.schema` to the same JSON in a vitest
(`docs/tiptap-schema.md` § Drift gate). A field added to the model therefore fails CI in
`cdoc-pm` until the schema knows it, and fails CI in the UI until the extension knows it.

## cdoc-para: the paragraph builder

Engine-neutral: it knows TeX's items (boxes, glue, penalties, hyphens), not fonts or documents,
so the Word-style layout (wave 2, replacing `wrap.rs`'s greedy decision) and a later TeX engine
feed it the same way. `KnuthPlass` runs TeX's three passes (pretolerance without pattern
hyphens, tolerance, emergency stretch) with `tex.web`'s integer badness, fitness classes and
demerits, `\parshape`-style `LineWidths` and `\looseness`; when no pass finds feasible breaks it
falls back to `FirstFit` and says so in the `BreakReport`. `justify` shares a line's slack in
integers so the glue widths sum to the measure exactly. `Hyphenator` compiles TeX pattern files
at runtime into a packed trie; the shipped files (`patterns/`) are hyph-utf8's, unmodified, MIT
or all-permissive only. The evidence is TeX itself: `tests/oracle/` runs TeX on the same items
and words and writes the golden data the tests compare against.

## cdoc-paint: one paint path

Every painter draws from the **display list**, a compact versioned binary description of the
pages (`docs/display-list.md` is the byte-level contract): the canvas editor (TypeScript,
Canvas2D, next wave), the SVG preview, and later the PDF writer. Lowering from the layout IR
turns text into glyph ids through each face's `cmap`, with a per-face glyph table carrying the
`hmtx` advances, so a painter needs neither the text nor a shaper; unshaped runs are placed by
one integer rule (`nominal_pen`) that the layout measures with too. A page index makes every
page seekable, `encode_pages` writes only the pages an edit changed, and `decode` reads in
place over the borrowed bytes. `glyph::glyph_path` and `glyph::face_metrics` serve outlines in
font units for a glyph atlas. `cdoc_pdf::svg::pages_svg` is a thin forwarder over
`cdoc_paint::svg` (adding only the image-type check), and a test in `cdoc-pdf` holds the
display list's glyphs, sizes and pen positions equal to what the PDF's content stream places,
read back with `lopdf`. The PDF writer still draws from the IR; moving it onto the display list
is the follow-up.

## Ports

The cores take the world through traits the caller implements. Names are proposals for the
crates that need them; `cdoc-model` and `cdoc-pm` need none.

| Port | Used by | Shape |
|---|---|---|
| `FontPort` | layout, pdf | `fn face(&self, family: &str, bold: bool, italic: bool) -> Result<FaceSource<'_>, FontError>` — the bytes of a font file plus the face actually served, so substitutions are reported; the layout crate parses and measures with `ttf-parser`. `cdoc_layout::FontLibrary` is the in-memory implementation (aliases, explicit fallback) the facade and tests share |
| `ImagePort` | pdf, docx (writer) | `fn image(&self, id: &ContentId) -> Result<ImageBytes<'_>, ImageError>` — bytes plus declared MIME; decoding (and checking the bytes are that type) stays in the core. The layout needs no images: the model carries each image's size |
| `ClockPort` | pdf, sign, UI glue | `fn now_rfc3339(&self) -> String` — the model stores dates as text; the PDF backend dates the file and each appended annotation, the signing crate its audit events |
| `IdPort` | UI glue, sign | `fn fresh_id(&self) -> String` — comment, field and revision ids; the browser supplies `crypto.randomUUID()` |

In the browser the WASM facade is the only place these are wired, from JS, inside the Web
Worker. In tests they are plain structs. Never implement a port by calling `std::fs`,
`std::time` or `getrandom` from inside a core crate; `scripts/dep-gate.sh` forbids those crates
in the wasm32 build.

`ImagePort` lives in `cdoc_model::ports` (re-exported by `cdoc-pdf`) because two crates take images
from it. The `.docx` reader needs the opposite direction, a place to put the images it finds, so
`cdoc-docx` adds one more port, `MediaSink::store(ImageBytes) -> ContentId`: the caller keeps the
bytes and says what id the model should name them by.

## cdoc-wasm

A thin layer: every export is one line over `facade.rs` (and `convert.rs`, the format
conversions it re-exports), which is string-in string-out JSON and
tested natively. Inputs and outputs are JSON text: the browser has a parser, nothing in the
payload is a `BigInt`, and one ABI type keeps the boundary trivial. The paint path is the exception: the display list, glyph paths and face metrics cross as bytes (a `Uint8Array` the worker can transfer), never as JSON. Built for
`wasm32-unknown-unknown` only (never WASI). Format conversions: `docx_to_pm`, `pm_to_docx`,
`md_to_pm`, `pm_to_md`, `txt_to_pm`, `pm_to_txt`; imports return `{pm, report, problems}`, exports
return the file or `{text, report}`; images cross in an `ImageStore`. The CI gate checks the artefact imports nothing but
wasm-bindgen's own glue modules.

## Collaboration layer

The UI's editor is TipTap v2 over Yjs (`y-prosemirror`). y-prosemirror syncs the document
*fragment*, not the top-level `doc` node's attributes, so in the live editor the document-level
attrs (`meta`, `settings`, `styles`, `numbering`, `comments`) live in `ydoc.getMap('cdoc')`
under the same keys, and the UI bridge assembles a full `doc` node (fragment + map) before
calling `cdoc-wasm`. The mapping in `cdoc-pm` is unaffected; the JSON shape is the interchange
format on both sides.

`cdoc-crdt` is the replacement for Yjs in that picture (`yrs` fails the wasm32 dependency gate
through `fastrand -> getrandom`). A document is one flat sequence of characters, inline atoms
and structural atoms (section, header/footer, footnote, paragraph, table, row, cell, end), a
YATA list CRDT over run-length items; formatting is Peritext-style mark ranges anchored to unit
ids; document attributes and block properties are multi-value registers ordered by vector
clocks. `to_model` / `from_model` map it to the model losslessly; updates and snapshots are a
documented varint binary format with no floats; `apply_update` buffers out-of-order operations
and reports the dirty blocks for incremental layout. The crate docs hold the design and the
format; `benches/` compare it with `yrs`.

## Later crates: how they build on the model

- **cdoc-layout** consumes a `Document`, resolves every paragraph and run through
  `StyleSheet::resolve_*`, and emits a layout IR of pages. Reuse from rMazing
  (`~/rMazing/crates/cc-export/src/layout/`): `ir.rs` (`Pt100`, `Rgb`, `Draw`, `Page` — the
  same unit as `Length`), `line.rs` and `flow.rs` (lines set downwards with keep-together
  rules), `wrap.rs` (measured wrapping), `fonts.rs` (faces parsed once, glyph advances,
  `sanitise`). Each copied file carries a provenance header; see Licensing. What changes:
  `Face` becomes a resolved `(family, bold, italic)` from the cascade rather than four fixed
  faces, `Geometry` comes from `PageSetup`, and columns, headers/footers and footnotes are new.
  Layout writes `SignatureField.anchor` (`PageAnchor`) so signing knows where a field landed.
  **Incremental relayout.** `LayoutSession` keeps the last layout and is told what changed
  (`Dirty`: block edits per section plus named flags; `Dirty::all()` assumes nothing). Blocks
  are measured and broken into strips once and cached by (128-bit fingerprint of the model
  block via its `Hash` derive, style epoch, width, list label / counters / footnote numbers it
  reads); placement resumes at the first changed block from the state recorded there and stops
  at the first later boundary where the flow state (column, y, region, pending footnotes,
  counters, numbering) equals the previous run's, splicing the old tail in with page indices
  shifted; headers and footers are set again only where what they print changed. Face ids are
  replayed into a fresh layout's first-use order, so `layout()` (which is a fresh session's
  `Dirty::all()` relayout) and every incremental relayout produce identical IR.
- **cdoc-pdf** renders the IR (`pdf-writer`, no C; faces subset with `subsetter`) and the SVG
  preview from the same IR, so preview and file cannot disagree (rMazing's two-backend
  pattern). rMazing's `printpdf` and `lopdf` are not used in the shipped build: both pull
  `getrandom`, `rand` and `time` into wasm32, which the dependency gate forbids; `lopdf` stays
  as the tests' independent reader. Existing PDFs are read with `hayro-syntax` (page count,
  boxes), and annotations and unsigned signature fields are appended as incremental updates
  that leave the original bytes a prefix of the result, the base `cdoc-sign` builds on.
- **cdoc-docx** reads and writes `.docx` with `quick-xml` and `zip` (deflate only, no C). Bytes
  in, bytes out; images go through `ImagePort` (writing) and `MediaSink` (reading). The model's
  units are OOXML's, so the mapping is arithmetic-free except `Length::from_twips`. A part is
  read by a function that names its children one by one; anything it does not name is an entry
  in the returned `Report` (an element path and a count), never a silent drop, and a part the
  reader never opened is reported too. Where the format has no slot for a model value (string
  ids, a form field's maximum length, a signature field's anchor) the writer carries it in a
  native construct that Word shows (a content control) or in a small sidecar part
  (`cdoc/ids.xml`), so a document survives `model -> docx -> model` unchanged. See the crate
  docs for what is mapped and the `tests/` for the evidence.
- **cdoc-sign** owns envelopes and signatures; from the model it needs only `SignatureField`
  (`id`, `signer_role`, `anchor`) and the document digest.
- **cdoc-md / cdoc-txt** are lossy by nature; each documents its mapping in its crate docs and
  reports every kind of loss. `cdoc-md` is `pulldown-cmark` (pure Rust, MIT) in, and a writer
  that escapes for exact read-back out; `cdoc-txt` has no dependencies of its own. Both take
  their reader options explicitly (no defaults), use `cdoc_layout::labels::Counters` for list
  labels and `Table::placements` for spans, so no crate re-derives them.

## Repository rules

- Every file ≤ 250 lines (`scripts/check-file-length.sh`, in CI). Split, never exempt.
- clippy 1.99 pedantic with `-D warnings`, rustfmt clean, `#![forbid(unsafe_code)]`.
- TDD with a negative control per fix: break the code, see the test go red, restore, verify
  the file by checksum. Controls that stay green are discarded, not reported.
- No `Default` on config-like types; named constructors (`Document::blank()`,
  `StyleSheet::word_builtin()`, `PageSetup::a4_normal()`) make the choice visible at the call
  site.
- Dependencies: MIT / Apache-2.0 / BSD only; no `ring`, OpenSSL, `*-sys`, randomness or clock
  crates in the wasm32 build (`scripts/dep-gate.sh`).

## Licensing

`MIT OR Apache-2.0`, matching the Citadel SDK (`citadel_sdk` is `MIT OR Apache-2.0`;
`citadel-internal-service` is MIT; `intersession-layer-messaging` is Apache-2.0; the
`citadel-workspace` parent and the UI carry no licence file). Dual licensing is the superset
that every one of those can consume.

The rMazing `cc-export` layout sources are AGPL-3.0 in their home repository. Their sole
copyright holder (Thomas Braun) has approved copying them into this repository under
`MIT OR Apache-2.0`. Each adapted file starts with:

```
// Adapted from rMazing crates/cc-export/src/layout/<file>.rs
// Copyright (c) Thomas Braun. Relicensed MIT OR Apache-2.0 by the copyright holder, 2026-10-04.
```
