//! The writer: header, face table, string table, document table, page index, then the page
//! bodies, in that order, so a reader holding only a prefix that ends after page N's body can
//! paint pages up to N.

use crate::codec::Writer;
use crate::encode_tables::{u32_of, Strings};
use crate::format::{
    HEADER_LEN, INDEX_ENTRY_LEN, MAGIC, OP_CARET_LINES, OP_CLIP_POP, OP_CLIP_PUSH, OP_FILL_PATH,
    OP_FILL_RECT, OP_GLYPH_RUN, OP_IMAGE, OP_LINE, OP_LINK, OP_SIG_FIELD, OP_STROKE_PATH,
    OP_STROKE_RECT, SEG_CLOSE, SEG_CUBIC, SEG_LINE, SEG_MOVE, STYLE_BOLD, STYLE_ITALIC, VERSION,
};
use crate::op::{DisplayList, DocumentInfo, Op, Rgba};
use crate::run_encode::RunWriter;
use crate::run_tables::RunTables;
use crate::PaintError;
use cdoc_layout::{Rect, Segment};

fn text(w: &mut Writer, s: &str) -> Result<(), PaintError> {
    w.uv(u32_of(s.len())?);
    w.bytes(s.as_bytes());
    Ok(())
}

fn rect(w: &mut Writer, r: Rect) {
    w.zzs(&[r.x, r.y, r.width, r.height]);
}

fn rgba(w: &mut Writer, c: Rgba) {
    w.bytes(&[c.r, c.g, c.b, c.a]);
}

fn segments(w: &mut Writer, segments: &[Segment]) -> Result<(), PaintError> {
    w.uv(u32_of(segments.len())?);
    for s in segments {
        let (verb, coords): (u8, &[i32]) = match s {
            Segment::Move(x, y) => (SEG_MOVE, &[*x, *y]),
            Segment::Line(x, y) => (SEG_LINE, &[*x, *y]),
            Segment::Cubic(c) => (SEG_CUBIC, c),
            Segment::Close => (SEG_CLOSE, &[]),
        };
        w.u8(verb);
        w.zzs(coords);
    }
    Ok(())
}

/// One op's opcode and payload (without the length prefix).
fn payload(
    w: &mut Writer,
    op: &Op,
    strings: &Strings<'_>,
    runs: &mut RunWriter,
    tables: &RunTables,
) -> Result<u8, PaintError> {
    Ok(match op {
        Op::FillRect { rect: r, colour } => {
            rect(w, *r);
            rgba(w, *colour);
            OP_FILL_RECT
        }
        Op::StrokeRect {
            rect: r,
            width,
            colour,
        } => {
            rect(w, *r);
            w.zz(*width);
            rgba(w, *colour);
            OP_STROKE_RECT
        }
        Op::Line {
            from,
            to,
            width,
            colour,
        } => {
            w.zzs(&[from.0, from.1, to.0, to.1, *width]);
            rgba(w, *colour);
            OP_LINE
        }
        Op::FillPath {
            segments: s,
            colour,
        } => {
            rgba(w, *colour);
            segments(w, s)?;
            OP_FILL_PATH
        }
        Op::StrokePath {
            segments: s,
            width,
            colour,
        } => {
            w.zz(*width);
            rgba(w, *colour);
            segments(w, s)?;
            OP_STROKE_PATH
        }
        Op::GlyphRun {
            head: h,
            text: t,
            glyphs,
        } => {
            runs.run(w, h, t, glyphs, tables)?;
            OP_GLYPH_RUN
        }
        Op::Image {
            rect: r,
            content_id: s,
        }
        | Op::Link { rect: r, href: s } => {
            rect(w, *r);
            w.uv(strings.index[s.as_str()]);
            if matches!(op, Op::Image { .. }) {
                OP_IMAGE
            } else {
                OP_LINK
            }
        }
        Op::SigField { rect: r, id, role } => {
            rect(w, *r);
            w.uv(strings.index[id.as_str()]);
            w.uv(strings.index[role.as_str()]);
            OP_SIG_FIELD
        }
        Op::ClipPush(r) => {
            rect(w, *r);
            OP_CLIP_PUSH
        }
        Op::ClipPop => OP_CLIP_POP,
        Op::CaretLines(lines) => {
            crate::caret::encode(w, lines)?;
            OP_CARET_LINES
        }
    })
}

