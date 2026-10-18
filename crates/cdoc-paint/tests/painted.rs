//! The compact glyph-run layout changed only the bytes, not what is painted: the SVG of the
//! kitchen sink and of a ten-page document is pinned to the checksums version 3 (before the
//! compaction, main at e31809a) produced, page for page.

#[path = "../benches/support/hundred.rs"]
#[allow(
    dead_code,
    reason = "the test uses the document, not the 100-page layout"
)]
mod hundred;
mod support;

use cdoc_layout::{layout, BreakerChoice, Layout, LayoutOptions, Params};
use cdoc_paint::{decode, encode, lower};

fn svg_checksums(laid: &Layout) -> Vec<u64> {
    let bytes = encode(laid, &support::fonts()).expect("encodes");
    let view = decode(&bytes).expect("decodes");
    cdoc_paint::svg::render_pages(&view, &support::images())
        .expect("paints")
        .iter()
        .map(|svg| support::fnv1a(svg.as_bytes()))
        .collect()
}

fn ten_pages() -> Layout {
    let options = LayoutOptions::word_like(BreakerChoice::KnuthPlass(Params::word_like()));
    layout(&hundred::document(100), &support::fonts(), &options).expect("lays out")
}

#[test]
fn the_kitchen_sink_paints_the_same_svg_as_before_the_compaction() {
    let sums = svg_checksums(&support::kitchen());
    assert_eq!(sums, KITCHEN_SVG);
}

#[test]
fn ten_pages_of_text_paint_the_same_svg_as_before_the_compaction() {
    let laid = ten_pages();
    let all: Vec<usize> = (0..laid.pages.len()).collect();
    let list = lower(&laid, &support::fonts(), &all).expect("lowers");
    let bytes = list.to_bytes().expect("writes");
    let back = decode(&bytes).expect("decodes").to_owned().expect("pages");
    assert_eq!(
        back, list,
        "every glyph's id, cluster, advance and offsets survive"
    );
    let sums = svg_checksums(&laid);
    assert_eq!(sums, TEN_PAGES_SVG);
}

const KITCHEN_SVG: [u64; 3] = [
    0x8c93_014b_faf2_22ed,
    0xba6e_1e13_8946_e9e5,
    0xc1ab_7f79_83f1_24f6,
];

const TEN_PAGES_SVG: [u64; 10] = [
    0xc3da_bede_2ed5_1be9,
    0xd4c0_f992_3cb8_307b,
    0x2fc8_a2e3_e4c3_8982,
    0xb191_4e0a_0a35_68cf,
    0x7b2a_d0e5_189e_d723,
    0x5a26_f722_7b29_d066,
    0xd4a9_ea2b_7788_fc5a,
    0x9e55_96d2_4b2f_97fb,
    0x4699_6f0e_398c_bb3c,
    0xe4d3_8890_da9f_e84d,
];
