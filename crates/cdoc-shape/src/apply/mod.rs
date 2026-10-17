//! Running lookups over a run: the apply context, the per-glyph loop, and recursion into the
//! nested lookups contextual rules call.

pub(crate) mod context;
pub(crate) mod matching;
mod nested;
pub(crate) mod skip;
mod string;

use crate::buffer::{Buffer, GlyphInfo};
use crate::face::gdef::MARK;
use crate::face::Face;

/// How deep contextual lookups may nest.
const MAX_NESTING: u8 = 64;

const IGNORE_FLAGS: u32 = 0x000E;
const USE_MARK_FILTERING_SET: u32 = 0x0010;
const MARK_ATTACHMENT_TYPE: u32 = 0xFF00;
/// Lookup flag: skip marks.
pub(crate) const IGNORE_MARKS: u32 = 0x0008;

/// Which table a lookup comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Table {
    Gsub,
    Gpos,
}

pub(crate) struct Ctx<'c, 'a> {
    pub(crate) face: &'c Face<'a>,
    pub(crate) buf: &'c mut Buffer,
    pub(crate) table: Table,
    pub(crate) lookup_props: u32,
    pub(crate) auto_zwj: bool,
    pub(crate) auto_zwnj: bool,
    pub(super) nesting_left: u8,
    /// Mark attachment's cache of the last base found, valid below `last_base_until`.
    pub(crate) last_base: Option<usize>,
    pub(crate) last_base_until: usize,
}

impl<'c, 'a> Ctx<'c, 'a> {
    pub(crate) fn new(face: &'c Face<'a>, buf: &'c mut Buffer, table: Table) -> Self {
        Self {
            face,
            buf,
            table,
            lookup_props: 0,
            auto_zwj: true,
            auto_zwnj: true,
            nesting_left: MAX_NESTING,
            last_base: None,
            last_base_until: 0,
        }
    }

    /// Whether a lookup with `props` may see `info` at all.
    pub(crate) fn check_glyph_property(&self, info: &GlyphInfo, props: u32) -> bool {
        let glyph_props = u32::from(info.gprops);
        if glyph_props & props & IGNORE_FLAGS != 0 {
            return false;
        }
        if glyph_props & u32::from(MARK) != 0 {
            if props & USE_MARK_FILTERING_SET != 0 {
                let set = u16::try_from(props >> 16).unwrap_or(u16::MAX);
                return self.face.gdef.is_mark_in_set(info.glyph, set);
            }
            if props & MARK_ATTACHMENT_TYPE != 0 {
                return props & MARK_ATTACHMENT_TYPE == glyph_props & MARK_ATTACHMENT_TYPE;
            }
        }
        true
    }

    /// Whether no contextual rule is running (reverse substitutions apply only then).
    pub(crate) fn is_outermost(&self) -> bool {
        self.nesting_left == MAX_NESTING
    }

    /// Applies lookup `index` once at the current glyph, for a contextual rule.
    pub(crate) fn recurse(&mut self, index: u16) -> bool {
        if self.nesting_left == 0 {
            return false;
        }
        self.buf.max_ops -= 1;
        if self.buf.max_ops < 0 {
            self.buf.failed = true;
            return false;
        }
        self.nesting_left -= 1;
        let saved = self.lookup_props;
        let applied = self.apply_once(index);
        self.lookup_props = saved;
        self.nesting_left += 1;
        applied
    }

    /// Applies lookup `index` at the current glyph: the first subtable that applies wins.
    fn apply_once(&mut self, index: u16) -> bool {
        let face = self.face;
        let at = usize::from(index);
        match self.table {
            Table::Gsub => {
                let Some(lookup) = face.gsub.as_ref().and_then(|t| t.lookups.get(at)) else {
                    return false;
                };
                self.lookup_props = lookup.props;
                lookup.applies(self.buf.cur(0).glyph, |s| crate::substitute::apply(self, s))
            }
            Table::Gpos => {
                let Some(lookup) = face.gpos.as_ref().and_then(|t| t.lookups.get(at)) else {
                    return false;
                };
                self.lookup_props = lookup.props;
                lookup.applies(self.buf.cur(0).glyph, |s| crate::position::apply(self, s))
            }
        }
    }
}
