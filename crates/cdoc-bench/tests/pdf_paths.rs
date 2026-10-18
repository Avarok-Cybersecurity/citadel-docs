//! The display-list PDF is the IR PDF, byte for byte, for every document there is a fixture
//! for: the kitchen sink, the benchmark corpus, its committed `.docx` and Markdown, and the
//! `.docx` and `.odt` reader fixtures. Transitional: it holds the IR path to account until
//! that path is removed.

use cdoc_bench::corpus::generate;
use cdoc_layout::test_support::default_fonts;
use cdoc_layout::{layout, BreakerChoice, LayoutOptions, Params};
use cdoc_model::ports::ImagePort;
use cdoc_model::Document;
use cdoc_pdf::{render_pdf, FixedClock};

const NOW: &str = "2026-01-01T00:00:00Z";

fn same(name: &str, doc: &Document, images: &dyn ImagePort) {
    let fonts = default_fonts();
    let options = LayoutOptions::word_like(BreakerChoice::KnuthPlass(Params::word_like()));
    let laid = layout(doc, &fonts, &options).expect("lays out");
    let clock = FixedClock(NOW.to_owned());
    let list = render_pdf(&laid, &fonts, images, &clock).expect("the list path renders");
    let ir = cdoc_pdf::ir_path::render_pdf_from_ir(&laid, &fonts, images, &clock)
        .expect("the IR path renders");
    assert!(
        list == ir,
        "{name}: the display-list PDF ({} bytes) differs from the IR PDF ({} bytes)",
        list.len(),
        ir.len()
    );
}

fn fixture(path: &str) -> Vec<u8> {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/..");
    std::fs::read(format!("{root}/{path}")).unwrap_or_else(|e| panic!("{path}: {e}"))
}

#[test]
fn the_kitchen_sink_and_the_generated_corpus_write_the_same_bytes() {
    let none = no_images();
    same(
        "kitchen sink",
        &cdoc_model::test_support::fixtures::kitchen_sink(),
        &kitchen_images(),
    );
    for pages in [1, 10, 100] {
        same(&format!("{pages}p"), &generate(pages), &none);
    }
}

#[test]
#[ignore = "release-only: `cargo test --release -p cdoc-bench --test pdf_paths -- --ignored`"]
fn the_500_page_corpus_writes_the_same_bytes() {
    same("500p", &generate(500), &no_images());
}

#[test]
fn docx_markdown_and_odt_fixtures_write_the_same_bytes() {
    for path in [
        "cdoc-bench/corpus/1p.docx",
        "cdoc-bench/corpus/10p.docx",
        "cdoc-docx/tests/fixtures/pandoc.docx",
        "cdoc-docx/tests/fixtures/textutil.docx",
    ] {
        let bytes = fixture(path);
        let mut media = cdoc_odt::MemoryMedia::with_prefix("img");
        let doc = cdoc_docx::read_docx(&bytes, &cdoc_docx::ReadLimits::generous(), &mut media)
            .expect("reads")
            .document;
        same(path, &doc, &media);
    }
    for path in ["cdoc-bench/corpus/1p.md", "cdoc-bench/corpus/10p.md"] {
        let text = String::from_utf8(fixture(path)).expect("utf-8");
        let options = cdoc_md::MdReadOptions {
            text_width: cdoc_model::Length::from_twips(9_026),
            image_width: cdoc_model::Length::from_twips(4_320),
            image_height: cdoc_model::Length::from_twips(2_880),
        };
        let doc = cdoc_md::md_to_document(&text, &options);
        same(path, &doc.0, &no_images());
    }
    for name in [
        "_annotation",
        "bullet_list",
        "changeTracked",
        "HeaderFooter",
        "TestTextTable",
    ] {
        let path = format!("cdoc-odt/tests/fixtures/{name}.odt");
        let bytes = fixture(&path);
        let mut media = cdoc_odt::MemoryMedia::with_prefix("img");
        let doc = cdoc_odt::read_odt(&bytes, &cdoc_odt::ReadLimits::generous(), &mut media)
            .expect("reads")
            .document;
        same(&path, &doc, &media);
    }
}

fn no_images() -> cdoc_model::ports::ImageLibrary {
    cdoc_model::ports::ImageLibrary::empty()
}

fn kitchen_images() -> cdoc_model::ports::ImageLibrary {
    let mut lib = cdoc_model::ports::ImageLibrary::empty();
    let data = concat!(env!("CARGO_MANIFEST_DIR"), "/../cdoc-pdf/tests/data");
    for (id, mime, file) in [
        ("img-1", "image/jpeg", "photo.jpg"),
        ("alpha", "image/png", "alpha.png"),
        ("opaque", "image/png", "opaque.png"),
    ] {
        let bytes = std::fs::read(format!("{data}/{file}")).expect("image");
        lib.add(cdoc_model::ContentId::new(id), mime, bytes);
    }
    lib
}
