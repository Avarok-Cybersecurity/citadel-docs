//! The R1 metrics, defined once. Each family has one name; a page-scaled metric is
//! `family/Np`. Every run of ours goes through [`Ctx::time`], so warmup, iteration counts and
//! statistics are the same for every metric.
//!
//! A family joins by adding a variant here, a module that times it, a plan entry and a budget;
//! the gate refuses a metric without a budget, so nothing can join unbudgeted.

pub mod crdt;
mod crdt_sync;
mod docs;
mod editor;
mod layout;
mod paint;
mod text;

#[cfg(feature = "competitors")]
pub mod competitors;

pub use docs::MD_OPTIONS;

use crate::plan::{Plan, PlanError};
use crate::schema::Measurement;
use crate::stats::{sample, Clock, StatsError, TimeSummary};
use cdoc_model::Document;
use std::collections::BTreeMap;

/// A metric family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    /// `.docx` bytes to model.
    DocxOpen,
    /// Model to `.docx` bytes.
    DocxSave,
    /// Markdown text to model.
    MdOpen,
    /// A fresh layout of the whole document.
    LayoutFull,
    /// Incremental relayout after a one-character edit in the middle paragraph.
    LayoutKeystroke,
    /// Laid-out pages to PDF bytes (layout excluded).
    PdfExport,
    /// `.docx` bytes to PDF bytes: open, lay out, export.
    DocxToPdf,
    /// Laid-out pages to the binary display list (lowering included).
    PaintEncode,
    /// The display list decoded with every op and glyph visited.
    PaintDecode,
    /// Shaping 10 000 Latin characters on Carlito.
    ShapeLatin,
    /// Knuth-Plass breaking of a 2 000-word hyphenated paragraph.
    ParaBreak,
    /// The model into a CRDT document.
    CrdtFromModel,
    /// A CRDT document's snapshot encoded.
    CrdtSnapshotEncode,
    /// A CRDT snapshot decoded into a document.
    CrdtSnapshotDecode,
    /// One character inserted in the middle of a CRDT document.
    CrdtKeystroke,
    /// The editor's result record with the whole body selected and a three-page viewport.
    KeystrokeSelectAll,
    /// Bytes of one shipped `cdoc-wasm` artefact (release, wasm32, wasm-bindgen, `wasm-opt -Oz`):
    /// `wasm_size/<artefact>`.
    WasmSize,
}

impl Family {
    /// The family's name, the prefix of its metric names.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::DocxOpen => "docx_open",
            Self::DocxSave => "docx_save",
            Self::MdOpen => "md_open",
            Self::LayoutFull => "layout_full",
            Self::LayoutKeystroke => "layout_keystroke",
            Self::PdfExport => "pdf_export",
            Self::DocxToPdf => "docx_to_pdf",
            Self::PaintEncode => "paint_encode",
            Self::PaintDecode => "paint_decode",
            Self::ShapeLatin => "shape_latin",
            Self::ParaBreak => "para_break",
            Self::CrdtFromModel => "crdt_from_model",
            Self::CrdtSnapshotEncode => "crdt_snapshot_encode",
            Self::CrdtSnapshotDecode => "crdt_snapshot_decode",
            Self::CrdtKeystroke => "crdt_keystroke",
            Self::KeystrokeSelectAll => "keystroke_select_all",
            Self::WasmSize => "wasm_size",
        }
    }
}

/// The name of a page-scaled metric: `family/Np`.
#[must_use]
pub fn paged(family: Family, pages: u32) -> String {
    format!("{}/{pages}p", family.name())
}

