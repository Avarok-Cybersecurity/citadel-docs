// Adapted from rMazing crates/cc-export/src/layout/flow.rs
// Copyright (c) Thomas Braun. Relicensed MIT OR Apache-2.0 by the copyright holder, 2026-10-04.

//! Strips set downwards into columns on sheets: the cursor, the columns of the current region,
//! the footnotes each column must keep room for at its foot, and the rule that nothing is set
//! below the bottom margin unless it is taller than a whole column on its own.
//!
//! A strip taller than a column cannot be kept anywhere, and pretending otherwise once wrote
//! text at a negative y (in the file, on no page, visible to nobody); such a strip is placed at
//! the top of a fresh column and reported as [`Warning::Oversized`].

mod keep;
mod notes;
mod resume;
mod section;

pub use keep::{chain_height, place_chained};
pub use resume::{Cursor, Resume};

use crate::context::Ctx;
use crate::fragment::{Fragment, Mark};
use crate::geometry::Geometry;
use crate::ir::{Draw, LinkArea, OutlineEntry, Page, Pt100, SignaturePlacement, TextRun, Warning};
use crate::LayoutError;
use cdoc_model::{BreakKind, FieldKind, FootnoteId};
use std::collections::BTreeMap;

/// A sheet and what is known about it before headers and footers are set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sheet {
    /// The page.
    pub page: Page,
    /// The section it belongs to.
    pub section: usize,
    /// Whether it is the section's first page.
    pub first_of_section: bool,
    /// Body fields waiting for the page number.
    pub fields: Vec<(FieldKind, TextRun)>,
}

/// The cursor.
pub struct Flow {
    /// Finished and current sheets; never empty.
    pub sheets: Vec<Sheet>,
    /// Headings met.
    pub outline: Vec<OutlineEntry>,
    /// Signature fields met.
    pub signatures: Vec<SignaturePlacement>,
    /// Approximations.
    pub warnings: Vec<Warning>,
    geometry: Geometry,
    /// The index of `sheets[0]` in the whole document: a resumed flow holds only its tail.
    base: usize,
    column: usize,
    region_top: Pt100,
    region_reach: Pt100,
    y: Pt100,
    notes: Vec<FootnoteId>,
    notes_height: Pt100,
    note_cache: BTreeMap<(FootnoteId, Pt100), Fragment>,
}

impl Flow {
    /// A first sheet for section 0.
    #[must_use]
    pub fn new(geometry: Geometry) -> Self {
        let sheet = Sheet {
            page: Page::new(geometry.page_width, geometry.page_height),
            section: 0,
            first_of_section: true,
            fields: Vec::new(),
        };
        Self {
            sheets: vec![sheet],
            outline: Vec::new(),
            signatures: Vec::new(),
            warnings: Vec::new(),
            base: 0,
            column: 0,
            region_top: geometry.top,
            region_reach: geometry.top,
            y: geometry.top,
            geometry,
            notes: Vec::new(),
            notes_height: 0,
            note_cache: BTreeMap::new(),
        }
    }

    fn page_index(&self) -> u32 {
        u32::try_from(self.base + self.sheets.len() - 1).unwrap_or(u32::MAX)
    }

    /// Whether anything has been set in the current column.
    #[must_use]
    pub fn column_started(&self) -> bool {
        self.y > self.region_top
    }

    pub(super) fn page_started(&self) -> bool {
        self.column > 0 || self.column_started() || self.region_top > self.geometry.top
    }

    /// The lowest y a strip may reach in this column.
    #[must_use]
    pub fn bottom(&self) -> Pt100 {
        self.geometry.bottom - self.notes_height
    }

    /// Room left in this column.
    #[must_use]
    pub fn room(&self) -> Pt100 {
        self.bottom() - self.y
    }

    /// The height a fresh column offers.
    #[must_use]
    pub const fn column_height(&self) -> Pt100 {
        self.geometry.bottom - self.geometry.top
    }

    /// Moves the cursor down without setting anything.
    pub fn advance(&mut self, by: Pt100) {
        self.y += by;
    }

