//! The compact `GLYPH_RUN` (version 5): what it costs, and what a reader refuses. A run shaped
//! like Latin text is its strike, colour, origin and text plus a few bytes; every column and
//! table a reader trusts is checked when the page is opened, by name (`flipped_bytes` checks
//! that no corrupted list makes it panic).

mod support;

use cdoc_layout::{FaceId, FaceKey};
use cdoc_paint::{
    decode, DecodeError, DisplayList, DocumentInfo, FaceEntry, Glyph, GlyphMetric, Op, OpView,
    PageOps, PaintError, Rgba, RunHead,
};

const SIZE: i32 = 1100;

/// One face whose glyph `id` is `id · 100` units wide, for `a`..`e` (ids 1..=5).
fn face() -> FaceEntry {
    FaceEntry {
        key: FaceKey {
            family: "Carlito".to_owned(),
            bold: false,
            italic: false,
        },
        units_per_em: 2048,
        glyphs: (1..=5)
            .map(|id| GlyphMetric {
                id,
                advance: id * 100,
            })
            .collect(),
    }
}

/// `text` (over `a`..`e`) set one glyph a character at its nominal advance, rounded down.
fn latin(text: &str) -> Op {
    let glyphs = (0_u32..)
        .zip(text.bytes())
        .map(|(at, b)| {
            let id = u16::from(b - b'a' + 1);
            Glyph {
                id,
                cluster: at,
                advance: i32::from(id) * 100 * SIZE / 2048,
                x_offset: 0,
                y_offset: 0,
            }
        })
        .collect();
    Op::GlyphRun {
        head: RunHead {
            face: FaceId(0),
            size: SIZE,
            colour: Rgba {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            },
            x: 0,
            y: 0,
        },
        text: text.to_owned(),
        glyphs,
    }
}

fn list(faces: Vec<FaceEntry>, ops: Vec<Op>) -> DisplayList {
    DisplayList {
        faces,
        document: DocumentInfo::default(),
        pages: vec![PageOps {
            number: 0,
            width: 1000,
            height: 1000,
            ops,
        }],
    }
}

/// The list's bytes with its one op's payload replaced by `payload`.
fn with_payload(bytes: &[u8], payload: &[u8]) -> Vec<u8> {
    let view = decode(bytes).expect("decodes");
    let body = bytes.len() - view.page(0).expect("page").body_len();
    let mut out = bytes[..body].to_vec();
    out.push(0x11);
    out.push(u8::try_from(payload.len()).expect("short payload"));
    out.extend_from_slice(payload);
    let index =
        usize::try_from(u32::from_le_bytes(bytes[20..24].try_into().expect("4"))).expect("offset");
    let len = u32::try_from(out.len() - body).expect("short");
    out[index + 8..index + 12].copy_from_slice(&len.to_le_bytes());
    out
}

#[test]
fn a_latin_run_costs_its_text_and_twelve_bytes() {
    let one = list(vec![face()], vec![latin("abcde")])
        .to_bytes()
        .expect("writes");
    let view = decode(&one).expect("decodes");
    let body = view.page(0).expect("page").body_len();
    // opcode, length, strike, rgba, x, y, text length + 5 bytes, glyph count, flags.
    assert_eq!(body, 1 + 1 + 1 + 4 + 1 + 1 + 1 + 5 + 1 + 1);
    let back = view.to_owned().expect("pages");
    assert_eq!(back.pages[0].ops, [latin("abcde")]);
}

#[test]
fn a_glyph_its_face_does_not_list_cannot_be_written() {
    let mut run = latin("ab");
    if let Op::GlyphRun { glyphs, .. } = &mut run {
        glyphs[1].id = 9;
    }
    assert_eq!(
        list(vec![face()], vec![run]).to_bytes().err(),
        Some(PaintError::NoGlyph { face: 0, id: 9 })
    );
}

