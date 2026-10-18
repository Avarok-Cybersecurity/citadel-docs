//! An ODF picture whose type is not a carried image type (here SVG, which can script) is held
//! as an opaque file under `application/octet-stream`, and the report names the type.

mod common;

use cdoc_model::image_probe::OPAQUE_FILE;
use cdoc_model::ports::ImagePort;

#[test]
fn an_svg_picture_is_held_as_an_opaque_file_and_reported() {
    let body =
        "<text:p><draw:frame text:anchor-type=\"as-char\" svg:width=\"2in\" svg:height=\"1in\">\
        <draw:image xlink:href=\"Pictures/a.svg\"/></draw:frame></text:p>";
    let bytes = common::odt("", body, "");
    let mut parts: Vec<(String, String)> = common::entries(&bytes)
        .into_iter()
        .map(|(n, _, d)| (n, String::from_utf8_lossy(&d).into_owned()))
        .collect();
    parts.push((
        "Pictures/a.svg".to_owned(),
        "<svg xmlns='http://www.w3.org/2000/svg'/>".to_owned(),
    ));
    let bytes = common::package(
        &parts
            .iter()
            .map(|(n, t)| (n.as_str(), t.as_str()))
            .collect::<Vec<_>>(),
    );
    let (_, report, media) = common::read_foreign(&bytes);
    let [id] = media.ids() else {
        panic!("one file held: {:?}", media.ids());
    };
    assert_eq!(media.image(id).expect("held").mime, OPAQUE_FILE);
    assert!(report.has(
        |_| true,
        |w| w == "image type \"image/svg+xml\" kept as an opaque file, not rendered"
    ));
}
