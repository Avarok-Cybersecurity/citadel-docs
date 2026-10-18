//! One relayout, start to end: records brought up to date, placement from the resume point,
//! the pages assembled and furnished, faces put in a fresh layout's order, the result kept.

use super::edit::{apply, Start};
use super::record::{Boundary, SectionRecord};
use super::run::{Catch, Ending, Run};
use super::splice::{assemble, Assembly, Old, Slot};
use super::{Dirty, LayoutSession};
use crate::context::Ctx;
use crate::flow::{Flow, Resume, Sheet};
use crate::fragment::Fragment;
use crate::geometry::Geometry;
use crate::ir::{Page, Pt100};
use crate::labels::Counters;
use crate::LayoutError;
use cdoc_model::{Document, FootnoteId};
use std::collections::{BTreeMap, VecDeque};

/// Footnotes laid out, by width.
pub type NoteCache = BTreeMap<(FootnoteId, Pt100), Fragment>;

fn split(base: Option<super::record::Baseline>) -> (Vec<SectionRecord>, Old, NoteCache) {
    let Some(base) = base else {
        let old = Old {
            pages: Vec::new(),
            metas: Vec::new(),
            outline: Vec::new(),
            signatures: Vec::new(),
            warnings: Vec::new(),
            note_order: Vec::new(),
            finish: crate::context::Trace::empty(),
        };
        return (Vec::new(), old, NoteCache::new());
    };
    let old = Old {
        pages: base.layout.pages,
        metas: base.pages,
        outline: base.layout.outline,
        signatures: base.layout.signatures,
        warnings: base.flow_warnings,
        note_order: base.note_order,
        finish: base.finish,
    };
    (base.sections, old, base.note_cache)
}

/// The flow standing at `b`, in section `section`, with the previous layout's prefix. The
/// sheet's size is the one the cursor recorded, not the finished page's: a sheet still empty
/// when its section ended is taken over, and resized, by the next section, so the page at
/// that index may have been given another section's size after the boundary was recorded.
fn restore(doc: &Document, section: usize, b: &Boundary, old: &Old, notes: &NoteCache) -> Flow {
    let c = &b.cursor;
    let page = &old.pages[c.page];
    let (width, height) = c.state.sheet_size;
    let sheet = Sheet {
        page: Page {
            width,
            height,
            draws: page.draws[..c.draws].to_vec(),
            links: page.links[..c.links].to_vec(),
        },
        section: c.state.sheet_section,
        first_of_section: c.state.first_of_section,
        fields: old.metas[c.page].fields[..c.fields].to_vec(),
    };
    let from = Resume {
        sheet,
        outline: old.outline[..c.outline].to_vec(),
        signatures: old.signatures[..c.signatures].to_vec(),
        warnings: old.warnings[..c.warnings].to_vec(),
        note_cache: c
            .state
            .laid
            .iter()
            .filter_map(|k| notes.get(k).map(|f| (*k, f.clone())))
            .collect(),
    };
    Flow::resume(
        Geometry::of(&doc.sections[section].properties.page),
        c,
        from,
    )
}

fn unchanged(old: Old) -> Assembly {
    let slots = old
        .pages
        .into_iter()
        .zip(old.metas)
        .map(|(p, m)| Slot::Stored(p, m))
        .collect();
    Assembly {
        slots,
        outline: old.outline,
        signatures: old.signatures,
        warnings: old.warnings,
        note_order: old.note_order,
        finish: old.finish,
        moved_from: None,
    }
}

/// Places what `start` says must be placed and assembles the pages, updating `notes`.
fn place(
    run: &mut Run<'_, '_, '_>,
    start: Start,
    old: Old,
    notes: &mut NoteCache,
) -> Result<Assembly, LayoutError> {
    let doc = run.ctx.doc;
    match start {
        Start::Nowhere => Ok(unchanged(old)),
        Start::Top => {
            let mut flow = Flow::new(Geometry::of(&doc.sections[0].properties.page));
            let ending = run.place(&mut flow, (0, 0), None, true)?;
            *notes = flow.take_note_cache();
            let order = run.ctx.order.clone();
            Ok(assemble(old, 0, flow, ending, &order, run.records))
        }
        Start::At(at) => {
            let (section, index, boundary) = (at.section, at.index, &at.boundary);
            let mut flow = restore(doc, section, boundary, &old, notes);
            run.ctx.renumber(&old.note_order[..boundary.numbered]);
            run.counters.restore(boundary.counts.clone());
            let catch = Catch {
                from: at.clean_from,
                old_order: &old.note_order,
            };
            let ending = run.place(&mut flow, (section, index), Some(&catch), false)?;
            let fresh = flow.take_note_cache();
            if matches!(ending, Ending::Finished(_)) {
                *notes = fresh;
            } else {
                notes.extend(fresh);
            }
            let order = run.ctx.order.clone();
            let first = boundary.cursor.page;
            Ok(assemble(old, first, flow, ending, &order, run.records))
        }
    }
}

impl LayoutSession<'_> {
    pub(super) fn attempt(
        &mut self,
        doc: &Document,
        dirty: &Dirty,
        incremental: bool,
    ) -> Result<Vec<usize>, LayoutError> {
        if dirty.invalidates_measures() {
            self.epoch += 1;
        }
        let (records, old, mut notes) = split(self.base.take());
        let (mut records, start) = apply(records, doc, dirty, incremental);
        let fonts = self.fonts.take().expect("held between relayouts");
        let hyphenators = self.hyphenators.take().expect("held between relayouts");
        let typeset = super::hash::fingerprint(&(
            &doc.meta.language,
            doc.settings.auto_hyphenation,
            self.options.breaker,
        ));
        let mut run = Run {
            ctx: Ctx::new(doc, fonts, hyphenators, self.options),
            typeset,
            counters: Counters::new(&doc.numbering),
            records: &mut records,
            cache: &mut self.cache,
            epoch: self.epoch,
            width: 0,
            ahead: VecDeque::new(),
            next: 0,
        };
        let assembly = place(&mut run, start, old, &mut notes)?;
        let ctx = run.ctx;
        self.keep(ctx, dirty, records, assembly, notes)
    }
}
