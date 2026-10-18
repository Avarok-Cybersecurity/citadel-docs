//! The new pages put together: the previous layout's pages before the resumed one, the pages
//! the placement set, and, when it caught up, the previous layout's tail moved to where it now
//! starts (page indices shifted, the caught-up page's remaining body appended).

use super::record::{Boundary, PageMeta, SectionRecord};
use super::run::{Caught, Ending};
use crate::context::Trace;
use crate::flow::{Cursor, Flow, Sheet};
use crate::ir::{OutlineEntry, Page, SignaturePlacement, Warning};
use cdoc_model::FootnoteId;

/// What the previous layout leaves to reuse.
pub struct Old {
    /// Its pages, furnished.
    pub pages: Vec<Page>,
    /// What each page was.
    pub metas: Vec<PageMeta>,
    /// Its outline.
    pub outline: Vec<OutlineEntry>,
    /// Its signature fields.
    pub signatures: Vec<SignaturePlacement>,
    /// Its flow warnings.
    pub warnings: Vec<Warning>,
    /// Its body footnote numbering.
    pub note_order: Vec<FootnoteId>,
    /// What finishing its last sheet did.
    pub finish: Trace,
}

/// A page of the new layout before furnishing.
pub enum Slot {
    /// A previous page, unchanged in body.
    Stored(Page, PageMeta),
    /// A page the placement set; not yet furnished.
    Body(Sheet),
}

/// The new layout before furnishing.
pub struct Assembly {
    /// Its pages.
    pub slots: Vec<Slot>,
    /// Its outline.
    pub outline: Vec<OutlineEntry>,
    /// Its signature fields.
    pub signatures: Vec<SignaturePlacement>,
    /// Its flow warnings.
    pub warnings: Vec<Warning>,
    /// Its body footnote numbering.
    pub note_order: Vec<FootnoteId>,
    /// What finishing its last sheet did.
    pub finish: Trace,
    /// The first page whose index now holds something it did not hold before, beyond the
    /// pages set: every page from here on moved.
    pub moved_from: Option<usize>,
}

fn moved(value: usize, old: usize, new: usize) -> usize {
    value - old + new
}

fn moved_u32(value: u32, old: usize, new: usize) -> u32 {
    let to = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
    value - to(old) + to(new)
}

/// Moves a recorded boundary of the previous layout's tail to where the tail now is.
fn shift(boundary: &mut Boundary, old: &Cursor, new: &Cursor) {
    let c = &mut boundary.cursor;
    if c.page == old.page {
        c.draws = moved(c.draws, old.draws, new.draws);
        c.links = moved(c.links, old.links, new.links);
        c.fields = moved(c.fields, old.fields, new.fields);
    }
    c.page = moved(c.page, old.page, new.page);
    c.outline = moved(c.outline, old.outline, new.outline);
    c.signatures = moved(c.signatures, old.signatures, new.signatures);
    c.warnings = moved(c.warnings, old.warnings, new.warnings);
}

/// Shifts every boundary at or after `(section, index)`.
fn shift_tail(records: &mut [SectionRecord], at: (usize, usize), old: &Cursor, new: &Cursor) {
    for (s, record) in records.iter_mut().enumerate().skip(at.0) {
        let from = if s == at.0 { at.1 } else { 0 };
        for block in record.blocks.iter_mut().skip(from) {
            if let Some(b) = &mut block.at {
                shift(b, old, new);
            }
        }
        if let Some(b) = &mut record.end {
            shift(b, old, new);
        }
    }
}

/// Puts the new layout together from the placement that resumed at page `first`.
pub fn assemble(
    mut old: Old,
    first: usize,
    mut flow: Flow,
    ending: Ending,
    order: &[FootnoteId],
    records: &mut [SectionRecord],
) -> Assembly {
    let mut prefix: Vec<Slot> = Vec::new();
    let mut tail: Vec<Slot> = Vec::new();
    let (finish, note_order, moved_from) = match ending {
        Ending::Finished(finish) => (finish, order.to_vec(), None),
        Ending::Caught(caught) => {
            let Caught {
                section,
                index,
                old: o,
                new: n,
            } = *caught;
            let (numbered_old, numbered_new) = (o.numbered, n.numbered);
            let (o, n) = (o.cursor, n.cursor);
            shift_tail(records, (section, index), &o, &n);
            let sheet = flow.sheets.last_mut().expect("never empty");
            let page = &old.pages[o.page];
            let meta = &old.metas[o.page];
            sheet
                .page
                .draws
                .extend_from_slice(&page.draws[o.draws..meta.body_draws]);
            sheet
                .page
                .links
                .extend_from_slice(&page.links[o.links..meta.body_links]);
            sheet.fields.extend_from_slice(&meta.fields[o.fields..]);
            // The rest of the sheet is the old layout's, and so is what became of it after the
            // cursor: a sheet left empty when its section ended is taken over, resized and
            // furnished by the next section.
            (sheet.page.width, sheet.page.height) = (page.width, page.height);
            (sheet.section, sheet.first_of_section) = (meta.section, meta.first_of_section);
            let page_shift = |e: OutlineEntry| OutlineEntry {
                page: moved_u32(e.page, o.page, n.page),
                ..e
            };
            flow.outline
                .extend(old.outline.drain(o.outline..).map(page_shift));
            flow.signatures
                .extend(
                    old.signatures
                        .drain(o.signatures..)
                        .map(|s| SignaturePlacement {
                            page: moved_u32(s.page, o.page, n.page),
                            ..s
                        }),
                );
            flow.warnings
                .extend(old.warnings.drain(o.warnings..).map(|w| match w {
                    Warning::Oversized { page } => Warning::Oversized {
                        page: moved_u32(page, o.page, n.page),
                    },
                    other => other,
                }));
            let mut note_order = order[..numbered_new].to_vec();
            note_order.extend_from_slice(&old.note_order[numbered_old..]);
            let pages = old.pages.drain(o.page + 1..);
            let metas = old.metas.drain(o.page + 1..);
            tail.extend(pages.zip(metas).map(|(p, m)| Slot::Stored(p, m)));
            let moved_from = (o.page != n.page).then_some(n.page + 1);
            (old.finish, note_order, moved_from)
        }
    };
    prefix.extend(
        old.pages
            .drain(..first)
            .zip(old.metas.drain(..first))
            .map(|(p, m)| Slot::Stored(p, m)),
    );
    prefix.extend(flow.sheets.drain(..).map(Slot::Body));
    prefix.append(&mut tail);
    Assembly {
        slots: prefix,
        outline: flow.outline,
        signatures: flow.signatures,
        warnings: flow.warnings,
        note_order,
        finish,
        moved_from,
    }
}
