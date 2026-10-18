//! Write the kitchen-sink document to a `.docx` so other programs (Word, `LibreOffice`, pandoc)
//! can open it: `cargo run -p cdoc-docx --example kitchen_sink_docx -- out.docx`.

use cdoc_docx::{write_docx, MemoryMedia, WriteOptions};
use cdoc_model::test_support::fixtures::kitchen_sink;
use cdoc_model::ContentId;

/// An 8x4 red PNG.
const PNG: &[u8] = include_bytes!("../tests/fixtures/pixel.png");

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage: kitchen_sink_docx <out.docx>");
    let mut media = MemoryMedia::with_prefix("unused-");
    media.insert(ContentId::new("img-1"), "image/png", PNG.to_vec());
    let (bytes, report) =
        write_docx(&kitchen_sink(), &media, WriteOptions::document()).expect("kitchen sink writes");
    assert!(report.is_lossless(), "{report:?}");
    std::fs::write(&path, bytes).expect("output is writable");
    println!("wrote {path}");
}