#[test]
fn ascii_clusters_that_decrease_or_pass_the_text_are_refused() {
    let why = "a cluster decreases or is not a character boundary of the text";
    for (clusters, refused) in [
        ([0, 1, 2], false),
        ([0, 0, 3], false),
        ([0, 2, 1], true),
        ([1, 0, 2], true),
        ([0, 1, 4], true),
    ] {
        let mut run = latin("abc");
        if let Op::GlyphRun { glyphs, .. } = &mut run {
            for (g, c) in glyphs.iter_mut().zip(clusters) {
                g.cluster = c;
            }
        }
        let bytes = list(vec![face()], vec![run.clone()])
            .to_bytes()
            .expect("writes");
        let view = decode(&bytes).expect("tables are intact");
        let got = view.page(0).err();
        assert_eq!(
            got,
            refused.then_some(DecodeError::Malformed(why)),
            "{clusters:?}"
        );
        if !refused {
            assert_eq!(view.to_owned().expect("pages").pages[0].ops, [run]);
        }
    }
}

#[test]
fn broken_columns_are_refused_by_name() {
    let good = list(vec![face()], vec![latin("ab")])
        .to_bytes()
        .expect("writes");
    // strike 0, opaque black, x 0, y 0, text "ab", 2 glyphs, then flags and columns.
    let head = [0, 0, 0, 0, 255, 0, 0, 2, b'a', b'b', 2];
    let cases: [(&[u8], &str); 6] = [
        (&[0x40], "unknown glyph-run column flags"),
        (&[0x04, 0b100], "the advance bitmap's padding bits are set"),
        (&[0x01, 2, 0, 5], "a glyph index is past the glyph table"),
        (
            &[0x08, 2, 2, 1],
            "a column lists a glyph past the run's last",
        ),
        (&[0x10, 2, 0, 1, 0], "payload longer than its fields"),
        (
            &[0x02, 2, 1, 3],
            "a cluster decreases or is not a character boundary of the text",
        ),
    ];
    for (tail, why) in cases {
        let payload: Vec<u8> = head.iter().chain(tail).copied().collect();
        let bytes = with_payload(&good, &payload);
        let view = decode(&bytes).expect("tables are intact");
        assert_eq!(
            view.page(0).err(),
            Some(DecodeError::Malformed(why)),
            "{tail:?}"
        );
    }
    let accepted = with_payload(&good, &[&head[..], &[0x04, 0b10]].concat());
    let view = decode(&accepted).expect("decodes");
    let ops: Vec<_> = view
        .page(0)
        .expect("a set bit is a unit more")
        .ops()
        .collect();
    let OpView::GlyphRun { glyphs, .. } = &ops[0] else {
        panic!("a glyph run");
    };
    let advances: Vec<i32> = glyphs.clone().map(|g| g.advance).collect();
    assert_eq!(advances, [53, 108]);
}

#[test]
fn a_run_in_a_face_with_no_glyphs_is_refused() {
    let mut empty = face();
    empty.glyphs.clear();
    let Op::GlyphRun { head, .. } = latin("") else {
        unreachable!("latin is a run")
    };
    let nothing = Op::GlyphRun {
        head,
        text: String::new(),
        glyphs: Vec::new(),
    };
    let good = list(vec![empty, face()], vec![nothing])
        .to_bytes()
        .expect("writes");
    // Strike 0 is face 0, which has no glyphs; give its run two.
    let payload = [0, 0, 0, 0, 255, 0, 0, 2, b'a', b'b', 2, 0];
    let view = decode(&good).expect("decodes");
    assert!(view.page(0).is_ok(), "a run of no glyphs is fine");
    let bytes = with_payload(&good, &payload);
    assert_eq!(
        decode(&bytes).expect("tables are intact").page(0).err(),
        Some(DecodeError::Malformed(
            "a run in a face with no glyphs has glyphs"
        ))
    );
}

#[test]
fn every_earlier_version_is_refused_by_name() {
    let good = list(vec![face()], vec![latin("ab")])
        .to_bytes()
        .expect("writes");
    assert_eq!(cdoc_paint::format::VERSION, 5);
    for old in 1..cdoc_paint::format::VERSION {
        let mut bytes = good.clone();
        bytes[4..6].copy_from_slice(&old.to_le_bytes());
        assert_eq!(decode(&bytes).err(), Some(DecodeError::Version(old)));
    }
}
