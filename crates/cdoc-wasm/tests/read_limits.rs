//! Package imports take their read limits from the caller: there is no built-in default, and
//! the browser-sized limits docs/edit-commands.md recommends refuse a 100 MiB part.

mod common;

use cdoc_wasm::facade;

const MIB: u32 = 1 << 20;

/// A `.docx` whose `word/document.xml` declares `size` bytes inflated in its central
/// directory entry: what the read checks before inflating anything.
fn docx_declaring(size: u32) -> Vec<u8> {
    let pm = cdoc_pm::to_pm_json(&cdoc_model::Document::blank());
    let media = facade::new_image_store();
    let (mut bytes, _) = facade::pm_to_docx(&pm, &media, r#"{"kind":"document"}"#).unwrap();
    let name = b"word/document.xml";
    let at = (0..bytes.len() - 46)
        .find(|&i| {
            bytes[i..i + 4] == [0x50, 0x4b, 0x01, 0x02]
                && bytes[i + 46..].starts_with(name)
                && usize::from(u16::from_le_bytes([bytes[i + 28], bytes[i + 29]])) == name.len()
        })
        .expect("the central directory entry");
    bytes[at + 24..at + 28].copy_from_slice(&size.to_le_bytes());
    bytes
}

fn docx_error(bytes: &[u8], options: &str) -> String {
    let mut media = facade::new_image_store();
    facade::docx_to_pm(bytes, &mut media, options).expect_err("refused")
}

#[test]
fn a_100_mib_part_is_refused_under_the_browser_limits() {
    let big = docx_declaring(100 * MIB);
    assert_eq!(
        docx_error(&big, common::BROWSER_READ),
        "package exceeds the part size limit"
    );
    let server = common::BROWSER_READ.replace("67108864", "134217728");
    assert_ne!(
        docx_error(&big, &server),
        "package exceeds the part size limit",
        "a caller allowing 128 MiB parts gets past the part size check"
    );
}

#[test]
fn the_limits_are_required_and_exact() {
    let bytes = docx_declaring(64 * MIB);
    for options in [
        "{}".to_owned(),
        r#"{"limits":{"maxParts":5000}}"#.to_owned(),
        common::BROWSER_READ.replace("\"maxDepth\"", "\"maxDepth\":1,\"extra\""),
    ] {
        assert!(
            docx_error(&bytes, &options).starts_with("bad package read options"),
            "{options}"
        );
    }
}
