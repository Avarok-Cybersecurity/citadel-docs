//! The end of a relayout: pages furnished, faces put in a fresh layout's order, warnings
//! gathered, and everything kept for the next relayout.

use super::attempt::NoteCache;
use super::canon::{canon, is_identity, replay, Remap};
use super::furnish::{furnish_all, furniture_numbers_notes};
use super::record::{Baseline, SectionRecord};
use super::splice::Assembly;
use super::{Dirty, LayoutSession};
use crate::context::Ctx;
use crate::ir::{Layout, Warning};
use crate::LayoutError;
use std::sync::Arc;

impl<'s> LayoutSession<'s> {
    /// Furnishes the assembled pages and keeps the result; returns the pages that changed.
    pub(super) fn keep(
        &mut self,
        mut ctx: Ctx<'_, 's>,
        dirty: &Dirty,
        mut records: Vec<SectionRecord>,
        assembly: Assembly,
        mut notes: NoteCache,
    ) -> Result<Vec<usize>, LayoutError> {
        let doc = ctx.doc;
        ctx.renumber(&assembly.note_order);
        let force = dirty.furniture_changed || dirty.replaces_all() || furniture_numbers_notes(doc);
        let (mut pages, mut metas, mut changed) = furnish_all(&mut ctx, assembly.slots, force)?;
        changed.extend(&assembly.renumbered);
        if let Some(from) = assembly.moved_from {
            changed.extend(from..pages.len());
        }
        let mut finish = assembly.finish;
        let found = canon(&ctx.fonts, replay(&records, &finish, &metas));
        let mut missing = found.missing;
        if !is_identity(&found.order, ctx.fonts.keys().len()) {
            let map = ctx.fonts.rebase(&found.order);
            let stored = (&mut pages[..], &mut metas[..], &mut finish);
            Remap(&map).all(&mut self.cache, &mut records, stored, &mut notes);
            missing = missing
                .into_iter()
                .map(|(f, n)| (map[usize::from(f.0)].expect("a live face"), n))
                .collect();
            changed = (0..pages.len()).collect();
        }
        changed.sort_unstable();
        changed.dedup();
        let faces = ctx.fonts.keys().to_vec();
        let substituted = found
            .substitutions
            .into_iter()
            .map(|(requested, served)| Warning::Substituted { requested, served });
        let unmapped = missing
            .into_iter()
            .map(|(face, count)| Warning::MissingGlyphs {
                face: faces[usize::from(face.0)].clone(),
                count,
            });
        let unhyphenated = found
            .unhyphenated
            .into_iter()
            .map(|language| Warning::NoHyphenationPatterns { language });
        let warnings = assembly
            .warnings
            .iter()
            .cloned()
            .chain(substituted)
            .chain(unmapped)
            .chain(unhyphenated)
            .collect();
        let blocks: usize = records.iter().map(|s| s.blocks.len()).sum();
        if self.cache.len() > 2 * blocks + 64 {
            self.cache.retain(|_, m| Arc::strong_count(m) > 1);
        }
        self.fonts = Some(ctx.fonts);
        self.hyphenators = Some(ctx.hyphenators);
        self.base = Some(Baseline {
            layout: Layout {
                pages,
                faces,
                outline: assembly.outline,
                signatures: assembly.signatures,
                title: doc.meta.title.clone(),
                language: doc
                    .meta
                    .language
                    .clone()
                    .or_else(|| doc.styles.defaults.run.lang.clone()),
                warnings,
            },
            sections: records,
            pages: metas,
            flow_warnings: assembly.warnings,
            note_order: assembly.note_order,
            note_cache: notes,
            finish,
        });
        Ok(changed)
    }
}
