# Citadel Docs

A Word-class document editor core for the Citadel Workspace, in pure Rust compiled to WASM.
Every core crate is pure: no I/O, no clock, no randomness; fonts, images, time and ids come in
through ports.

| Crate | Holds |
|---|---|
| `cdoc-model` | the document model, the single source of truth for the editor schema |
| `cdoc-pm` | the lossless mapping between the model and ProseMirror JSON, plus the generated schema |
| `cdoc-wasm` | the thin wasm-bindgen facade |

See `ARCHITECTURE.md` for the crate graph and the ports, and `docs/tiptap-schema.md` for the
TipTap extension spec the UI implements.

Licensed under MIT or Apache-2.0, at your option, like the Citadel SDK.
