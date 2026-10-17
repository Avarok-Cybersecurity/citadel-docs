//! GSUB, GPOS and GDEF bytes for test fonts. Every builder returns a self-contained table with
//! offsets from its own start, as the OpenType spec lays them out.

use super::{u16s, with_children};

/// A lookup: type, flags, subtables, optional mark filtering set.
pub struct Lookup {
    pub kind: u16,
    pub flag: u16,
    pub subtables: Vec<Vec<u8>>,
    pub mark_set: Option<u16>,
}

pub fn lookup(kind: u16, subtables: Vec<Vec<u8>>) -> Lookup {
    Lookup {
        kind,
        flag: 0,
        subtables,
        mark_set: None,
    }
}

fn offsets_table(head: Vec<u8>, first_field: usize, children: Vec<Vec<u8>>) -> Vec<u8> {
    let children = children
        .into_iter()
        .enumerate()
        .map(|(i, c)| (first_field + 2 * i, c))
        .collect();
    with_children(head, children)
}

/// A GSUB or GPOS table: scripts `DFLT` and `latn` with one default language system holding
/// every feature, in order.
pub fn table(features: &[(&[u8; 4], &[u16])], lookups: Vec<Lookup>) -> Vec<u8> {
    let n = u16::try_from(features.len()).expect("features");
    let mut langsys = u16s(&[0, 0xFFFF, n]);
    langsys.extend(u16s(&(0..n).collect::<Vec<_>>()));
    let script = with_children(u16s(&[0, 0]), vec![(0, langsys)]);
    let mut scripts = u16s(&[2]);
    scripts.extend(b"DFLT");
    scripts.extend([0, 0]);
    scripts.extend(b"latn");
    scripts.extend([0, 0]);
    let scripts = with_children(scripts, vec![(6, script.clone()), (12, script)]);
    let mut list = u16s(&[n]);
    let mut children = Vec::new();
    for (i, (tag, indices)) in features.iter().enumerate() {
        list.extend(*tag);
        list.extend([0, 0]);
        let mut feature = u16s(&[0, u16::try_from(indices.len()).expect("lookups")]);
        feature.extend(u16s(indices));
        children.push((2 + 6 * i + 4, feature));
    }
    let features = with_children(list, children);
    let count = u16::try_from(lookups.len()).expect("lookups");
    let mut head = u16s(&[count]);
    head.extend(vec![0; 2 * lookups.len()]);
    let lookups = lookups
        .into_iter()
        .map(|l| {
            let flag = l.flag | if l.mark_set.is_some() { 0x10 } else { 0 };
            let mut h = u16s(&[
                l.kind,
                flag,
                u16::try_from(l.subtables.len()).expect("subtables"),
            ]);
            h.extend(vec![0; 2 * l.subtables.len()]);
            if let Some(set) = l.mark_set {
                h.extend(u16s(&[set]));
            }
            offsets_table(h, 6, l.subtables)
        })
        .collect();
    let lookups = offsets_table(head, 2, lookups);
    with_children(
        u16s(&[1, 0, 0, 0, 0]),
        vec![(4, scripts), (6, features), (8, lookups)],
    )
}

pub fn coverage(glyphs: &[u16]) -> Vec<u8> {
    let mut sorted = glyphs.to_vec();
    sorted.sort_unstable();
    let mut t = u16s(&[1, u16::try_from(sorted.len()).expect("glyphs")]);
    t.extend(u16s(&sorted));
    t
}

/// Class definition format 2: `(first, last, class)` ranges.
pub fn classes(ranges: &[(u16, u16, u16)]) -> Vec<u8> {
    let mut t = u16s(&[2, u16::try_from(ranges.len()).expect("ranges")]);
    for &(a, b, c) in ranges {
        t.extend(u16s(&[a, b, c]));
    }
    t
}

/// GSUB 1 format 2: `from[i]` becomes `to[i]`.
pub fn single(from: &[u16], to: &[u16]) -> Vec<u8> {
    let mut pairs: Vec<(u16, u16)> = from.iter().copied().zip(to.iter().copied()).collect();
    pairs.sort_unstable();
    let mut h = u16s(&[2, 0, u16::try_from(pairs.len()).expect("pairs")]);
    h.extend(pairs.iter().flat_map(|&(_, t)| t.to_be_bytes()));
    with_children(h, vec![(2, coverage(from))])
}

