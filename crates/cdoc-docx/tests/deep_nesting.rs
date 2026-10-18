//! A document nested as deep as the read limit allows reads on a 1 MiB stack, the WASM
//! default, even in a debug build; one level deeper is an XML error, not a stack overflow.

mod common;

use cdoc_docx::{read_docx, DocxError, MemoryMedia, ReadLimits};

const TYPES: &str =
    "<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"/>";
const RELS: &str = "<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
    <Relationship Id=\"r\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" \
    Target=\"word/document.xml\"/></Relationships>";
const WASM_STACK: usize = 1 << 20;

/// `w:document`, `w:body`, `levels` wrappers of `open`/`close`, then a paragraph of "deep".
fn docx(open: &str, close: &str, levels: usize) -> Vec<u8> {
    let body = format!(
        "<w:document xmlns:w=\"x\"><w:body>{}<w:p><w:r><w:t>deep</w:t></w:r></w:p>{}</w:body></w:document>",
        open.repeat(levels),
        close.repeat(levels)
    );
    common::package(&[
        ("[Content_Types].xml", TYPES),
        ("_rels/.rels", RELS),
        ("word/document.xml", &body),
    ])
}

fn read_on_a_small_stack(bytes: Vec<u8>) -> Result<String, DocxError> {
    std::thread::Builder::new()
        .stack_size(WASM_STACK)
        .spawn(move || {
            read_docx(
                &bytes,
                &ReadLimits::generous(),
                &mut MemoryMedia::with_prefix("i"),
            )
            .map(|read| format!("{:?}", read.document))
        })
        .expect("a thread")
        .join()
        .expect("no stack overflow")
}

fn check(open: &str, close: &str, per_level: usize) {
    let max = ReadLimits::generous().max_depth;
    let levels = (max - 5) / per_level;
    let body = read_on_a_small_stack(docx(open, close, levels)).expect("at the limit");
    assert!(body.contains("deep"), "the innermost text is read");
    let past = (max - 5) / per_level + 1;
    assert!(matches!(
        read_on_a_small_stack(docx(open, close, past)),
        Err(DocxError::Xml { .. })
    ));
}

#[test]
fn tables_nested_to_the_limit_read_on_a_wasm_sized_stack() {
    check("<w:tbl><w:tr><w:tc>", "</w:tc></w:tr></w:tbl>", 3);
}

#[test]
fn content_controls_nested_to_the_limit_read_on_a_wasm_sized_stack() {
    check("<w:sdt><w:sdtContent>", "</w:sdtContent></w:sdt>", 2);
}
