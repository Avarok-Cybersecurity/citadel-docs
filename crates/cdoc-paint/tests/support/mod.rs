//! Shared by the display-list tests and the golden emitter: the kitchen sink laid out with the
//! shipped faces, its image, a hand-built list that uses every opcode, and proptest strategies
//! for arbitrary lists.

#![allow(dead_code, reason = "each test binary uses a different part")]

mod every;
pub mod strategies;

#[allow(unused_imports, reason = "each test binary uses a different part")]
pub use every::every_op;

use cdoc_layout::{layout, FontLibrary, Layout, LayoutOptions};
use cdoc_model::{ContentId, Document, ImageLibrary};

pub fn fonts() -> FontLibrary {
    cdoc_layout::test_support::default_fonts()
}

pub fn images() -> ImageLibrary {
    let mut lib = ImageLibrary::empty();
    lib.add(
        ContentId::new("img-1"),
        "image/jpeg",
        include_bytes!("../../../cdoc-pdf/tests/data/photo.jpg").to_vec(),
    );
    lib
}

pub fn laid_out(doc: &Document) -> Layout {
    layout(
        doc,
        &fonts(),
        &LayoutOptions::word_like(cdoc_layout::BreakerChoice::KnuthPlass(
            cdoc_layout::Params::word_like(),
        )),
    )
    .expect("lays out")
}

/// A generated document made layable: Word's built-in style sheet (which sets a font and a
/// size) in place of the random one, and every style reference cleared, since a random id
/// names no style in it. Content, direct formatting and structure are kept.
pub fn layable(mut doc: Document) -> Document {
    doc.styles = cdoc_model::StyleSheet::word_builtin();
    doc.for_each_table_mut(&mut |t| t.properties.style = None);
    doc.for_each_paragraph_mut(&mut |p| {
        p.properties.style = None;
        for inline in &mut p.inlines {
            if let cdoc_model::InlineContent::Text(run) = &mut inline.content {
                run.properties.style = None;
            }
        }
    });
    doc
}

pub fn kitchen() -> Layout {
    laid_out(&cdoc_model::test_support::fixtures::kitchen_sink())
}

/// The bytes of a Carlito face, by style.
pub fn face_bytes(bold: bool, italic: bool) -> &'static [u8] {
    cdoc_layout::test_support::FACES
        .iter()
        .find(|(b, i, _)| *b == bold && *i == italic)
        .expect("the shipped set has every style")
        .2
}

/// FNV-1a, 64-bit: a checksum with no dependency, for the golden files.
pub fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xCBF2_9CE4_8422_2325, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x0000_0100_0000_01B3)
    })
}
