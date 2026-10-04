//! Writes the kitchen sink as a PDF and SVG pages to the directory given, for looking at.

use cdoc_layout::{layout, LayoutOptions};
use cdoc_model::ContentId;
use cdoc_pdf::{render_pdf, svg::pages_svg, FixedClock, ImageLibrary};

fn main() {
    let out = std::env::args().nth(1).expect("an output directory");
    let fonts = cdoc_layout::test_support::default_fonts();
    let mut images = ImageLibrary::empty();
    images.add(
        ContentId::new("img-1"),
        "image/jpeg",
        include_bytes!("../tests/data/photo.jpg").to_vec(),
    );
    let doc = cdoc_model::test_support::fixtures::kitchen_sink();
    let laid = layout(&doc, &fonts, &LayoutOptions::word_like()).expect("lays out");
    let pdf = render_pdf(
        &laid,
        &fonts,
        &images,
        &FixedClock("2026-10-04T12:00:00Z".into()),
    )
    .expect("renders");
    std::fs::write(format!("{out}/kitchen.pdf"), pdf).expect("writes");
    for (i, svg) in pages_svg(&laid, &images).expect("svg").iter().enumerate() {
        std::fs::write(format!("{out}/kitchen-{i}.svg"), svg).expect("writes");
    }
}
