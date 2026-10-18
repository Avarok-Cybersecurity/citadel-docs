//! The cursor between two blocks, as a value: recorded at every block boundary so a later
//! layout can resume there, and compared so it can tell when it has caught up with the
//! previous one.

use super::{Flow, Sheet};
use crate::fragment::Fragment;
use crate::geometry::Geometry;
use crate::ir::{OutlineEntry, Pt100, SignaturePlacement, Warning};
use cdoc_model::FootnoteId;
use std::collections::BTreeMap;

/// Everything the rest of the placement depends on. Two flows in equal states, given the same
/// remaining blocks, place them identically.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CursorState {
    /// The section the current sheet belongs to.
    pub sheet_section: usize,
    /// Whether the current sheet is its section's first.
    pub first_of_section: bool,
    /// The current sheet's size.
    pub sheet_size: (Pt100, Pt100),
    /// The column being filled.
    pub column: usize,
    /// The top of the current column region.
    pub region_top: Pt100,
    /// The lowest point reached in the region.
    pub region_reach: Pt100,
    /// The cursor.
    pub y: Pt100,
    /// Footnotes reserved in this column.
    pub notes: Vec<FootnoteId>,
    /// Their height, separator included.
    pub notes_height: Pt100,
    /// The footnotes laid out so far, by width.
    pub laid: Vec<(FootnoteId, Pt100)>,
}

/// Where the cursor stands in the output, and its state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cursor {
    /// The index of the current sheet.
    pub page: usize,
    /// Drawings already on the current sheet.
    pub draws: usize,
    /// Links already on the current sheet.
    pub links: usize,
    /// Caret lines already on the current sheet.
    pub lines: usize,
    /// Body fields already on the current sheet.
    pub fields: usize,
    /// Outline entries so far.
    pub outline: usize,
    /// Signature fields so far.
    pub signatures: usize,
    /// Warnings so far.
    pub warnings: usize,
    /// The state.
    pub state: CursorState,
}

/// What a resumed flow starts from: the sheet being filled, cut back to the cursor, and
/// everything collected before it.
pub struct Resume {
    /// The current sheet.
    pub sheet: Sheet,
    /// Outline entries so far.
    pub outline: Vec<OutlineEntry>,
    /// Signature fields so far.
    pub signatures: Vec<SignaturePlacement>,
    /// Warnings so far.
    pub warnings: Vec<Warning>,
    /// The footnotes already laid out, for the cursor's `laid`.
    pub note_cache: BTreeMap<(FootnoteId, Pt100), Fragment>,
}

impl Flow {
    /// The cursor now.
    #[must_use]
    pub fn cursor(&self) -> Cursor {
        let sheet = self.sheets.last().expect("never empty");
        Cursor {
            page: self.base + self.sheets.len() - 1,
            draws: sheet.page.draws.len(),
            links: sheet.page.links.len(),
            lines: sheet.page.lines.len(),
            fields: sheet.fields.len(),
            outline: self.outline.len(),
            signatures: self.signatures.len(),
            warnings: self.warnings.len(),
            state: CursorState {
                sheet_section: sheet.section,
                first_of_section: sheet.first_of_section,
                sheet_size: (sheet.page.width, sheet.page.height),
                column: self.column,
                region_top: self.region_top,
                region_reach: self.region_reach,
                y: self.y,
                notes: self.notes.clone(),
                notes_height: self.notes_height,
                laid: self.note_cache.keys().copied().collect(),
            },
        }
    }

    /// A flow standing at `cursor` in `geometry`, holding only the current sheet.
    #[must_use]
    pub fn resume(geometry: Geometry, cursor: &Cursor, from: Resume) -> Self {
        let state = &cursor.state;
        Self {
            sheets: vec![from.sheet],
            outline: from.outline,
            signatures: from.signatures,
            warnings: from.warnings,
            geometry,
            base: cursor.page,
            column: state.column,
            region_top: state.region_top,
            region_reach: state.region_reach,
            y: state.y,
            notes: state.notes.clone(),
            notes_height: state.notes_height,
            note_cache: from.note_cache,
        }
    }

    /// The footnotes laid out, handed over once the flow is finished.
    pub fn take_note_cache(&mut self) -> BTreeMap<(FootnoteId, Pt100), Fragment> {
        std::mem::take(&mut self.note_cache)
    }
}
