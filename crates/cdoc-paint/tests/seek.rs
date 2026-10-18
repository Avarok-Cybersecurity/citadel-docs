//! The page index: any page can be opened without decoding the others, a buffer cut off after
//! page N still paints pages up to N, a partial list keeps page numbers and the full face
//! table, and broken input is refused by name rather than painted.

mod support;

use cdoc_paint::format::{HEADER_LEN, INDEX_ENTRY_LEN, VERSION};
use cdoc_paint::{decode, encode, encode_pages, DecodeError, Op, PaintError};

fn kitchen_bytes() -> Vec<u8> {
    encode(&support::kitchen(), &support::fonts()).expect("encodes")
}

/// Where page `slot`'s body ends, read from the index the way a TypeScript reader would.
fn body_end(bytes: &[u8], slot: usize) -> usize {
    let index = u32::from_le_bytes(bytes[20..24].try_into().expect("4 bytes")) as usize;
    let entry = &bytes[index + slot * INDEX_ENTRY_LEN..];
    let word =
        |k: usize| u32::from_le_bytes(entry[4 * k..4 * k + 4].try_into().expect("4")) as usize;
    word(1) + word(2)
}

#[test]
fn a_buffer_cut_after_page_n_still_paints_pages_up_to_n() {
    let bytes = kitchen_bytes();
    let full = decode(&bytes).expect("decodes").to_owned().expect("pages");
    let pages = full.pages.len();
    assert!(pages >= 3);
    for n in 0..pages - 1 {
        let cut = &bytes[..body_end(&bytes, n)];
        let view = decode(cut).expect("the tables precede every body");
        assert_eq!(view.page_count(), pages, "the index still lists every page");
        for slot in 0..=n {
            let page = view.page(slot).expect("a page before the cut");
            let ops: Vec<Op> = page.ops().map(|op| op.to_owned()).collect();
            assert_eq!(
                ops, full.pages[slot].ops,
                "page {slot} with the cut after {n}"
            );
        }
        assert!(
            matches!(view.page(n + 1), Err(DecodeError::Truncated(_))),
            "page {} is past the cut",
            n + 1
        );
    }
    assert!(
        matches!(decode(&bytes).expect("decodes").page(pages), Err(DecodeError::NoPage(p)) if p == pages)
    );
}

#[test]
fn a_partial_list_keeps_page_numbers_and_the_whole_face_table() {
    let laid = support::kitchen();
    let fonts = support::fonts();
    let full = decode(&kitchen_bytes())
        .expect("decodes")
        .to_owned()
        .expect("pages");
    let bytes = encode_pages(&laid, &fonts, &[2, 0]).expect("encodes");
    let partial = decode(&bytes).expect("decodes").to_owned().expect("pages");
    let keys: Vec<_> = partial.faces.iter().map(|f| f.key.clone()).collect();
    assert_eq!(keys, laid.faces);
    assert_eq!(
        partial.pages,
        vec![full.pages[2].clone(), full.pages[0].clone()]
    );
    assert_eq!(partial.pages[0].number, 2);
    assert!(bytes.len() < kitchen_bytes().len());
    assert_eq!(
        encode_pages(&laid, &fonts, &[9]),
        Err(PaintError::NoPage(9))
    );
}

#[test]
fn broken_input_is_refused_by_name() {
    let good = support::every_op().to_bytes().expect("writes");
    assert_eq!(decode(b"PDF-1.7").err(), Some(DecodeError::BadMagic));
    let mut version = good.clone();
    version[4..6].copy_from_slice(&(VERSION + 1).to_le_bytes());
    assert_eq!(
        decode(&version).err(),
        Some(DecodeError::Version(VERSION + 1))
    );
    assert!(matches!(
        decode(&good[..HEADER_LEN + 3]),
        Err(DecodeError::Truncated(_))
    ));

    let view = decode(&good).expect("decodes");
    let first = good.len() - view.page(0).expect("page").body_len();
    let mut unknown = good.clone();
    unknown[first] = 0x7F;
    let view = decode(&unknown).expect("tables are intact");
    assert_eq!(view.page(0).err(), Some(DecodeError::UnknownOp(0x7F)));

    for (ops, why) in [
        (vec![Op::ClipPop], "clip pop without a push"),
        (
            vec![Op::ClipPush(cdoc_layout::Rect {
                x: 0,
                y: 0,
                width: 1,
                height: 1,
            })],
            "clip push without a pop",
        ),
    ] {
        let mut list = support::every_op();
        list.pages[0].ops = ops;
        let bytes = list.to_bytes().expect("writes");
        let view = decode(&bytes).expect("tables are intact");
        assert_eq!(view.page(0).err(), Some(DecodeError::Malformed(why)));
    }
}

#[test]
fn a_glyph_run_whose_clusters_decrease_or_split_a_character_is_refused() {
    let why = "a cluster decreases or is not a character boundary of the text";
    // "fi é": é is bytes 3 and 4, so 4 is not a character boundary.
    for (clusters, refused) in [
        ([0, 2, 3, 3], false),
        ([0, 2, 1, 3], true),
        ([0, 2, 4, 4], true),
    ] {
        let mut list = support::every_op();
        let run = list.pages[0]
            .ops
            .iter_mut()
            .find_map(|op| match op {
                Op::GlyphRun { glyphs, .. } => Some(glyphs),
                _ => None,
            })
            .expect("every_op has a glyph run");
        for (glyph, cluster) in run.iter_mut().zip(clusters) {
            glyph.cluster = cluster;
        }
        let bytes = list.to_bytes().expect("writes");
        let view = decode(&bytes).expect("tables are intact");
        let got = view.page(0).err();
        let want = refused.then_some(DecodeError::Malformed(why));
        assert_eq!(got, want, "clusters {clusters:?}");
    }
}

#[test]
fn a_document_table_naming_a_missing_string_or_overrunning_its_entries_is_refused() {
    let mut list = support::every_op();
    list.document.language = None;
    list.document.outline.truncate(1);
    list.document.outline[0].y = 0;
    let good = list.to_bytes().expect("writes");
    decode(&good).expect("the unmutated table decodes");
    // [title + 1, language 0, count 1, level, page, y, title index], one byte each, then the index.
    let index =
        usize::try_from(u32::from_le_bytes(good[20..24].try_into().expect("4"))).expect("fits");
    let mut missing = good.clone();
    missing[index - 7] = 0x7F;
    assert_eq!(
        decode(&missing).err(),
        Some(DecodeError::Malformed("string index out of range"))
    );
    let mut overrun = good;
    overrun[index - 5] = 0;
    assert_eq!(
        decode(&overrun).err(),
        Some(DecodeError::Malformed(
            "document table longer than its entries"
        ))
    );
}
