//! No byte flipped anywhere in a display list makes the reader panic: every byte of the golden
//! lists flipped several ways, and random lists flipped at random, are decoded and every page
//! and glyph visited. A reader of corrupt bytes gets an error or fewer glyphs, never a crash.

mod support;

use cdoc_paint::{decode, OpView};
use proptest::prelude::*;

fn visit_all(bytes: &[u8]) -> usize {
    let Ok(view) = decode(bytes) else { return 0 };
    let mut seen = 0;
    for slot in 0..view.page_count() {
        if let Ok(page) = view.page(slot) {
            for op in page.ops() {
                if let OpView::GlyphRun { glyphs, text, .. } = op {
                    for g in glyphs {
                        seen += usize::from(text.is_char_boundary(g.cluster as usize));
                    }
                }
            }
        }
    }
    seen
}

#[test]
fn no_byte_of_the_golden_lists_flipped_makes_the_reader_panic() {
    let every = support::every_op().to_bytes().expect("writes");
    let kitchen = cdoc_paint::encode(&support::kitchen(), &support::fonts()).expect("encodes");
    for good in [every, kitchen] {
        for at in 0..good.len() {
            for xor in [0x01, 0x02, 0x40, 0x7F, 0x80, 0xFF] {
                let mut bytes = good.clone();
                bytes[at] ^= xor;
                let _ = visit_all(&bytes);
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 512, ..ProptestConfig::default() })]

    #[test]
    fn no_flipped_byte_makes_the_reader_panic(
        list in support::strategies::display_list(),
        flips in prop::collection::vec((any::<prop::sample::Index>(), 1..=255_u8), 1..4),
    ) {
        let mut bytes = list.to_bytes().expect("writes");
        for (at, xor) in flips {
            let k = at.index(bytes.len());
            bytes[k] ^= xor;
        }
        let _ = visit_all(&bytes);
    }
}
