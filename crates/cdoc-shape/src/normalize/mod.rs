//! Unicode normalisation against the face: characters the font cannot draw are decomposed into
//! ones it can, marks are put in canonical order, and base-plus-mark sequences are recomposed
//! wherever the font has the precomposed glyph. The font decides, not a normal form: `é` stays
//! one glyph in a font that has it and becomes `e` + U+0301 in one that does not.

mod decompose;

use crate::buffer::{Buffer, GlyphInfo};
use crate::face::Face;
use crate::unicode::{self, Props};
use decompose::decompose_all;

/// Longest run of marks that is put in canonical order; longer runs are left as typed.
const MAX_COMBINING_MARKS: usize = 32;

fn ccc(info: &GlyphInfo) -> u8 {
    info.props.combining_class()
}

pub(super) fn is_variation_selector(cp: u32) -> bool {
    matches!(cp, 0x180B..=0x180D | 0x180F | 0xFE00..=0xFE0F | 0xE_0100..=0xE_01EF)
}

fn reorder_marks(buf: &mut Buffer) {
    let count = buf.len;
    let mut i = 0;
    while i < count {
        if ccc(&buf.info[i]) == 0 {
            i += 1;
            continue;
        }
        let mut end = i + 1;
        while end < count && ccc(&buf.info[end]) != 0 {
            end += 1;
        }
        if end - i <= MAX_COMBINING_MARKS {
            buf.sort_by_key(i, end, ccc);
        }
        i = end + 1;
    }
}

/// A combining grapheme joiner between marks already in order is kept visible to lookups.
fn unhide_joiners(buf: &mut Buffer) {
    for i in 1..buf.len.saturating_sub(1) {
        if buf.info[i].cp == 0x034F {
            let (last, next) = (ccc(&buf.info[i - 1]), ccc(&buf.info[i + 1]));
            if next == 0 || last <= next {
                buf.info[i].props.unhide();
            }
        }
    }
}

fn recompose(buf: &mut Buffer, face: &Face<'_>) {
    let count = buf.len;
    let mut starter = 0;
    buf.clear_output();
    buf.next_glyph();
    while buf.idx < count && !buf.failed {
        let cur = *buf.cur(0);
        if cur.props.is_mark() && (starter == buf.out_len - 1 || ccc(buf.prev()) < ccc(&cur)) {
            let composed = unicode::compose(buf.out_info(starter).cp, cur.cp)
                .and_then(|c| face.glyph_of(c).map(|g| (c, g)));
            if let Some((cp, glyph)) = composed {
                buf.next_glyph();
                buf.merge_out_clusters(starter, buf.out_len);
                buf.out_len -= 1;
                let info = buf.out_info_mut(starter);
                info.cp = cp;
                info.glyph = glyph;
                info.props = Props::of(cp);
                continue;
            }
        }
        buf.next_glyph();
        if ccc(buf.prev()) == 0 {
            starter = buf.out_len - 1;
        }
    }
    buf.sync();
}

/// Normalises the run against the face, leaving every character mapped to a glyph.
pub(crate) fn normalize(buf: &mut Buffer, face: &Face<'_>) {
    if buf.len == 0 {
        return;
    }
    let simple = decompose_all(buf, face);
    if !simple {
        reorder_marks(buf);
    }
    if buf.has_cgj {
        unhide_joiners(buf);
    }
    if !simple && !buf.failed {
        recompose(buf, face);
    }
}
