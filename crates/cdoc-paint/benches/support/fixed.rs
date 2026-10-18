//! The control for the varint choice: the size the same display list would take if every
//! varint and zigzag field were a fixed four-byte integer instead, field for field, with the
//! sections laid out as the real format lays them out.

use cdoc_layout::Segment;
use cdoc_paint::{DisplayList, Op};

const INT: usize = 4;
const RGBA: usize = 4;

fn segments(s: &[Segment]) -> usize {
    INT + s
        .iter()
        .map(|s| {
            1 + INT
                * match s {
                    Segment::Move(..) | Segment::Line(..) => 2,
                    Segment::Cubic(_) => 6,
                    Segment::Close => 0,
                }
        })
        .sum::<usize>()
}

fn payload(op: &Op) -> usize {
    let head = 5 * INT + RGBA + INT;
    match op {
        Op::FillRect { .. } => 4 * INT + RGBA,
        Op::StrokeRect { .. } | Op::Line { .. } => 5 * INT + RGBA,
        Op::FillPath { segments: s, .. } => RGBA + segments(s),
        Op::StrokePath { segments: s, .. } => INT + RGBA + segments(s),
        Op::GlyphRun { text, glyphs, .. } => head + INT + text.len() + glyphs.len() * 5 * INT,
        Op::Image { .. } | Op::Link { .. } => 5 * INT,
        Op::SigField { .. } => 6 * INT,
        Op::ClipPush(_) => 4 * INT,
        Op::ClipPop => 0,
        Op::CaretLines(lines) => lines
            .iter()
            .map(|l| (7 + l.from.path.len() + 2 * l.stops.len()) * INT)
            .sum(),
    }
}

/// Bytes with fixed-width integers. Strings are counted once each, as the string table does.
pub fn fixed_len(list: &DisplayList) -> usize {
    let mut strings = std::collections::BTreeSet::new();
    let ops: usize = list
        .pages
        .iter()
        .flat_map(|p| &p.ops)
        .map(|op| {
            match op {
                Op::Image { content_id: s, .. } | Op::Link { href: s, .. } => {
                    strings.insert(s.as_str());
                }
                Op::SigField { id, role, .. } => {
                    strings.extend([id.as_str(), role.as_str()]);
                }
                _ => {}
            }
            1 + INT + payload(op)
        })
        .sum();
    let faces: usize = list
        .faces
        .iter()
        .map(|f| 1 + INT + f.key.family.len() + 2 * INT + f.glyphs.len() * 2 * INT)
        .sum();
    let doc = &list.document;
    strings.extend(doc.title.iter().chain(&doc.language).map(String::as_str));
    strings.extend(doc.outline.iter().map(|e| e.title.as_str()));
    let document = 3 * INT + doc.outline.len() * (1 + 3 * INT);
    let table: usize = strings.iter().map(|s| INT + s.len()).sum();
    24 + INT + faces + INT + table + document + 20 * list.pages.len() + ops
}
