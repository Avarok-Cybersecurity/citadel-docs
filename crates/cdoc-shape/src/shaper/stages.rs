//! The steps between and around the lookups: grapheme clusters, glyph classes, default
//! advances, mark widths, and hiding default ignorables.

use crate::buffer::{Buffer, GlyphPos};
use crate::face::gdef::{BASE, MARK};
use crate::face::Face;
use crate::unicode::{self, Category, SpaceWidth};

/// Grapheme continuations beyond combining marks: emoji modifiers, the second of a pair of
/// regional indicators, a zero-width joiner and the pictograph it joins, halfwidth sound
/// marks and tag characters.
pub(super) fn mark_continuations(buf: &mut Buffer) {
    let regional = |c: u32| (0x1_F1E6..=0x1_F1FF).contains(&c);
    let mut i = 0;
    while i < buf.len {
        let cp = buf.info[i].cp;
        let after_lone_regional = i != 0
            && regional(cp)
            && regional(buf.info[i - 1].cp)
            && !buf.info[i - 1].props.is_continuation();
        if (0x1_F3FB..=0x1_F3FF).contains(&cp)
            || matches!(cp, 0xFF9E..=0xFF9F | 0xE_0020..=0xE_007F)
            || after_lone_regional
        {
            buf.info[i].props.set_continuation();
        } else if buf.info[i].is_zwj() {
            buf.info[i].props.set_continuation();
            if i + 1 < buf.len && unicode::is_extended_pictographic(buf.info[i + 1].cp) {
                buf.info[i + 1].props.set_continuation();
                i += 1;
            }
        }
        i += 1;
    }
}

/// Every glyph of a grapheme (a base and the marks and joiners after it) shares the base's
/// cluster.
pub(super) fn form_clusters(buf: &mut Buffer) {
    if !buf.non_ascii {
        return;
    }
    let mut start = 0;
    while start < buf.len {
        let mut end = start + 1;
        while end < buf.len && buf.info[end].props.is_continuation() {
            end += 1;
        }
        buf.merge_clusters(start, end);
        start = end;
    }
}

/// Glyph classes from GDEF, or guessed from Unicode when the font has none.
pub(super) fn glyph_props(face: &Face<'_>, buf: &mut Buffer, synthesize: bool) {
    for info in &mut buf.info[..buf.len] {
        info.lig = 0;
        info.gprops = if synthesize {
            let mark =
                info.props.category() == Category::NonspacingMark && !info.props.is_ignorable();
            if mark {
                MARK
            } else {
                BASE
            }
        } else {
            face.gdef.props(info.glyph)
        };
    }
}

/// Advances from `hmtx`, then the widths of spaces drawn with the space glyph.
pub(super) fn default_positions(face: &Face<'_>, buf: &mut Buffer) {
    buf.pos.clear();
    buf.pos.resize(buf.len, GlyphPos::ZERO);
    for (info, pos) in buf.info[..buf.len].iter().zip(&mut buf.pos) {
        pos.x_advance = face.advance(info.glyph);
    }
    if buf.has_space_fallback {
        space_widths(face, buf);
    }
}

fn space_widths(face: &Face<'_>, buf: &mut Buffer) {
    let em = i32::from(face.units_per_em());
    let width_of = |c: char| face.glyph(c).map(|g| face.advance(g));
    for (info, pos) in buf.info[..buf.len].iter().zip(&mut buf.pos) {
        if !info.props.has_space_fallback() || info.ligated() {
            continue;
        }
        match unicode::space_width(info.cp) {
            Some(SpaceWidth::EmDivided(n)) => {
                pos.x_advance = (em + i32::from(n) / 2) / i32::from(n);
            }
            Some(SpaceWidth::FourEighteenths) => pos.x_advance = em * 4 / 18,
            Some(SpaceWidth::Figure) => {
                if let Some(w) = ('0'..='9').find_map(width_of) {
                    pos.x_advance = w;
                }
            }
            Some(SpaceWidth::Punctuation) => {
                if let Some(w) = width_of('.').or_else(|| width_of(',')) {
                    pos.x_advance = w;
                }
            }
            Some(SpaceWidth::Narrow) => pos.x_advance /= 2,
            Some(SpaceWidth::Space) | None => {}
        }
    }
}

/// Marks take no room of their own once positioned. Without GPOS their drawing is kept where
/// it was by moving the offset back by the width removed.
pub(super) fn zero_mark_widths(buf: &mut Buffer, adjust_offsets: bool) {
    for (info, pos) in buf.info[..buf.len].iter().zip(&mut buf.pos) {
        if info.is_mark() {
            if adjust_offsets {
                pos.x_offset -= pos.x_advance;
                pos.y_offset -= pos.y_advance;
            }
            pos.x_advance = 0;
            pos.y_advance = 0;
        }
    }
}

pub(super) fn zero_ignorable_widths(buf: &mut Buffer) {
    if !buf.has_ignorables {
        return;
    }
    for (info, pos) in buf.info[..buf.len].iter().zip(&mut buf.pos) {
        if info.is_default_ignorable() {
            *pos = GlyphPos {
                attach_chain: pos.attach_chain,
                attach_type: pos.attach_type,
                ..GlyphPos::ZERO
            };
        }
    }
}

/// Default ignorables (soft hyphen, joiners, variation selectors) are drawn as the space
/// glyph with no width, or removed when the face has no space.
pub(super) fn hide_ignorables(face: &Face<'_>, buf: &mut Buffer) {
    if !buf.has_ignorables {
        return;
    }
    if let Some(space) = face.glyph(' ') {
        for info in &mut buf.info[..buf.len] {
            if info.is_default_ignorable() {
                info.glyph = space;
            }
        }
    } else {
        buf.delete_where(crate::buffer::GlyphInfo::is_default_ignorable);
    }
}
