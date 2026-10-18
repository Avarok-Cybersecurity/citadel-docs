//! GSUB and GPOS lookups, parsed once per face: each lookup's flags, its subtables, and the set
//! of glyphs any of them can start on, so a lookup that cannot apply at a glyph is rejected
//! with one bit test. Class-based pair kerning gets flat class arrays, so the hot kerning path
//! is two array reads instead of two binary searches.

use ttf_parser::gpos::{PairAdjustment, PositioningSubtable};
use ttf_parser::gsub::SubstitutionSubtable;
use ttf_parser::opentype_layout::{Coverage, LayoutTable, LookupSubtable};

/// A set of glyph ids, one bit each.
pub(crate) struct GlyphSet(Vec<u64>);

impl GlyphSet {
    fn new(glyphs: u16) -> Self {
        Self(vec![0; usize::from(glyphs) / 64 + 1])
    }

    fn insert(&mut self, glyph: u16) {
        if let Some(word) = self.0.get_mut(usize::from(glyph) / 64) {
            *word |= 1 << (glyph % 64);
        }
    }

    fn add_coverage(&mut self, coverage: Coverage<'_>) {
        match coverage {
            Coverage::Format1 { glyphs } => glyphs.into_iter().for_each(|g| self.insert(g.0)),
            Coverage::Format2 { records } => {
                for r in records {
                    (r.start.0..=r.end.0).for_each(|g| self.insert(g));
                }
            }
        }
    }

    pub(crate) fn contains(&self, glyph: u16) -> bool {
        self.0
            .get(usize::from(glyph) / 64)
            .is_some_and(|w| w & (1 << (glyph % 64)) != 0)
    }
}

/// A lookup ready to apply.
pub(crate) struct Lookup<T> {
    /// Lookup flags in the low half, the mark filtering set in the high half.
    pub(crate) props: u32,
    /// Every glyph a subtable's coverage starts on.
    pub(crate) glyphs: GlyphSet,
    pub(crate) subtables: Vec<T>,
    /// Per subtable, the glyphs its coverage starts on.
    pub(crate) firsts: Vec<GlyphSet>,
    /// A reverse chaining substitution, applied from the end of the run.
    pub(crate) reverse: bool,
}

impl<T> Lookup<T> {
    /// Tries the subtables whose coverage holds `glyph`, in order, until one applies.
    pub(crate) fn applies(&self, glyph: u16, mut apply: impl FnMut(&T) -> bool) -> bool {
        self.subtables
            .iter()
            .zip(&self.firsts)
            .any(|(s, first)| first.contains(glyph) && apply(s))
    }
}

/// Class-based pair adjustment with its class definitions flattened.
pub(crate) struct ClassPairs<'a> {
    pub(crate) table: PairAdjustment<'a>,
    pub(crate) first: GlyphSet,
    pub(crate) class1: Vec<u16>,
    pub(crate) class2: Vec<u16>,
}

/// A positioning subtable; class-based pairs carry their flattened classes.
pub(crate) enum PosSubtable<'a> {
    ClassPairs(Box<ClassPairs<'a>>),
    Other(PositioningSubtable<'a>),
}

/// One layout table's lookups, indexed as the font indexes them.
pub(crate) struct Layout<'a, T> {
    pub(crate) table: LayoutTable<'a>,
    pub(crate) lookups: Vec<Lookup<T>>,
}

pub(crate) trait Covered<'a> {
    fn first_coverage(&self) -> Coverage<'a>;
    fn is_reverse(&self) -> bool;
}

impl<'a> Covered<'a> for SubstitutionSubtable<'a> {
    fn first_coverage(&self) -> Coverage<'a> {
        self.coverage()
    }
    fn is_reverse(&self) -> bool {
        matches!(self, Self::ReverseChainSingle(_))
    }
}

impl<'a> Covered<'a> for PositioningSubtable<'a> {
    fn first_coverage(&self) -> Coverage<'a> {
        self.coverage()
    }
    fn is_reverse(&self) -> bool {
        false
    }
}

fn lookups<'a, T: LookupSubtable<'a> + Covered<'a>>(
    table: LayoutTable<'a>,
    glyphs: u16,
) -> Vec<Lookup<T>> {
    table
        .lookups
        .into_iter()
        .map(|lookup| {
            let mut set = GlyphSet::new(glyphs);
            let subtables: Vec<T> = (0..lookup.subtables.len())
                .filter_map(|i| lookup.subtables.get::<T>(i))
                .collect();
            let firsts = subtables
                .iter()
                .map(|s| {
                    let mut first = GlyphSet::new(glyphs);
                    first.add_coverage(s.first_coverage());
                    set.add_coverage(s.first_coverage());
                    first
                })
                .collect();
            Lookup {
                props: u32::from(lookup.flags.0)
                    | (u32::from(lookup.mark_filtering_set.unwrap_or(0)) << 16),
                glyphs: set,
                reverse: subtables.first().is_some_and(Covered::is_reverse),
                subtables,
                firsts,
            }
        })
        .collect()
}

pub(crate) fn gsub(table: LayoutTable<'_>, glyphs: u16) -> Layout<'_, SubstitutionSubtable<'_>> {
    Layout {
        table,
        lookups: lookups(table, glyphs),
    }
}

/// Every glyph's class, walked from the definition's own records (one pass over its ranges
/// rather than a search per glyph); a glyph the definition does not name is class 0.
fn classes_of(c: ttf_parser::opentype_layout::ClassDefinition<'_>, glyphs: u16) -> Vec<u16> {
    use ttf_parser::opentype_layout::ClassDefinition;
    let mut out = vec![0; usize::from(glyphs)];
    match c {
        ClassDefinition::Format1 { start, classes } => {
            let from = usize::from(start.0);
            for (slot, class) in out.iter_mut().skip(from).zip(classes) {
                *slot = class;
            }
        }
        ClassDefinition::Format2 { records } => {
            for r in records {
                let end = (usize::from(r.end.0) + 1).min(out.len());
                if let Some(range) = out.get_mut(usize::from(r.start.0)..end) {
                    range.fill(r.value);
                }
            }
        }
        ClassDefinition::Empty => {}
    }
    out
}

fn flatten(sub: PositioningSubtable<'_>, glyphs: u16) -> PosSubtable<'_> {
    match sub {
        PositioningSubtable::Pair(
            table @ PairAdjustment::Format2 {
                coverage, classes, ..
            },
        ) => {
            let mut first = GlyphSet::new(glyphs);
            first.add_coverage(coverage);
            let class = |c| classes_of(c, glyphs);
            PosSubtable::ClassPairs(Box::new(ClassPairs {
                table,
                first,
                class1: class(classes.0),
                class2: class(classes.1),
            }))
        }
        other => PosSubtable::Other(other),
    }
}

pub(crate) fn gpos<'a>(table: LayoutTable<'a>, glyphs: u16) -> Layout<'a, PosSubtable<'a>> {
    let lookups = lookups::<PositioningSubtable<'a>>(table, glyphs)
        .into_iter()
        .map(|l| Lookup {
            props: l.props,
            glyphs: l.glyphs,
            reverse: false,
            firsts: l.firsts,
            subtables: l
                .subtables
                .into_iter()
                .map(|s| flatten(s, glyphs))
                .collect(),
        })
        .collect();
    Layout { table, lookups }
}
