//! Placement, from a boundary onwards: blocks measured (through the cache) as the flow needs
//! them, placed with their keep chains, and every boundary recorded. A placement that checks
//! for reconvergence stops at the first boundary, past the last change, where the flow stands
//! exactly as it stood in the previous layout: from there on it would only repeat itself.

use super::measure::Cache;
use super::record::{Boundary, Measured, SectionRecord};
use crate::context::{Ctx, Trace};
use crate::flow::{chain_height, place_chained, Flow};
use crate::geometry::Geometry;
use crate::ir::Pt100;
use crate::labels::{Counters, Counts};
use crate::LayoutError;
use cdoc_model::{FootnoteId, SectionStart};
use std::collections::VecDeque;
use std::sync::Arc;

/// The counters and footnote count before a block was measured.
pub type Pre = (Counts, usize);

/// One placement in progress.
pub struct Run<'r, 'd, 'f> {
    /// Shared layout state.
    pub ctx: Ctx<'d, 'f>,
    /// The body's list counters.
    pub counters: Counters<'d>,
    /// The records, rewritten as blocks are placed.
    pub records: &'r mut [SectionRecord],
    /// The paragraph cache.
    pub cache: &'r mut Cache,
    /// The style epoch.
    pub epoch: u64,
    /// The current section's column width.
    pub width: Pt100,
    /// Blocks measured but not yet placed, from block `next` on.
    pub ahead: VecDeque<(Arc<Measured>, Pre)>,
    /// The next block to place.
    pub next: usize,
}

/// When to stop early.
pub struct Catch<'o> {
    /// The first position (section, index) past every change.
    pub from: (usize, usize),
    /// The previous layout's footnote numbering.
    pub old_order: &'o [FootnoteId],
}

/// How a placement ended.
pub enum Ending {
    /// It ran to the end of the document; finishing the last sheet did this.
    Finished(Trace),
    /// It caught up with the previous layout.
    Caught(Box<Caught>),
}

/// Where a placement caught up with the previous layout: a position whose old and new states
/// differ only in where they are in the output.
pub struct Caught {
    /// The section.
    pub section: usize,
    /// The block (the section's length for its end).
    pub index: usize,
    /// The state the previous layout recorded there.
    pub old: Boundary,
    /// The state now.
    pub new: Boundary,
}

impl Run<'_, '_, '_> {
    fn caught(
        &self,
        catch: Option<&Catch<'_>>,
        at: (usize, usize),
        old: Option<&Boundary>,
        new: &Boundary,
    ) -> bool {
        let (Some(catch), Some(old)) = (catch, old) else {
            return false;
        };
        let n = new.numbered;
        let shift = new.cursor.page.abs_diff(old.cursor.page);
        let parity_free = self.ctx.doc.sections[at.0 + 1..].iter().all(|s| {
            matches!(
                s.properties.start,
                SectionStart::NextPage | SectionStart::Continuous
            )
        });
        at >= catch.from
            && old.cursor.state == new.cursor.state
            && old.counts == new.counts
            && old.numbered == n
            && catch.old_order.get(..n) == self.ctx.order.get(..n)
            && (shift % 2 == 0 || parity_free)
    }

    fn place_one(
        &mut self,
        flow: &mut Flow,
        section: usize,
        index: usize,
    ) -> Result<(), LayoutError> {
        let count = self.records[section].blocks.len();
        self.measure_to(section, index)?;
        let mut last = index;
        while last + 1 < count && self.ahead[last - self.next].0.block.keep_with_next {
            last += 1;
            self.measure_to(section, last)?;
        }
        let chain = chain_height(self.ahead.iter().map(|(m, _)| &m.block));
        let cursor = flow.cursor();
        let (measured, (counts, numbered)) = self.ahead.pop_front().expect("measured above");
        self.next += 1;
        place_chained(flow, &mut self.ctx, measured.block.clone(), chain)?;
        let record = &mut self.records[section].blocks[index];
        record.at = Some(Boundary {
            cursor,
            counts,
            numbered,
        });
        record.place = self.ctx.take_trace();
        record.measured = Some(measured);
        Ok(())
    }

    /// Places blocks from `from` with `flow` standing there, measuring each section whole
    /// first when `eager` (what a fresh layout does, so it fails on the same block).
    pub fn place(
        &mut self,
        flow: &mut Flow,
        from: (usize, usize),
        catch: Option<&Catch<'_>>,
        eager: bool,
    ) -> Result<Ending, LayoutError> {
        let doc = self.ctx.doc;
        let (mut section, mut index) = from;
        loop {
            self.width = Geometry::of(&doc.sections[section].properties.page).columns[0].width;
            self.ahead.clear();
            self.next = index;
            let count = doc.sections[section].blocks.len();
            if eager && count > 0 {
                self.measure_to(section, count - 1)?;
            }
            while index < count {
                let (counts, numbered) = self
                    .ahead
                    .front()
                    .map_or_else(|| self.pre(), |(_, p)| p.clone());
                let new = Boundary {
                    cursor: flow.cursor(),
                    counts,
                    numbered,
                };
                let old = self.records[section].blocks[index].at.as_ref();
                if self.caught(catch, (section, index), old, &new) {
                    let old = old.cloned().expect("checked");
                    return Ok(Ending::Caught(Box::new(Caught {
                        section,
                        index,
                        old,
                        new,
                    })));
                }
                self.place_one(flow, section, index)?;
                index += 1;
            }
            let (counts, numbered) = self.pre();
            let new = Boundary {
                cursor: flow.cursor(),
                counts,
                numbered,
            };
            let old = self.records[section].end.as_ref();
            if self.caught(catch, (section, count), old, &new) {
                let old = old.cloned().expect("checked");
                return Ok(Ending::Caught(Box::new(Caught {
                    section,
                    index: count,
                    old,
                    new,
                })));
            }
            self.records[section].end = Some(new);
            section += 1;
            if section == doc.sections.len() {
                break;
            }
            let props = &doc.sections[section].properties;
            flow.start_section(
                &mut self.ctx,
                section,
                Geometry::of(&props.page),
                props.start,
            )?;
            self.records[section].start = self.ctx.take_trace();
            index = 0;
        }
        flow.finish(&mut self.ctx)?;
        Ok(Ending::Finished(self.ctx.take_trace()))
    }
}