/// The shaping metric's name.
pub const SHAPE_LATIN: &str = "shape_latin/10k_chars";
/// The line-breaking metric's name.
pub const PARA_BREAK: &str = "para_break/2000_words";
/// The CRDT typing metric's name.
pub const CRDT_TYPE: &str = "crdt_type/10k_chars";
/// The bytes of the update one keystroke in a 1 MB CRDT document makes for a peer in sync.
pub const CRDT_DIFF_BYTES: &str = "crdt_diff_bytes/after_1_keystroke_in_1MB_doc";
/// The time to type that keystroke and encode that update.
pub const CRDT_DIFF: &str = "crdt_diff/after_1_keystroke_in_1MB_doc";
/// The snapshot bytes of a CRDT document after 100 000 edits of insert/delete churn.
pub const CRDT_CHURN_BYTES: &str = "crdt_snapshot_bytes/100k_churn";
/// The same snapshot after garbage collection with every peer caught up.
pub const CRDT_CHURN_GC_BYTES: &str = "crdt_snapshot_bytes/100k_churn_after_gc";
/// The bytes of the update one changed paragraph property makes for a peer in sync.
pub const CRDT_FIELD_BYTES: &str = "crdt_diff_bytes/after_1_block_field_change";
/// How many characters the CRDT typing metric types.
pub const CRDT_TYPE_CHARS: usize = 10_000;
/// How many characters the shaping metric shapes.
pub const SHAPE_CHARS: usize = 10_000;
/// How many words the line-breaking metric breaks.
pub const PARA_WORDS: usize = 2_000;

/// Why a metric could not run.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BenchError {
    /// The plan has no entry.
    #[error(transparent)]
    Plan(#[from] PlanError),
    /// No samples.
    #[error(transparent)]
    Stats(#[from] StatsError),
    /// The `.docx` reader or writer failed.
    #[error(transparent)]
    Docx(#[from] cdoc_docx::DocxError),
    /// Layout failed.
    #[error(transparent)]
    Layout(#[from] cdoc_layout::LayoutError),
    /// PDF export failed.
    #[error(transparent)]
    Pdf(#[from] cdoc_pdf::PdfError),
    /// Shaping failed.
    #[error(transparent)]
    Shape(#[from] cdoc_shape::ShapeError),
    /// Line breaking failed.
    #[error(transparent)]
    Break(#[from] cdoc_para::BreakError),
    /// Hyphenation patterns failed to compile.
    #[error(transparent)]
    Pattern(#[from] cdoc_para::PatternError),
    /// The corpus does not have what a metric needs.
    #[error("corpus: {0}")]
    Corpus(String),
    /// The display list could not be encoded or decoded.
    #[error("display list: {0}")]
    Paint(String),
    /// The CRDT failed.
    #[error("crdt: {0}")]
    Crdt(String),
    /// A competitor library failed.
    #[error("{tool}: {message}")]
    Competitor {
        /// Which.
        tool: &'static str,
        /// What it said.
        message: String,
    },
}

/// One corpus document in every form the metrics read.
#[derive(Debug, Clone)]
pub struct Fixture {
    /// The nominal page count.
    pub pages: u32,
    /// The model (from the fixture JSON).
    pub doc: Document,
    /// The `.docx` fixture.
    pub docx: Vec<u8>,
    /// The Markdown fixture.
    pub md: String,
}

/// What every metric runs against: the clock port and the plan.
pub struct Ctx<'a> {
    /// The clock.
    pub clock: &'a dyn Clock,
    /// The plan.
    pub plan: &'a Plan,
}

impl Ctx<'_> {
    /// Times `step` under the plan entry `key`.
    pub fn time(
        &self,
        key: &str,
        step: impl FnMut() -> Result<(), BenchError>,
    ) -> Result<TimeSummary, BenchError> {
        sample(self.clock, self.plan.get(key)?, step)
    }
}

/// Our metrics by name.
pub type Results = BTreeMap<String, Measurement>;

/// Runs every metric of ours over `fixtures`; `wasm_bytes` is the shipped size of each
/// `cdoc-wasm` artefact by name.
pub fn run_ours(
    ctx: &Ctx<'_>,
    fixtures: &[Fixture],
    wasm_bytes: &BTreeMap<String, u64>,
) -> Result<Results, BenchError> {
    let mut out = Results::new();
    for fx in fixtures {
        docs::run(ctx, fx, &mut out)?;
        layout::run(ctx, fx, &mut out)?;
        paint::run(ctx, fx, &mut out)?;
        crdt::run(ctx, fx, &mut out)?;
        editor::run(ctx, fx, &mut out)?;
    }
    text::run(ctx, &mut out)?;
    crdt::run_typing(ctx, &mut out)?;
    crdt_sync::run(ctx, &mut out)?;
    for (artefact, &bytes) in wasm_bytes {
        out.insert(
            format!("{}/{artefact}", Family::WasmSize.name()),
            Measurement::Size { bytes },
        );
    }
    Ok(out)
}
