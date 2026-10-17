//! Text for the SVG painter. The display list carries glyph ids, not text; SVG wants text (so
//! the preview is selectable and searchable, and draws in the `@font-face` the UI loaded), so
//! the characters are recovered from each face's `cmap`. Where a face maps several characters
//! to one glyph (a space and a no-break space) the lowest code point is written, which is also
//! what the PDF's `ToUnicode` map says, so copying text from either gives the same string.

use super::{attr_paint, escape};
use crate::decode::FaceView;
use crate::op::RunHead;
use crate::{NominalGlyphs, PaintError, ShapedGlyphs};
use cdoc_layout::ir::to_pt;
use cdoc_layout::FontPort;
use std::fmt::Write as _;

/// Glyph id → character, per face, built the first time a face is painted.
pub struct Characters<'a> {
    fonts: &'a dyn FontPort,
    by_face: Vec<Option<Vec<char>>>,
}

const NONE: char = '\0';

fn reverse_cmap(face: &ttf_parser::Face<'_>) -> Vec<char> {
    let mut out = vec![NONE; usize::from(face.number_of_glyphs())];
    let Some(cmap) = face.tables().cmap else {
        return out;
    };
    for sub in cmap
        .subtables
        .into_iter()
        .filter(ttf_parser::cmap::Subtable::is_unicode)
    {
        sub.codepoints(|cp| {
            let (Some(gid), Some(ch)) = (sub.glyph_index(cp), char::from_u32(cp)) else {
                return;
            };
            if let Some(slot) = out.get_mut(usize::from(gid.0)) {
                if *slot == NONE || ch < *slot {
                    *slot = ch;
                }
            }
        });
    }
    out
}

impl<'a> Characters<'a> {
    pub fn new(fonts: &'a dyn FontPort, faces: usize) -> Self {
        Self {
            fonts,
            by_face: vec![None; faces],
        }
    }

    fn table(&mut self, id: u16, face: &FaceView<'_>) -> Result<&[char], PaintError> {
        let index = usize::from(id);
        if self.by_face[index].is_none() {
            let source = self.fonts.face(face.family, face.bold, face.italic)?;
            let parsed = ttf_parser::Face::parse(source.bytes, 0)
                .map_err(|_| PaintError::Unreadable(face.key()))?;
            self.by_face[index] = Some(reverse_cmap(&parsed));
        }
        Ok(self.by_face[index].as_deref().unwrap_or(&[]))
    }

    fn text(
        &mut self,
        head: &RunHead,
        face: &FaceView<'_>,
        ids: impl Iterator<Item = u16>,
    ) -> Result<String, PaintError> {
        let table = self.table(head.face.0, face)?;
        Ok(ids
            .map(|id| match table.get(usize::from(id)) {
                Some(&ch) if ch != NONE => ch,
                _ => char::REPLACEMENT_CHARACTER,
            })
            .collect())
    }
}

fn family(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_'))
        .collect()
}

fn open(out: &mut String, head: &RunHead, face: &FaceView<'_>, x: &str, y: &str) {
    let _ = write!(
        out,
        "<text x=\"{x}\" y=\"{y}\" font-family=\"'{}'\" font-weight=\"{}\" font-style=\"{}\" \
         font-size=\"{}\" {} xml:space=\"preserve\">",
        family(face.family),
        if face.bold { 700 } else { 400 },
        if face.italic { "italic" } else { "normal" },
        to_pt(head.size),
        attr_paint("fill", head.colour),
    );
}

/// An unshaped run: one origin, the browser advancing by the same nominal widths (kerning is
/// off for the whole sheet).
pub fn nominal(
    out: &mut String,
    chars: &mut Characters<'_>,
    head: &RunHead,
    face: &FaceView<'_>,
    glyphs: NominalGlyphs<'_>,
) -> Result<(), PaintError> {
    let text = chars.text(head, face, glyphs.map(|g| g.id))?;
    open(
        out,
        head,
        face,
        &to_pt(head.x).to_string(),
        &to_pt(head.y).to_string(),
    );
    let _ = write!(out, "{}</text>", escape(&text));
    Ok(())
}

/// A shaped run: every glyph at its own position, from the advances and offsets.
pub fn shaped(
    out: &mut String,
    chars: &mut Characters<'_>,
    head: &RunHead,
    face: &FaceView<'_>,
    glyphs: ShapedGlyphs<'_>,
) -> Result<(), PaintError> {
    let (mut xs, mut ys, mut pen) = (String::new(), String::new(), head.x);
    for g in glyphs.clone() {
        let _ = write!(xs, "{} ", to_pt(pen + g.x_offset));
        let _ = write!(ys, "{} ", to_pt(head.y + g.y_offset));
        pen += g.advance;
    }
    let text = chars.text(head, face, glyphs.map(|g| g.id))?;
    open(out, head, face, xs.trim_end(), ys.trim_end());
    let _ = write!(out, "{}</text>", escape(&text));
    Ok(())
}