/// The document table: title and language (0 none, else string index + 1), then the outline.
fn document(w: &mut Writer, doc: &DocumentInfo, strings: &Strings<'_>) -> Result<(), PaintError> {
    w.uv(strings.optional(doc.title.as_ref()));
    w.uv(strings.optional(doc.language.as_ref()));
    w.uv(u32_of(doc.outline.len())?);
    for entry in &doc.outline {
        w.u8(entry.level);
        w.uv(entry.page);
        w.zz(entry.y);
        w.uv(strings.index[entry.title.as_str()]);
    }
    Ok(())
}

impl DisplayList {
    /// The list in the binary format (`docs/display-list.md`).
    pub fn to_bytes(&self) -> Result<Vec<u8>, PaintError> {
        let ops: usize = self.pages.iter().map(|p| p.ops.len()).sum();
        let mut w = Writer::with_capacity(HEADER_LEN + 64 * self.faces.len() + 24 * ops);
        w.bytes(&MAGIC);
        w.u16(VERSION);
        w.u16(0);
        w.u32(u32_of(self.pages.len())?);
        let offsets_at = w.buf.len();
        w.bytes(&[0; 12]);
        debug_assert_eq!(w.buf.len(), HEADER_LEN);
        let tables = RunTables::collect(self)?;
        w.uv(u32_of(self.faces.len())?);
        for (slot, face) in self.faces.iter().enumerate() {
            let k = &face.key;
            w.u8((u8::from(k.bold) * STYLE_BOLD) | (u8::from(k.italic) * STYLE_ITALIC));
            text(&mut w, &k.family)?;
            w.uv(u32::from(face.units_per_em));
            w.uv(u32_of(face.glyphs.len())?);
            for g in &face.glyphs {
                w.uv(u32::from(g.id));
                w.uv(u32::from(g.advance));
            }
            tables.write_chars(&mut w, slot)?;
        }
        tables.write_strikes(&mut w)?;
        let strings_offset = u32_of(w.buf.len())?;
        let strings = Strings::collect(self)?;
        w.uv(u32_of(strings.order.len())?);
        for s in &strings.order {
            text(&mut w, s)?;
        }
        document(&mut w, &self.document, &strings)?;
        let index_offset = w.buf.len();
        w.bytes(&vec![0; INDEX_ENTRY_LEN * self.pages.len()]);
        let mut scratch = Writer::with_capacity(64);
        let mut runs = RunWriter::new();
        for (slot, page) in self.pages.iter().enumerate() {
            let start = w.buf.len();
            for op in &page.ops {
                scratch.buf.clear();
                let code = payload(&mut scratch, op, &strings, &mut runs, &tables)?;
                w.u8(code);
                w.uv(u32_of(scratch.buf.len())?);
                w.bytes(&scratch.buf);
            }
            let entry = index_offset + slot * INDEX_ENTRY_LEN;
            let fields = [
                page.number,
                u32_of(start)?,
                u32_of(w.buf.len() - start)?,
                page.width.cast_unsigned(),
                page.height.cast_unsigned(),
            ];
            for (k, value) in fields.into_iter().enumerate() {
                w.patch_u32(entry + 4 * k, value);
            }
        }
        w.patch_u32(offsets_at, u32_of(HEADER_LEN)?);
        w.patch_u32(offsets_at + 4, strings_offset);
        w.patch_u32(offsets_at + 8, u32_of(index_offset)?);
        Ok(w.buf)
    }
}
