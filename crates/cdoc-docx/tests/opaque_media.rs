//! An embedded file whose declared type is not a carried image type (SVG, which can script,
//! or anything else) is held as an opaque file under `application/octet-stream`, and the
//! report names the type; a carried type keeps its own.

mod common;

use cdoc_model::image_probe::OPAQUE_FILE;
use cdoc_model::ports::ImagePort;

fn picture(target: &str) -> Vec<u8> {
    let body = "<w:p><w:r><w:drawing><wp:inline><wp:extent cx=\"127000\" cy=\"127000\"/>\
        <a:graphic><a:graphicData uri=\"x\"><pic:pic><pic:blipFill><a:blip r:embed=\"rId9\"/>\
        </pic:blipFill></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>";
    let rel = format!(
        "<Relationship Id=\"rId9\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/image\" Target=\"{target}\"/>"
    );
    let part = format!("word/{target}");
    common::word_package(
        body,
        &rel,
        &[(&part, "<svg xmlns='http://www.w3.org/2000/svg'/>")],
    )
}

fn held(target: &str) -> (String, Vec<String>) {
    let bytes = picture(target);
    let (_, report, media) = common::read_foreign(&bytes);
    let [id] = media.ids() else {
        panic!("one file held: {:?}", media.ids());
    };
    let mime = media.image(id).expect("held").mime.to_owned();
    let losses = report
        .losses
        .iter()
        .filter_map(|l| l.what())
        .map(str::to_owned)
        .collect();
    (mime, losses)
}

#[test]
fn an_svg_is_held_as_an_opaque_file_and_reported() {
    let (mime, losses) = held("media/p.svg");
    assert_eq!(mime, OPAQUE_FILE);
    assert!(
        losses
            .iter()
            .any(|l| l == "image type \"image/svg+xml\" kept as an opaque file, not rendered"),
        "{losses:?}"
    );
}

#[test]
fn a_carried_type_keeps_its_own() {
    let (mime, losses) = held("media/p.gif");
    assert_eq!(mime, "image/gif");
    assert!(!losses.iter().any(|l| l.contains("opaque")), "{losses:?}");
}

#[test]
fn carried_types_match_case_insensitively_and_nothing_else_does() {
    use cdoc_model::image_probe::carried_mime;
    assert_eq!(carried_mime("IMAGE/PNG"), Some("image/png"));
    assert_eq!(carried_mime("image/x-emf"), Some("image/x-emf"));
    for other in ["image/svg+xml", "text/html", "image/png; x=1", "", "image/"] {
        assert_eq!(carried_mime(other), None, "{other}");
    }
}
