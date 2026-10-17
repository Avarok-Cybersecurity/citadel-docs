//! A font built byte by byte for the lookup tests: a handful of glyphs, a character map, and
//! exactly the GSUB, GPOS, GDEF or `kern` data a test describes, so each expectation is written
//! by hand against tables small enough to read.

#![allow(dead_code)]

pub mod fixture;
pub mod gpos;
pub mod layout;

/// Big-endian byte writing.
pub fn u16s(values: &[u16]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_be_bytes()).collect()
}

/// A table whose 16-bit offsets point at child tables appended after it: `(position of the
/// offset field, child bytes)`. Offsets are from the table's start.
pub fn with_children(mut head: Vec<u8>, children: Vec<(usize, Vec<u8>)>) -> Vec<u8> {
    for (at, child) in children {
        let offset = u16::try_from(head.len()).expect("table fits 64 KiB");
        head[at..at + 2].copy_from_slice(&offset.to_be_bytes());
        head.extend(child);
    }
    head
}

/// The pieces of a test font.
pub struct TestFont {
    /// Glyph advances; glyph 0 is `.notdef`.
    pub advances: Vec<u16>,
    /// Characters and their glyphs.
    pub cmap: Vec<(char, u16)>,
    /// Extra tables: tag and bytes.
    pub tables: Vec<([u8; 4], Vec<u8>)>,
}

fn head() -> Vec<u8> {
    let mut t = u16s(&[1, 0, 1, 0, 0, 0, 0x5F0F, 0x3CF5, 0, 1000]);
    t.extend([0; 16]); // created, modified
    t.extend(u16s(&[0, 0xFF38, 1000, 800, 0, 8, 2, 0, 0]));
    t
}

fn hhea(glyphs: u16) -> Vec<u8> {
    let mut t = u16s(&[1, 0, 800, 0xFF38, 0, 1000, 0, 0, 1000, 1, 0, 0]);
    t.extend([0; 8]);
    t.extend(u16s(&[0, glyphs]));
    t
}

/// Format 4, one segment per character.
fn cmap(map: &[(char, u16)]) -> Vec<u8> {
    let mut map: Vec<(u16, u16)> = map
        .iter()
        .map(|&(c, g)| (u16::try_from(u32::from(c)).expect("BMP"), g))
        .collect();
    map.sort_unstable();
    map.push((0xFFFF, 0));
    let segs = u16::try_from(map.len()).expect("segments");
    let mut sub = u16s(&[4, 0, 0, segs * 2, 0, 0, 0]);
    sub.extend(map.iter().flat_map(|&(c, _)| c.to_be_bytes()));
    sub.extend([0, 0]);
    sub.extend(map.iter().flat_map(|&(c, _)| c.to_be_bytes()));
    sub.extend(
        map.iter()
            .flat_map(|&(c, g)| g.wrapping_sub(c).to_be_bytes()),
    );
    sub.extend(map.iter().flat_map(|_| [0, 0]));
    let len = u16::try_from(sub.len()).expect("cmap length");
    sub[2..4].copy_from_slice(&len.to_be_bytes());
    let mut t = u16s(&[0, 1, 3, 1, 0, 12]);
    t.extend(sub);
    t
}

impl TestFont {
    /// Assembles the sfnt.
    pub fn build(&self) -> Vec<u8> {
        let glyphs = u16::try_from(self.advances.len()).expect("glyph count");
        let hmtx: Vec<u8> = self.advances.iter().flat_map(|&a| u16s(&[a, 0])).collect();
        let mut tables = vec![
            (*b"cmap", cmap(&self.cmap)),
            (*b"head", head()),
            (*b"hhea", hhea(glyphs)),
            (*b"hmtx", hmtx),
            (*b"maxp", u16s(&[0, 0x5000, glyphs])),
        ];
        tables.extend(self.tables.iter().cloned());
        tables.sort_by_key(|(tag, _)| *tag);
        let count = u16::try_from(tables.len()).expect("tables");
        let mut out = u16s(&[1, 0, count, 0, 0, 0]);
        let mut offset = 12 + 16 * tables.len();
        let mut body = Vec::new();
        for (tag, data) in &tables {
            out.extend(tag);
            out.extend([0; 4]); // checksum, unchecked
            out.extend(u32::try_from(offset).expect("offset").to_be_bytes());
            out.extend(u32::try_from(data.len()).expect("length").to_be_bytes());
            let mut padded = data.clone();
            padded.resize(data.len().div_ceil(4) * 4, 0);
            offset += padded.len();
            body.extend(padded);
        }
        out.extend(body);
        out
    }
}
