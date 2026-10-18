//! A document nested as deep as the read limit allows reads on a 1 MiB stack, the WASM
//! default, even in a debug build; one level deeper is an XML error, not a stack overflow.

mod common;

use cdoc_odt::{read_odt, MemoryMedia, OdtError, ReadLimits};

const WASM_STACK: usize = 1 << 20;

/// `office:document-content`, `office:body`, `office:text`, `levels` wrappers of
/// `open`/`close`, then a paragraph of "deep".
fn odt(open: &str, close: &str, levels: usize) -> Vec<u8> {
    let content = format!(
        "<office:document-content xmlns:office=\"o\" xmlns:text=\"t\" xmlns:table=\"b\">\
         <office:body><office:text>{}<text:p>deep</text:p>{}</office:text></office:body>\
         </office:document-content>",
        open.repeat(levels),
        close.repeat(levels)
    );
    common::package(&[
        ("mimetype", "application/vnd.oasis.opendocument.text"),
        ("content.xml", &content),
    ])
}

fn read_on_a_small_stack(bytes: Vec<u8>) -> Result<String, OdtError> {
    std::thread::Builder::new()
        .stack_size(WASM_STACK)
        .spawn(move || {
            read_odt(
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
    let doc = read_on_a_small_stack(odt(open, close, levels)).expect("at the limit");
    assert!(doc.contains("deep"), "the innermost text is read");
    let past = levels + 1 + 5 / per_level;
    assert!(matches!(
        read_on_a_small_stack(odt(open, close, past)),
        Err(OdtError::Xml { .. })
    ));
}

#[test]
fn tables_nested_to_the_limit_read_on_a_wasm_sized_stack() {
    check(
        "<table:table><table:table-row><table:table-cell>",
        "</table:table-cell></table:table-row></table:table>",
        3,
    );
}

#[test]
fn lists_and_sections_nested_to_the_limit_read_on_a_wasm_sized_stack() {
    check(
        "<text:list><text:list-item>",
        "</text:list-item></text:list>",
        2,
    );
    check("<text:section>", "</text:section>", 1);
}
