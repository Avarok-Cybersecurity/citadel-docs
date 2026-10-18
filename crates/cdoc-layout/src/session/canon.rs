//! Faces numbered as a fresh layout numbers them. A face's id is the order a layout first meets
//! it in, and an incremental layout meets faces in another order (or keeps some nobody uses any
//! more), so the order a fresh layout would meet them in is replayed from the traces; when it
//! differs from the session's, every stored id is renumbered once.

use super::measure::Cache;
use super::record::{Measured, PageMeta, SectionRecord};
use crate::context::Trace;
use crate::fonts::Fonts;
use crate::fragment::{Fragment, Mark};
use crate::ir::{Draw, FaceId, FaceKey, GlyphRun, Page, Pt100};
use cdoc_model::FootnoteId;
use std::collections::BTreeMap;
use std::sync::Arc;

/// What the replay found.
pub struct Canon {
    /// The faces, as session ids, in the order a fresh layout meets them.
    pub order: Vec<FaceId>,
    /// Substitutions, in the order a fresh layout reports them.
    pub substitutions: Vec<(FaceKey, FaceKey)>,
    /// Characters replaced, per face (session ids).
    pub missing: BTreeMap<FaceId, usize>,
    /// Languages without patterns met in hyphenated text, first met first.
    pub unhyphenated: Vec<String>,
}

/// Every trace in the order a fresh layout makes them: per section its start, its blocks'
/// measurements, their placements; the last sheet's finish; each page's furnishing.
pub fn replay<'t>(
    sections: &'t [SectionRecord],
    finish: &'t Trace,
    pages: &'t [PageMeta],
) -> impl Iterator<Item = &'t Trace> + 't {
    sections
        .iter()
        .flat_map(|s| {
            let measured = s
                .blocks
                .iter()
                .filter_map(|b| b.measured.as_ref().map(|m| &m.trace));
            std::iter::once(&s.start)
                .chain(measured)
                .chain(s.blocks.iter().map(|b| &b.place))
        })
        .chain(std::iter::once(finish))
        .chain(pages.iter().map(|p| &p.trace))
}

/// Replays the traces against the session's faces.
pub fn canon<'t>(fonts: &Fonts<'_>, traces: impl Iterator<Item = &'t Trace>) -> Canon {
    let mut seen_request: Vec<bool> = Vec::new();
    let mut seen_face = vec![false; fonts.keys().len()];
    let mut out = Canon {
        order: Vec::new(),
        substitutions: Vec::new(),
        missing: BTreeMap::new(),
        unhyphenated: Vec::new(),
    };
    for trace in traces {
        for &request in &trace.requests {
            if seen_request.len() <= request {
                seen_request.resize(request + 1, false);
            }
            if std::mem::replace(&mut seen_request[request], true) {
                continue;
            }
            out.substitutions.extend(fonts.substitution(request));
            let face = fonts
                .served(request)
                .expect("a live trace's request is live");
            if !std::mem::replace(&mut seen_face[usize::from(face.0)], true) {
                out.order.push(face);
            }
        }
        for (face, count) in &trace.missing {
            *out.missing.entry(*face).or_insert(0) += count;
        }
        for language in &trace.unhyphenated {
            if !out.unhyphenated.contains(language) {
                out.unhyphenated.push(language.clone());
            }
        }
    }
    out
}

/// Whether the session's ids already are the fresh layout's: `order` names every one of the
/// `faces`, in id order.
#[must_use]
pub fn is_identity(order: &[FaceId], faces: usize) -> bool {
    order.len() == faces && order.iter().enumerate().all(|(i, f)| usize::from(f.0) == i)
}

/// Renumbers face ids through `map` (old id → new id).
pub struct Remap<'m>(pub &'m [Option<FaceId>]);

impl Remap<'_> {
    fn face(&self, face: &mut FaceId) {
        *face = self.0[usize::from(face.0)].expect("a live face");
    }

    fn run(&self, run: &mut GlyphRun) {
        self.face(&mut run.face);
    }

    fn draws(&self, draws: &mut [Draw]) {
        for draw in draws {
            if let Draw::Text(run) = draw {
                self.run(run);
            }
        }
    }

    fn fragment(&self, fragment: &mut Fragment) {
        self.draws(&mut fragment.draws);
        for mark in &mut fragment.marks {
            if let Mark::Field(field) = mark {
                self.run(&mut field.run);
            }
        }
    }

    fn trace(&self, trace: &mut Trace) {
        for (face, _) in &mut trace.missing {
            self.face(face);
        }
    }

    fn measured(&self, measured: &Measured) -> Arc<Measured> {
        let mut out = measured.clone();
        for strip in &mut out.block.strips {
            self.fragment(strip);
        }
        self.trace(&mut out.trace);
        Arc::new(out)
    }

    /// Renumbers everything a session stores. Unreferenced cache entries are dropped first,
    /// since they may hold faces that no longer exist.
    pub fn all(
        &self,
        cache: &mut Cache,
        sections: &mut [SectionRecord],
        stored: (&mut [Page], &mut [PageMeta], &mut Trace),
        notes: &mut BTreeMap<(FootnoteId, Pt100), Fragment>,
    ) {
        cache.retain(|_, m| Arc::strong_count(m) > 1);
        let mut renewed: BTreeMap<*const Measured, Arc<Measured>> = BTreeMap::new();
        for entry in cache.values_mut() {
            let new = self.measured(entry);
            renewed.insert(Arc::as_ptr(entry), Arc::clone(&new));
            *entry = new;
        }
        for section in sections {
            self.trace(&mut section.start);
            for block in &mut section.blocks {
                self.trace(&mut block.place);
                if let Some(m) = &mut block.measured {
                    *m = renewed
                        .get(&Arc::as_ptr(m))
                        .cloned()
                        .unwrap_or_else(|| self.measured(m));
                }
            }
        }
        let (pages, metas, finish) = stored;
        for page in pages {
            self.draws(&mut page.draws);
        }
        for meta in metas {
            meta.fields.iter_mut().for_each(|f| self.run(&mut f.run));
            self.trace(&mut meta.trace);
        }
        self.trace(finish);
        notes.retain(|_, f| self.covers(f));
        for fragment in notes.values_mut() {
            self.fragment(fragment);
        }
    }

    /// Whether every face a fragment draws in survives: a footnote laid out for a layout that
    /// no longer references it may not.
    fn covers(&self, fragment: &Fragment) -> bool {
        let live = |run: &GlyphRun| self.0[usize::from(run.face.0)].is_some();
        fragment
            .draws
            .iter()
            .all(|d| !matches!(d, Draw::Text(run) if !live(run)))
            && fragment
                .marks
                .iter()
                .all(|m| !matches!(m, Mark::Field(f) if !live(&f.run)))
    }
}