    pub(super) fn finish_column(&mut self, ctx: &mut Ctx<'_, '_>) -> Result<(), LayoutError> {
        self.region_reach = self.region_reach.max(self.y);
        notes::set_at_foot(self, ctx)
    }

    pub(super) fn finish_region(&mut self) {
        let g = &self.geometry;
        if g.separator && self.column > 0 {
            let sheet = self.sheets.last_mut().expect("never empty");
            for pair in g.columns.windows(2).take(self.column) {
                let x = (pair[0].x + pair[0].width + pair[1].x) / 2;
                sheet.page.draws.push(Draw::Rule {
                    from: (x, self.region_top),
                    to: (x, self.region_reach.min(g.bottom)),
                    width: 50,
                    colour: crate::style::INK,
                });
            }
        }
    }

    /// Starts a fresh sheet.
    pub fn new_page(&mut self, ctx: &mut Ctx<'_, '_>) -> Result<(), LayoutError> {
        self.finish_column(ctx)?;
        self.finish_region();
        let section = self.sheets.last().map_or(0, |s| s.section);
        self.sheets.push(Sheet {
            page: Page::new(self.geometry.page_width, self.geometry.page_height),
            section,
            first_of_section: false,
            fields: Vec::new(),
        });
        self.column = 0;
        self.region_top = self.geometry.top;
        self.region_reach = self.geometry.top;
        self.y = self.geometry.top;
        Ok(())
    }

    /// Moves to the next column, or the next sheet after the last one.
    pub fn next_column(&mut self, ctx: &mut Ctx<'_, '_>) -> Result<(), LayoutError> {
        if self.column + 1 < self.geometry.columns.len() {
            self.finish_column(ctx)?;
            self.column += 1;
            self.y = self.region_top;
            Ok(())
        } else {
            self.new_page(ctx)
        }
    }

    /// Sets one strip at the cursor, breaking first when it does not fit (with the footnotes
    /// it references) in what is left of the column.
    pub fn place(&mut self, ctx: &mut Ctx<'_, '_>, strip: Fragment) -> Result<(), LayoutError> {
        let extra = notes::extra_for(self, ctx, &strip)?;
        if self.y + strip.height > self.bottom() - extra && self.column_started() {
            self.next_column(ctx)?;
        }
        let extra = notes::extra_for(self, ctx, &strip)?;
        if self.y + strip.height > self.bottom() - extra {
            self.warnings.push(Warning::Oversized {
                page: self.page_index(),
            });
        }
        let column = self.geometry.columns[self.column];
        let (dx, dy) = (column.x, self.y);
        let page_index = self.page_index();
        for id in strip.footnotes() {
            notes::reserve(self, ctx, id)?;
        }
        let sheet = self.sheets.last_mut().expect("never empty");
        sheet
            .page
            .draws
            .extend(strip.draws.into_iter().map(|d| d.shifted(dx, dy)));
        for mark in strip.marks {
            match mark.shifted(dx, dy) {
                Mark::Link { rect, href } => sheet.page.links.push(LinkArea { rect, href }),
                Mark::Signature { id, role, rect } => self.signatures.push(SignaturePlacement {
                    id,
                    signer_role: role,
                    page: page_index,
                    rect,
                }),
                Mark::Field { kind, run } => sheet.fields.push((kind, run)),
                Mark::Heading { level, title } => self.outline.push(OutlineEntry {
                    level,
                    title,
                    page: page_index,
                    y: dy,
                }),
                Mark::Footnote(_) => {}
            }
        }
        self.y += strip.height;
        match strip.break_after {
            Some(BreakKind::Page) => self.new_page(ctx)?,
            Some(BreakKind::Column) => self.next_column(ctx)?,
            Some(BreakKind::Line) | None => {}
        }
        Ok(())
    }

    /// Finishes the last sheet.
    pub fn finish(&mut self, ctx: &mut Ctx<'_, '_>) -> Result<(), LayoutError> {
        self.finish_column(ctx)?;
        self.finish_region();
        Ok(())
    }
}
