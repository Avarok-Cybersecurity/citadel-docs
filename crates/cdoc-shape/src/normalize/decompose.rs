//! The decomposition pass: each character gets its nominal glyph, or is decomposed (one
//! canonical step at a time, recursively) into characters the face has; inside a cluster with
//! marks everything is decomposed as far as the face allows, so recomposition starts clean.

use super::is_variation_selector;
use crate::buffer::Buffer;
use crate::face::Face;
use crate::unicode::{self, Category, Props};

fn output_char(buf: &mut Buffer, cp: u32, glyph: u16) {
    let mut info = *buf.cur(0);
    info.cp = cp;
    info.glyph = glyph;
    info.props = Props::of(cp);
    buf.has_ignorables |= info.props.is_ignorable();
    buf.has_cgj |= cp == 0x034F;
    buf.output_info(info);
}

/// Writes the decomposition of `ab` the font can draw; how many characters, 0 for none.
fn decompose(buf: &mut Buffer, face: &Face<'_>, shortest: bool, ab: u32) -> u32 {
    let Some((a, b)) = unicode::decompose(ab) else {
        return 0;
    };
    let a_glyph = face.glyph_of(a);
    let b_glyph = if b == 0 {
        None
    } else {
        match face.glyph_of(b) {
            Some(g) => Some(g),
            None => return 0,
        }
    };
    if !shortest || a_glyph.is_none() {
        let written = decompose(buf, face, shortest, a);
        if written != 0 {
            if let Some(g) = b_glyph {
                output_char(buf, b, g);
                return written + 1;
            }
            return written;
        }
    }
    let Some(g) = a_glyph else {
        return 0;
    };
    output_char(buf, a, g);
    match b_glyph {
        Some(bg) => {
            output_char(buf, b, bg);
            2
        }
        None => 1,
    }
}

fn decompose_current(buf: &mut Buffer, face: &Face<'_>, shortest: bool) {
    let cp = buf.cur(0).cp;
    let glyph = face.glyph_of(cp);
    if (!shortest || glyph.is_none()) && decompose(buf, face, shortest, cp) > 0 {
        buf.skip_glyph();
        return;
    }
    let fallback = if glyph.is_some() {
        glyph
    } else if buf.cur(0).props.category() == Category::Space && unicode::space_width(cp).is_some() {
        let space = face.glyph_of(0x20);
        if space.is_some() {
            buf.cur_mut(0).props.set_space_fallback();
            buf.has_space_fallback = true;
        }
        space
    } else if cp == 0x2011 {
        face.glyph_of(0x2010)
    } else {
        None
    };
    buf.cur_mut(0).glyph = fallback.unwrap_or(0);
    buf.next_glyph();
}

fn set_nominal(buf: &mut Buffer, face: &Face<'_>) {
    if let Some(g) = face.glyph_of(buf.cur(0).cp) {
        buf.cur_mut(0).glyph = g;
    }
}

/// A cluster holding variation selectors: nothing is decomposed, each character keeps its
/// nominal glyph and the selectors are left for hiding.
fn variation_cluster(buf: &mut Buffer, face: &Face<'_>, end: usize) {
    while buf.idx + 1 < end && !buf.failed {
        if is_variation_selector(buf.cur(1).cp) {
            set_nominal(buf, face);
            buf.next_glyph();
            buf.cur_mut(0).props.set_category(Category::Format);
            set_nominal(buf, face);
            buf.next_glyph();
            while buf.idx < end && is_variation_selector(buf.cur(0).cp) {
                set_nominal(buf, face);
                buf.next_glyph();
            }
        } else {
            set_nominal(buf, face);
            buf.next_glyph();
        }
    }
    if buf.idx < end {
        set_nominal(buf, face);
        buf.next_glyph();
    }
}

fn decompose_cluster(buf: &mut Buffer, face: &Face<'_>, end: usize) {
    if (buf.idx..end).any(|i| is_variation_selector(buf.info[i].cp)) {
        variation_cluster(buf, face, end);
        return;
    }
    while buf.idx < end && !buf.failed {
        decompose_current(buf, face, false);
    }
}

/// Maps every character to a glyph, decomposing where the font needs it. Whether any cluster
/// held a mark (only then is there anything to reorder or recompose).
pub(super) fn decompose_all(buf: &mut Buffer, face: &Face<'_>) -> bool {
    let count = buf.len;
    let mut simple = true;
    buf.clear_output();
    loop {
        let mut end = buf.idx + 1;
        while end < count && !buf.info[end].props.is_mark() {
            end += 1;
        }
        if end < count {
            end -= 1; // the base before the mark starts the next cluster
        }
        let mut done = 0;
        while buf.idx + done < end {
            match face.glyph_of(buf.cur(done).cp) {
                Some(g) => buf.cur_mut(done).glyph = g,
                None => break,
            }
            done += 1;
        }
        buf.next_glyphs(done);
        while buf.idx < end && !buf.failed {
            decompose_current(buf, face, true);
        }
        if buf.idx >= count || buf.failed {
            break;
        }
        simple = false;
        let mut end = buf.idx + 1;
        while end < count && buf.info[end].props.is_mark() {
            end += 1;
        }
        decompose_cluster(buf, face, end);
        if buf.idx >= count || buf.failed {
            break;
        }
    }
    buf.sync();
    simple
}
