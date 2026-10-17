//! The legacy TrueType `kern` table, used when GPOS has no kerning for the run's script. Each
//! pair's adjustment is split between the two glyphs (half on the first's advance, half on the
//! second's advance and offset) so the visual gap changes by the full amount either way a line
//! is cut.

use crate::apply::skip::{Walker, ANY};
use crate::apply::{Ctx, Table, IGNORE_MARKS};
use crate::buffer::{Buffer, ATTACH_CURSIVE};
use crate::face::Face;
use ttf_parser::GlyphId;

/// Whether the face has a `kern` table at all.
pub(crate) fn has_table(face: &Face<'_>) -> bool {
    face.kern.is_some()
}

/// Whether any subtable is a state machine (its marks keep their widths then).
pub(crate) fn has_state_machine(face: &Face<'_>) -> bool {
    face.kern
        .is_some_and(|k| k.subtables.into_iter().any(|s| s.has_state_machine))
}

/// Whether any subtable kerns across the line (marks keep their offsets when zeroed then).
pub(crate) fn has_cross_stream(face: &Face<'_>) -> bool {
    face.kern
        .is_some_and(|k| k.subtables.into_iter().any(|s| s.has_cross_stream))
}

/// Applies the table's horizontal, non-variable subtables. State-machine subtables are skipped
/// (they are an Apple extension no Latin, Greek or Cyrillic font this shaper targets uses).
pub(crate) fn apply(face: &Face<'_>, buf: &mut Buffer) {
    let Some(table) = face.kern else {
        return;
    };
    let mut seen_cross_stream = false;
    for sub in table.subtables {
        if sub.variable || !sub.horizontal || sub.has_state_machine {
            continue;
        }
        if !seen_cross_stream && sub.has_cross_stream {
            seen_cross_stream = true;
            for pos in &mut buf.pos[..buf.len] {
                pos.attach_type = ATTACH_CURSIVE;
                pos.attach_chain = -1;
            }
        }
        let mut ctx = Ctx::new(face, buf, Table::Gpos);
        ctx.lookup_props = IGNORE_MARKS;
        let mut i = 0;
        while i < ctx.buf.len {
            let mut walker = Walker::new(&ctx, i, false);
            if !walker.next(&ctx, ANY) {
                i += 1;
                continue;
            }
            let j = walker.idx;
            let (left, right) = (ctx.buf.info[i].glyph, ctx.buf.info[j].glyph);
            let kern = sub
                .glyphs_kerning(GlyphId(left), GlyphId(right))
                .map_or(0, i32::from);
            if kern != 0 {
                let pos = &mut ctx.buf.pos;
                if sub.has_cross_stream {
                    pos[j].y_offset = kern;
                    ctx.buf.has_attachment = true;
                } else {
                    let first = kern >> 1;
                    let second = kern - first;
                    pos[i].x_advance += first;
                    pos[j].x_advance += second;
                    pos[j].x_offset += second;
                }
            }
            i = j;
        }
    }
}
