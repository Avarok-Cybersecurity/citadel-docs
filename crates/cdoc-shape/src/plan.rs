//! A shaping plan: for one face and one parameter set, which lookups run in which order, and
//! which positioning fallbacks apply. Built once, then cached by the shaper.

use crate::face::Face;
use crate::params::{Feature, ShapeParams};
use crate::position::kern;
use ttf_parser::opentype_layout::{FeatureIndex, LanguageIndex, LayoutTable, ScriptIndex};
use ttf_parser::Tag;

/// A lookup in a plan, with the joiner handling its features asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PlannedLookup {
    pub(crate) index: u16,
    pub(crate) auto_zwj: bool,
    pub(crate) auto_zwnj: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each switch is a separate pipeline decision taken once per plan"
)]
pub(crate) struct Plan {
    /// Stage 0 holds a required feature no enabled feature names; stage 1 everything else.
    pub(crate) gsub: [Vec<PlannedLookup>; 2],
    pub(crate) gpos: Vec<PlannedLookup>,
    pub(crate) apply_gpos: bool,
    /// Legacy `kern` positioning, when GPOS has no kerning for this script.
    pub(crate) apply_kern: bool,
    pub(crate) kern_requested: bool,
    pub(crate) zero_marks: bool,
    pub(crate) adjust_offsets_when_zeroing: bool,
    pub(crate) synthesize_classes: bool,
}

struct Selected<'a> {
    table: LayoutTable<'a>,
    script: ScriptIndex,
    language: Option<LanguageIndex>,
}

fn select(table: LayoutTable<'_>, params: ShapeParams) -> Option<Selected<'_>> {
    let wanted = params.script.tag();
    let fallbacks = [
        Tag::from_bytes(b"DFLT"),
        Tag::from_bytes(b"dflt"),
        Tag::from_bytes(b"latn"),
    ];
    let script = wanted
        .into_iter()
        .chain(fallbacks)
        .find_map(|t| table.scripts.index(t))?;
    let languages = table.scripts.get(script)?.languages;
    let language = params
        .language
        .tag()
        .and_then(|t| languages.index(t))
        .or_else(|| languages.index(Tag::from_bytes(b"dflt")));
    Some(Selected {
        table,
        script,
        language,
    })
}

impl Selected<'_> {
    fn system(&self) -> Option<ttf_parser::opentype_layout::LanguageSystem<'_>> {
        let script = self.table.scripts.get(self.script)?;
        match self.language {
            Some(i) => script.languages.get(i),
            None => script.default_language,
        }
    }

    fn feature(&self, tag: Tag) -> Option<FeatureIndex> {
        let system = self.system()?;
        system
            .feature_indices
            .into_iter()
            .find(|&i| self.table.features.get(i).is_some_and(|f| f.tag == tag))
    }

    fn required(&self) -> Option<(FeatureIndex, Tag)> {
        let index = self.system()?.required_feature?;
        Some((index, self.table.features.get(index)?.tag))
    }

    fn add(&self, into: &mut Vec<PlannedLookup>, feature: FeatureIndex, auto: bool) {
        let count = self.table.lookups.len();
        if let Some(f) = self.table.features.get(feature) {
            into.extend(
                f.lookup_indices
                    .into_iter()
                    .filter(|&i| i < count)
                    .map(|index| PlannedLookup {
                        index,
                        auto_zwj: auto,
                        auto_zwnj: auto,
                    }),
            );
        }
    }
}

/// Lookup order within a stage is the font's lookup order; a lookup two features share runs
/// once, with joiners skipped only if both features skip them.
fn sort_dedup(lookups: &mut Vec<PlannedLookup>) {
    lookups.sort_by_key(|l| l.index);
    lookups.dedup_by(|later, kept| {
        if later.index == kept.index {
            kept.auto_zwj &= later.auto_zwj;
            kept.auto_zwnj &= later.auto_zwnj;
            true
        } else {
            false
        }
    });
}

/// The lookups of one table, as `[stage 0, stage 1]`, and whether each feature was found.
fn collect(
    sel: Option<&Selected<'_>>,
    params: ShapeParams,
) -> ([Vec<PlannedLookup>; 2], Vec<Feature>) {
    let mut stages = [Vec::new(), Vec::new()];
    let mut found = Vec::new();
    let Some(sel) = sel else {
        return (stages, found);
    };
    let enabled = |f: &Feature| params.features.contains(*f);
    let required = sel.required();
    if let Some((index, tag)) = required {
        let stage = usize::from(
            Feature::ALL
                .iter()
                .filter(|f| enabled(f))
                .any(|f| f.tag() == tag),
        );
        sel.add(&mut stages[stage], index, true);
    }
    for feature in Feature::ALL.iter().filter(|f| enabled(f)) {
        if let Some(index) = sel.feature(feature.tag()) {
            found.push(*feature);
            sel.add(&mut stages[1], index, !feature.manual_joiners());
        }
    }
    stages.iter_mut().for_each(sort_dedup);
    (stages, found)
}

impl Plan {
    pub(crate) fn new(face: &Face<'_>, params: ShapeParams) -> Self {
        let gsub_sel = face.gsub.as_ref().and_then(|l| select(l.table, params));
        let gpos_sel = face.gpos.as_ref().and_then(|l| select(l.table, params));
        let (gsub, _) = collect(gsub_sel.as_ref(), params);
        let (gpos_stages, gpos_found) = collect(gpos_sel.as_ref(), params);
        let [mut gpos, rest] = gpos_stages;
        gpos.extend(rest);
        sort_dedup(&mut gpos);
        let apply_gpos = face.gpos.is_some();
        let has_gpos_kern = gpos_found.contains(&Feature::Kern);
        let apply_kern = (!has_gpos_kern || !apply_gpos) && kern::has_table(face);
        Self {
            gsub,
            gpos,
            apply_gpos,
            apply_kern,
            kern_requested: params.features.contains(Feature::Kern),
            zero_marks: !apply_kern || !kern::has_state_machine(face),
            adjust_offsets_when_zeroing: !apply_gpos
                && (!apply_kern || !kern::has_cross_stream(face)),
            synthesize_classes: !face.gdef.has_classes(),
        }
    }
}
