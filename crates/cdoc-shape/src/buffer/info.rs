//! One glyph in flight: the character it came from, its cluster, its glyph, and the class and
//! ligature bookkeeping lookups read and write.

use crate::face::gdef::{BASE, LIGATED, LIGATURE, MARK, MULTIPLIED, SUBSTITUTED};
use crate::unicode::{Category, Props};

/// A glyph and everything shaping knows about it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GlyphInfo {
    /// The character, or the last character it was composed or decomposed to.
    pub(crate) cp: u32,
    /// Byte offset of the cluster's first character in the input.
    pub(crate) cluster: u32,
    pub(crate) glyph: u16,
    pub(crate) props: Props,
    /// GDEF class bits, shaper flags and (high byte) mark attachment class.
    pub(crate) gprops: u16,
    /// Ligature id (high 3 bits), "is ligature base" (bit 4), component or count (low 4).
    pub(crate) lig: u8,
}

const IS_LIG_BASE: u8 = 0x10;

impl GlyphInfo {
    pub(crate) const EMPTY: Self = Self {
        cp: 0,
        cluster: 0,
        glyph: 0,
        props: Props(0),
        gprops: 0,
        lig: 0,
    };

    pub(crate) fn new(cp: u32, cluster: u32) -> Self {
        Self {
            cp,
            cluster,
            props: Props::of(cp),
            ..Self::EMPTY
        }
    }

    pub(crate) fn is_mark(&self) -> bool {
        self.gprops & MARK != 0
    }
    pub(crate) fn is_base(&self) -> bool {
        self.gprops & BASE != 0
    }
    pub(crate) fn is_ligature(&self) -> bool {
        self.gprops & LIGATURE != 0
    }
    pub(crate) fn substituted(&self) -> bool {
        self.gprops & SUBSTITUTED != 0
    }
    pub(crate) fn ligated(&self) -> bool {
        self.gprops & LIGATED != 0
    }
    pub(crate) fn multiplied(&self) -> bool {
        self.gprops & MULTIPLIED != 0
    }

    /// Default ignorable and not yet replaced by a lookup.
    pub(crate) fn is_default_ignorable(&self) -> bool {
        self.props.is_ignorable() && !self.substituted()
    }
    pub(crate) fn is_zwj(&self) -> bool {
        self.cp == 0x200D && self.props.category() == Category::Format
    }
    pub(crate) fn is_zwnj(&self) -> bool {
        self.cp == 0x200C && self.props.category() == Category::Format
    }

    pub(crate) fn lig_id(&self) -> u8 {
        self.lig >> 5
    }
    fn is_lig_base(&self) -> bool {
        self.lig & IS_LIG_BASE != 0
    }
    /// Which component of a ligature a mark or component belongs to (0: none).
    pub(crate) fn lig_comp(&self) -> u8 {
        if self.is_lig_base() {
            0
        } else {
            self.lig & 0x0F
        }
    }
    /// How many components a ligature glyph stands for (1 for anything else).
    pub(crate) fn lig_num_comps(&self) -> u8 {
        if self.is_ligature() && self.is_lig_base() {
            self.lig & 0x0F
        } else {
            1
        }
    }
    pub(crate) fn set_lig_for_ligature(&mut self, id: u8, comps: u8) {
        self.lig = (id << 5) | IS_LIG_BASE | (comps & 0x0F);
    }
    pub(crate) fn set_lig_for_mark(&mut self, id: u8, comp: u8) {
        self.lig = (id << 5) | (comp & 0x0F);
    }
}

/// A glyph's placement, in font units.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GlyphPos {
    pub(crate) x_advance: i32,
    pub(crate) y_advance: i32,
    pub(crate) x_offset: i32,
    pub(crate) y_offset: i32,
    /// Offset to the glyph this one is attached to (0: none).
    pub(crate) attach_chain: i16,
    pub(crate) attach_type: u8,
}

impl GlyphPos {
    pub(crate) const ZERO: Self = Self {
        x_advance: 0,
        y_advance: 0,
        x_offset: 0,
        y_offset: 0,
        attach_chain: 0,
        attach_type: 0,
    };
}

/// A mark attached to a base, ligature or mark.
pub(crate) const ATTACH_MARK: u8 = 1;
/// A cross-stream kerning attachment (legacy `kern`).
pub(crate) const ATTACH_CURSIVE: u8 = 2;
