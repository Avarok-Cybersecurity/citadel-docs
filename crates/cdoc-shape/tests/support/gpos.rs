//! GPOS subtables and the legacy `kern` table for test fonts.

use super::layout::{classes, coverage};
use super::{u16s, with_children};

/// Value format bits: x placement, y placement, x advance.
pub const X_PLACEMENT: u16 = 0x0001;
pub const Y_PLACEMENT: u16 = 0x0002;
pub const X_ADVANCE: u16 = 0x0004;

fn i16s(values: &[i16]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_be_bytes()).collect()
}

/// GPOS 1 format 1: one value record for every covered glyph.
pub fn single(glyphs: &[u16], format: u16, value: &[i16]) -> Vec<u8> {
    let mut h = u16s(&[1, 0, format]);
    h.extend(i16s(value));
    with_children(h, vec![(2, coverage(glyphs))])
}

/// GPOS 2 format 1: `(first, second, first's values, second's values)`.
pub fn pairs(format1: u16, format2: u16, entries: &[(u16, u16, &[i16], &[i16])]) -> Vec<u8> {
    let mut firsts: Vec<u16> = entries.iter().map(|e| e.0).collect();
    firsts.sort_unstable();
    firsts.dedup();
    let mut h = u16s(&[
        1,
        0,
        format1,
        format2,
        u16::try_from(firsts.len()).expect("firsts"),
    ]);
    h.extend(vec![0; 2 * firsts.len()]);
    let mut children = vec![(2, coverage(&firsts))];
    for (i, &first) in firsts.iter().enumerate() {
        let mut rows: Vec<_> = entries.iter().filter(|e| e.0 == first).collect();
        rows.sort_unstable_by_key(|e| e.1);
        let mut set = u16s(&[u16::try_from(rows.len()).expect("rows")]);
        for (_, second, v1, v2) in rows {
            set.extend(u16s(&[*second]));
            set.extend(i16s(v1));
            set.extend(i16s(v2));
        }
        children.push((10 + 2 * i, set));
    }
    with_children(h, children)
}

/// GPOS 2 format 2: class definitions and an x-advance matrix for the first glyph.
pub fn class_pairs(
    first: &[u16],
    class1: &[(u16, u16, u16)],
    class2: &[(u16, u16, u16)],
    matrix: &[&[i16]],
) -> Vec<u8> {
    let rows = u16::try_from(matrix.len()).expect("rows");
    let cols = u16::try_from(matrix[0].len()).expect("cols");
    let mut h = u16s(&[2, 0, X_ADVANCE, 0, 0, 0, rows, cols]);
    for row in matrix {
        h.extend(i16s(row));
    }
    with_children(
        h,
        vec![
            (2, coverage(first)),
            (8, classes(class1)),
            (10, classes(class2)),
        ],
    )
}

fn anchor(x: i16, y: i16) -> Vec<u8> {
    let mut a = u16s(&[1]);
    a.extend(i16s(&[x, y]));
    a
}

/// A mark array: `(class, anchor)` per covered mark, in coverage order.
fn mark_array(marks: &[(u16, (i16, i16))]) -> Vec<u8> {
    let mut h = u16s(&[u16::try_from(marks.len()).expect("marks")]);
    let mut children = Vec::new();
    for (i, &(class, (x, y))) in marks.iter().enumerate() {
        h.extend(u16s(&[class, 0]));
        children.push((2 + 4 * i + 2, anchor(x, y)));
    }
    with_children(h, children)
}

/// An anchor matrix: one row per base (or component), one anchor per mark class.
fn anchor_matrix(rows: &[&[(i16, i16)]]) -> Vec<u8> {
    let mut h = u16s(&[u16::try_from(rows.len()).expect("rows")]);
    let mut children = Vec::new();
    for row in rows {
        for &(x, y) in *row {
            children.push((h.len(), anchor(x, y)));
            h.extend([0, 0]);
        }
    }
    with_children(h, children)
}

/// GPOS 4 (mark to base) or 6 (mark to mark, `bases` being the marks attached to).
pub fn mark_attach(
    marks: &[u16],
    bases: &[u16],
    mark_anchors: &[(u16, (i16, i16))],
    base_anchors: &[&[(i16, i16)]],
) -> Vec<u8> {
    let classes = mark_anchors.iter().map(|m| m.0).max().map_or(0, |c| c + 1);
    with_children(
        u16s(&[1, 0, 0, classes, 0, 0]),
        vec![
            (2, coverage(marks)),
            (4, coverage(bases)),
            (8, mark_array(mark_anchors)),
            (10, anchor_matrix(base_anchors)),
        ],
    )
}

/// GPOS 5: one ligature with one anchor row per component.
pub fn mark_ligature(
    marks: &[u16],
    ligature: u16,
    mark_anchors: &[(u16, (i16, i16))],
    components: &[&[(i16, i16)]],
) -> Vec<u8> {
    let classes = mark_anchors.iter().map(|m| m.0).max().map_or(0, |c| c + 1);
    let array = with_children(u16s(&[1, 0]), vec![(2, anchor_matrix(components))]);
    with_children(
        u16s(&[1, 0, 0, classes, 0, 0]),
        vec![
            (2, coverage(marks)),
            (4, coverage(&[ligature])),
            (8, mark_array(mark_anchors)),
            (10, array),
        ],
    )
}

/// A version 0 `kern` table with one horizontal format 0 subtable.
pub fn kern(pairs: &[(u16, u16, i16)]) -> Vec<u8> {
    let mut sorted = pairs.to_vec();
    sorted.sort_unstable_by_key(|p| (p.0, p.1));
    let n = u16::try_from(sorted.len()).expect("pairs");
    let mut sub = u16s(&[0, 0, 0x0001, n, 0, 0, 0]);
    for (l, r, v) in sorted {
        sub.extend(u16s(&[l, r]));
        sub.extend(i16s(&[v]));
    }
    let len = u16::try_from(sub.len()).expect("length");
    sub[2..4].copy_from_slice(&len.to_be_bytes());
    let mut t = u16s(&[0, 1]);
    t.extend(sub);
    t
}