/// GSUB 2 or 3 (same layout): `glyph` becomes (or may become) `seq`.
pub fn sequences(entries: &[(u16, &[u16])]) -> Vec<u8> {
    let mut sorted = entries.to_vec();
    sorted.sort_unstable_by_key(|e| e.0);
    let glyphs: Vec<u16> = sorted.iter().map(|e| e.0).collect();
    let mut h = u16s(&[1, 0, u16::try_from(sorted.len()).expect("entries")]);
    h.extend(vec![0; 2 * sorted.len()]);
    let mut children = vec![(2, coverage(&glyphs))];
    for (i, (_, seq)) in sorted.iter().enumerate() {
        let mut s = u16s(&[u16::try_from(seq.len()).expect("seq")]);
        s.extend(u16s(seq));
        children.push((6 + 2 * i, s));
    }
    with_children(h, children)
}

/// GSUB 4: each `(components, ligature)`; components all start with `first`.
pub fn ligatures(first: u16, ligs: &[(&[u16], u16)]) -> Vec<u8> {
    let mut set = u16s(&[u16::try_from(ligs.len()).expect("ligs")]);
    set.extend(vec![0; 2 * ligs.len()]);
    let children = ligs
        .iter()
        .enumerate()
        .map(|(i, (comps, lig))| {
            let mut l = u16s(&[*lig, u16::try_from(comps.len()).expect("comps")]);
            l.extend(u16s(&comps[1..]));
            (2 + 2 * i, l)
        })
        .collect();
    let set = with_children(set, children);
    with_children(u16s(&[1, 0, 1, 0]), vec![(2, coverage(&[first])), (6, set)])
}

/// Chained context format 3 (GSUB 6 / GPOS 8): coverage sequences and `(position, lookup)`.
pub fn chain(
    back: &[&[u16]],
    input: &[&[u16]],
    ahead: &[&[u16]],
    records: &[(u16, u16)],
) -> Vec<u8> {
    let mut h = Vec::new();
    let mut children = Vec::new();
    for seq in [back, input, ahead] {
        h.extend(u16s(&[u16::try_from(seq.len()).expect("seq")]));
        for cov in seq {
            children.push((h.len(), coverage(cov)));
            h.extend([0, 0]);
        }
    }
    h.extend(u16s(&[u16::try_from(records.len()).expect("records")]));
    for &(at, lookup) in records {
        h.extend(u16s(&[at, lookup]));
    }
    let mut full = u16s(&[3]);
    full.extend(h);
    let children = children.into_iter().map(|(at, c)| (at + 2, c)).collect();
    with_children(full, children)
}

/// Extension (GSUB 7 / GPOS 9) around a subtable of `kind`.
pub fn extension(kind: u16, subtable: Vec<u8>) -> Vec<u8> {
    let mut t = u16s(&[1, kind, 0, 8]);
    t.extend(subtable);
    t
}

/// GSUB 8 with one backtrack and one lookahead coverage.
pub fn reverse(from: &[u16], to: &[u16], back: &[u16], ahead: &[u16]) -> Vec<u8> {
    let mut h = u16s(&[1, 0, 1, 0, 1, 0, u16::try_from(to.len()).expect("to")]);
    h.extend(u16s(to));
    with_children(
        h,
        vec![
            (2, coverage(from)),
            (6, coverage(back)),
            (10, coverage(ahead)),
        ],
    )
}

/// GDEF: glyph classes `(first, last, class)` and ligature carets `(glyph, [x...])`.
pub fn gdef(glyph_classes: &[(u16, u16, u16)], carets: &[(u16, &[i16])]) -> Vec<u8> {
    let glyphs: Vec<u16> = carets.iter().map(|c| c.0).collect();
    let mut list = u16s(&[0, u16::try_from(carets.len()).expect("carets")]);
    list.extend(vec![0; 2 * carets.len()]);
    let mut children = vec![(0, coverage(&glyphs))];
    for (i, (_, xs)) in carets.iter().enumerate() {
        let mut lig = u16s(&[u16::try_from(xs.len()).expect("xs")]);
        lig.extend(vec![0; 2 * xs.len()]);
        let values = xs
            .iter()
            .enumerate()
            .map(|(j, &x)| (2 + 2 * j, u16s(&[1, u16::from_be_bytes(x.to_be_bytes())])))
            .collect();
        children.push((4 + 2 * i, with_children(lig, values)));
    }
    let list = with_children(list, children);
    with_children(
        u16s(&[1, 0, 0, 0, 0, 0]),
        vec![(4, classes(glyph_classes)), (8, list)],
    )
}
