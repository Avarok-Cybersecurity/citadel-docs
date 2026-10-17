//! The Unicode properties shaping needs, from tables generated out of the Unicode Character
//! Database (`scripts/gen-unicode-tables.py`, version in [`UNICODE_VERSION`]).
//!
//! Properties, scripts and emoji classes cover all of Unicode; canonical (de)compositions cover
//! every character a shaped run can hold (common, inherited, Latin, Greek and Cyrillic), since
//! runs of other scripts are refused before they reach normalisation.

mod norm_table;
mod pict_table;
mod props_table;
mod script_table;

/// The Unicode version the tables were generated from.
pub const UNICODE_VERSION: (u8, u8, u8) = (16, 0, 0);

/// General category, reduced to what shaping distinguishes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Category {
    /// Anything not listed below.
    Other,
    /// `Mn`.
    NonspacingMark,
    /// `Mc`.
    SpacingMark,
    /// `Me`.
    EnclosingMark,
    /// `Zs`.
    Space,
    /// `Cf`.
    Format,
}

/// A character's properties, packed: bits 0-2 [`Category`], bit 3 default ignorable, bit 4
/// hidden (ignorable but not skipped in substitution), bit 5 grapheme continuation; the high
/// byte is the combining class of a mark or the fallback width class of a space.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Props(pub(crate) u16);

const IGNORABLE: u16 = 8;
const HIDDEN: u16 = 16;
const CONTINUATION: u16 = 32;

impl Props {
    pub(crate) fn of(c: u32) -> Self {
        let raw = if c < 0x80 {
            if c == 0x20 {
                4
            } else {
                0
            }
        } else {
            lookup(props_table::PROPS, c).unwrap_or(0)
        };
        Self(if (1..=3).contains(&(raw & 7)) {
            raw | CONTINUATION
        } else {
            raw
        })
    }

    pub(crate) fn category(self) -> Category {
        match self.0 & 7 {
            1 => Category::NonspacingMark,
            2 => Category::SpacingMark,
            3 => Category::EnclosingMark,
            4 => Category::Space,
            5 => Category::Format,
            _ => Category::Other,
        }
    }

    pub(crate) fn set_category(&mut self, category: Category) {
        let bits = match category {
            Category::Other => 0,
            Category::NonspacingMark => 1,
            Category::SpacingMark => 2,
            Category::EnclosingMark => 3,
            Category::Space => 4,
            Category::Format => 5,
        };
        self.0 = (self.0 & 0x00F8) | bits;
    }

    pub(crate) fn is_mark(self) -> bool {
        (1..=3).contains(&(self.0 & 7))
    }

    /// The combining class, for a mark; 0 for anything else.
    pub(crate) fn combining_class(self) -> u8 {
        if self.is_mark() {
            self.0.to_be_bytes()[0]
        } else {
            0
        }
    }

    pub(crate) fn is_ignorable(self) -> bool {
        self.0 & IGNORABLE != 0
    }

    pub(crate) fn is_hidden(self) -> bool {
        self.0 & HIDDEN != 0
    }

    pub(crate) fn unhide(&mut self) {
        self.0 &= !HIDDEN;
    }

    pub(crate) fn set_continuation(&mut self) {
        self.0 |= CONTINUATION;
    }

    pub(crate) fn is_continuation(self) -> bool {
        self.0 & CONTINUATION != 0
    }

    /// Whether a space was drawn with the space glyph because its own was missing.
    pub(crate) fn has_space_fallback(self) -> bool {
        self.category() == Category::Space && self.0 >> 8 != 0
    }

    pub(crate) fn set_space_fallback(&mut self) {
        self.0 = (self.0 & 0x00FF) | 0x0100;
    }
}

fn lookup<T: Copy>(table: &[(u32, u32, T)], c: u32) -> Option<T> {
    let i = table.partition_point(|&(_, end, _)| end < c);
    table
        .get(i)
        .filter(|&&(start, _, _)| start <= c)
        .map(|&(_, _, v)| v)
}

/// A character's script, folded to what itemisation needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CharScript {
    /// `Common` or `Unknown`: takes the script of its neighbours.
    Common,
    /// `Inherited` (combining marks): takes the script of what it follows.
    Inherited,
    /// Latin.
    Latin,
    /// Greek.
    Greek,
    /// Cyrillic.
    Cyrillic,
    /// Any other script.
    Other,
}

pub(crate) fn script(c: char) -> CharScript {
    if c.is_ascii() {
        return if c.is_ascii_alphabetic() {
            CharScript::Latin
        } else {
            CharScript::Common
        };
    }
    match lookup(script_table::SCRIPTS, u32::from(c)) {
        Some(1) => CharScript::Inherited,
        Some(2) => CharScript::Latin,
        Some(3) => CharScript::Greek,
        Some(4) => CharScript::Cyrillic,
        Some(_) => CharScript::Other,
        None => CharScript::Common,
    }
}

/// Whether `c` is `Extended_Pictographic`: an emoji a zero-width joiner binds to.
pub(crate) fn is_extended_pictographic(c: u32) -> bool {
    let table = pict_table::EXTENDED_PICTOGRAPHIC;
    let i = table.partition_point(|&(_, end)| end < c);
    table.get(i).is_some_and(|&(start, _)| start <= c)
}

/// One step of canonical decomposition: `(first, second)`, `second` 0 for a singleton.
pub(crate) fn decompose(c: u32) -> Option<(u32, u32)> {
    let table = norm_table::DECOMPOSE;
    table
        .binary_search_by_key(&c, |&(composed, _, _)| composed)
        .ok()
        .map(|i| (table[i].1, table[i].2))
}

/// The primary composite of `a` followed by `b`, if there is one.
pub(crate) fn compose(a: u32, b: u32) -> Option<u32> {
    let table = norm_table::DECOMPOSE;
    norm_table::COMPOSE
        .binary_search_by_key(&(a, b), |&i| {
            let (_, x, y) = table[usize::from(i)];
            (x, y)
        })
        .ok()
        .map(|i| table[usize::from(norm_table::COMPOSE[i])].0)
}

/// How wide a space is drawn when the face has no glyph of its own for it, as a fraction of
/// the em or by reference to another glyph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SpaceWidth {
    /// The space glyph's own advance.
    Space,
    /// `1/n` em.
    EmDivided(u8),
    /// `4/18` em.
    FourEighteenths,
    /// The width of a digit.
    Figure,
    /// The width of a full stop or comma.
    Punctuation,
    /// Half the width of the space glyph.
    Narrow,
}

/// The fallback width of a space character, or `None` for a character that is not one.
pub(crate) fn space_width(c: u32) -> Option<SpaceWidth> {
    Some(match c {
        0x0020 | 0x00A0 => SpaceWidth::Space,
        0x2000 | 0x2002 => SpaceWidth::EmDivided(2),
        0x2001 | 0x2003 | 0x3000 => SpaceWidth::EmDivided(1),
        0x2004 => SpaceWidth::EmDivided(3),
        0x2005 => SpaceWidth::EmDivided(4),
        0x2006 => SpaceWidth::EmDivided(6),
        0x2009 => SpaceWidth::EmDivided(5),
        0x200A => SpaceWidth::EmDivided(16),
        0x2007 => SpaceWidth::Figure,
        0x2008 => SpaceWidth::Punctuation,
        0x202F => SpaceWidth::Narrow,
        0x205F => SpaceWidth::FourEighteenths,
        _ => return None,
    })
}

#[cfg(test)]
mod tests;
