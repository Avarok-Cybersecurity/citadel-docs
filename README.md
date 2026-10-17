# Citadel Docs

A Word-class document editor core for the Citadel Workspace, in pure Rust compiled to WASM.
Every core crate is pure: no I/O, no clock, no randomness; fonts, images, time and ids come in
through ports.

| Crate | Holds |
|---|---|
| `cdoc-model` | the document model, the single source of truth for the editor schema |
| `cdoc-pm` | the lossless mapping between the model and ProseMirror JSON, plus the generated schema |
| `cdoc-layout` | pagination and line breaking: the model to a page IR, plus list labels |
| `cdoc-paint` | the binary display list every painter draws from (`docs/display-list.md`), glyph outlines and metrics, and the SVG painter |
| `cdoc-pdf` | the IR as PDF and the SVG preview pages (painted by `cdoc-paint`); reading PDFs and appending incremental updates |
| `cdoc-docx` | `.docx` (OOXML) reader and writer, with a loss report |
| `cdoc-md` | CommonMark and GFM tables in and out, lossy and documented |
| `cdoc-txt` | plain text in and out |
| `cdoc-wasm` | the thin wasm-bindgen facade |

See `ARCHITECTURE.md` for the crate graph and the ports, and `docs/tiptap-schema.md` for the
TipTap extension spec the UI implements.

Licensed under MIT or Apache-2.0, at your option, like the Citadel SDK. The default faces in
`fonts/` (Carlito, metric-compatible with Calibri) are under the SIL Open Font License 1.1;
see `fonts/OFL-Carlito.txt` and `fonts/manifest.json`.
