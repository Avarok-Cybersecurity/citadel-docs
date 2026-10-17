//! Glyph outlines and face metrics as the canvas atlas reads them: the record parsed the way
//! `docs/display-list.md` says, compared with what `ttf-parser` reports for the same glyph.

mod support;

use cdoc_paint::glyph::{
    face_metrics, glyph_path, GlyphError, PATH_HEADER_LEN, PATH_VERSION, VERB_CLOSE, VERB_CUBIC,
    VERB_LINE, VERB_MOVE, VERB_QUAD,
};

/// `(verb, coordinates)` per command, and the bounding box, read from a record.
fn read(record: &[u8]) -> ([i16; 4], Vec<(u8, Vec<i16>)>) {
    let i16_at = |at: usize| i16::from_le_bytes([record[at], record[at + 1]]);
    assert_eq!(record[0], PATH_VERSION);
    assert_eq!(record[1], 0, "reserved");
    let count = u16::from_le_bytes([record[2], record[3]]);
    let bbox = [i16_at(4), i16_at(6), i16_at(8), i16_at(10)];
    let mut at = PATH_HEADER_LEN;
    let mut commands = Vec::new();
    for _ in 0..count {
        let verb = record[at];
        let n = match verb {
            VERB_MOVE | VERB_LINE => 2,
            VERB_QUAD => 4,
            VERB_CUBIC => 6,
            VERB_CLOSE => 0,
            other => panic!("verb {other}"),
        };
        commands.push((verb, (0..n).map(|k| i16_at(at + 1 + 2 * k)).collect()));
        at += 1 + 2 * n;
    }
    assert_eq!(at, record.len(), "the record ends after its last command");
    (bbox, commands)
}

#[test]
fn an_outline_is_the_fonts_own_contour_in_font_units() {
    let bytes = support::face_bytes(false, false);
    let face = ttf_parser::Face::parse(bytes, 0).expect("parses");
    let a = face.glyph_index('A').expect("A").0;
    let (bbox, commands) = read(&glyph_path(bytes, a).expect("path"));
    let expected = face
        .glyph_bounding_box(ttf_parser::GlyphId(a))
        .expect("bbox");
    assert_eq!(
        bbox,
        [
            expected.x_min,
            expected.y_min,
            expected.x_max,
            expected.y_max
        ]
    );
    assert_eq!(commands[0].0, VERB_MOVE);
    assert_eq!(commands.last().map(|c| c.0), Some(VERB_CLOSE));
    for (verb, coords) in &commands {
        for pair in coords.chunks(2) {
            assert!(
                (bbox[0]..=bbox[2]).contains(&pair[0]) && (bbox[1]..=bbox[3]).contains(&pair[1]),
                "verb {verb} point {pair:?} outside {bbox:?}"
            );
        }
    }
    let o = face.glyph_index('o').expect("o").0;
    let closes = read(&glyph_path(bytes, o).expect("path"))
        .1
        .iter()
        .filter(|c| c.0 == VERB_CLOSE)
        .count();
    assert_eq!(closes, 2, "an o is two contours");
}

#[test]
fn a_space_has_no_outline_and_an_unknown_glyph_is_an_error() {
    let bytes = support::face_bytes(true, false);
    let face = ttf_parser::Face::parse(bytes, 0).expect("parses");
    let space = face.glyph_index(' ').expect("space").0;
    let (bbox, commands) = read(&glyph_path(bytes, space).expect("path"));
    assert_eq!((bbox, commands.len()), ([0; 4], 0));
    let past = face.number_of_glyphs();
    assert_eq!(glyph_path(bytes, past), Err(GlyphError::NoGlyph(past)));
    assert_eq!(glyph_path(b"not a font", 1), Err(GlyphError::Unreadable));
}

#[test]
fn metrics_are_the_faces_and_serialise_little_endian() {
    let bytes = support::face_bytes(false, true);
    let face = ttf_parser::Face::parse(bytes, 0).expect("parses");
    let m = face_metrics(bytes).expect("metrics");
    assert_eq!(
        (m.units_per_em, m.ascender, m.descender, m.line_gap),
        (
            face.units_per_em(),
            face.ascender(),
            face.descender(),
            face.line_gap()
        )
    );
    assert!(m.ascender > 0 && m.descender < 0);
    let record = m.to_bytes();
    assert_eq!(u16::from_le_bytes([record[0], record[1]]), m.units_per_em);
    assert_eq!(i16::from_le_bytes([record[4], record[5]]), m.descender);
}
