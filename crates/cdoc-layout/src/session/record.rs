//! What a session remembers between layouts: per block its fingerprint, its measurement and
//! where it was placed; per page what its body was before headers and footers went on.

use crate::context::Trace;
use crate::flow::Cursor;
use crate::fragment::FieldRun;
use crate::fragment::{Block, Fragment};
use crate::ir::{Layout, Pt100, Warning};
use crate::labels::Counts;
use cdoc_model::FootnoteId;
use std::collections::BTreeMap;
use std::sync::Arc;

/// A block measured and broken into strips, before placement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Measured {
    /// The strips and pagination flags.
    pub block: Block,
    /// What measuring it did.
    pub trace: Trace,
    /// The list counters after it, for a table (whose cells count on them); a paragraph's
    /// label is counted before the cache is consulted.
    pub counts_after: Option<Counts>,
}

/// The state at a block boundary: the cursor when the block was placed, and the counters and
/// footnote numbering when it was measured.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Boundary {
    /// The cursor.
    pub cursor: Cursor,
    /// The list counters.
    pub counts: Counts,
    /// How many footnotes were numbered.
    pub numbered: usize,
}

/// One body block.
#[derive(Debug, Clone)]
pub struct BlockRecord {
    /// The fingerprint of the model block.
    pub content: u128,
    /// The footnotes it references, which its measurement depends on.
    pub refs: Vec<FootnoteId>,
    /// Its measurement, once measured.
    pub measured: Option<Arc<Measured>>,
    /// Where it was placed, once placed.
    pub at: Option<Boundary>,
    /// What placing it did.
    pub place: Trace,
}

/// One section.
#[derive(Debug, Clone)]
pub struct SectionRecord {
    /// Its blocks.
    pub blocks: Vec<BlockRecord>,
    /// What starting it did (nothing for the first section).
    pub start: Trace,
    /// The state after its last block.
    pub end: Option<Boundary>,
}

/// Which header and footer a page shows and the field values they print.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FurnitureKey {
    /// The section.
    pub section: usize,
    /// Its title page (first header and footer).
    pub first: bool,
    /// An even page, when even pages have their own.
    pub even: bool,
    /// The page number, when something on the page prints it.
    pub number: Option<String>,
    /// The page count, when something on the page prints it.
    pub count: Option<usize>,
}

/// One page.
#[derive(Debug, Clone)]
pub struct PageMeta {
    /// Drawings that are the body; the rest are headers, footers and fields.
    pub body_draws: usize,
    /// Links that are the body.
    pub body_links: usize,
    /// Caret lines that are the body (its text and footnotes).
    pub body_lines: usize,
    /// Body fields waiting for the page number, as placed.
    pub fields: Vec<FieldRun>,
    /// The section.
    pub section: usize,
    /// Whether it is its section's first page.
    pub first_of_section: bool,
    /// What it was furnished for.
    pub key: FurnitureKey,
    /// What furnishing it did.
    pub trace: Trace,
}

/// Everything a session keeps from its last successful layout.
pub struct Baseline {
    /// The layout handed out.
    pub layout: Layout,
    /// Per section.
    pub sections: Vec<SectionRecord>,
    /// Per page.
    pub pages: Vec<PageMeta>,
    /// The flow's own warnings (oversized strips), before font warnings.
    pub flow_warnings: Vec<Warning>,
    /// Body footnotes in numbering order.
    pub note_order: Vec<FootnoteId>,
    /// Footnotes laid out, by width.
    pub note_cache: BTreeMap<(FootnoteId, Pt100), Fragment>,
    /// What finishing the last sheet did.
    pub finish: Trace,
}
