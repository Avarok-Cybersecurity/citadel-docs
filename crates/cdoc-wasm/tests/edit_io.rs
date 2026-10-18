//! Files in and out of an editing session: a `.docx` opened as a session, exported from the
//! live replica and opened again is the same document, with its image held by content id;
//! every export kind writes, and bad kinds, options and text are refused by name.

use cdoc_layout::test_support::{ALIASES, FACES, FALLBACK};
use cdoc_model::test_support::fixtures::kitchen_sink;
use cdoc_model::{ContentId, Document, ImagePort};
use cdoc_wasm::edit_facade::{EditFonts, EditSession, SealedFonts};
use cdoc_wasm::facade;
use serde_json::{json, Value};
use std::sync::OnceLock;

const PNG: &[u8] = include_bytes!("../../cdoc-docx/tests/fixtures/pixel.png");
const DOCUMENT: &str = r#"{"kind":"document"}"#;
const LIMIT: usize = 1 << 20;

fn fonts() -> SealedFonts {
    static SEALED: OnceLock<SealedFonts> = OnceLock::new();
    *SEALED.get_or_init(|| {
        let mut f = EditFonts::new(FALLBACK);
        for (bold, italic, bytes) in FACES {
            f.add_font(FALLBACK, bold, italic, bytes);
        }
        for (name, target) in ALIASES {
            f.alias_font(name, target);
        }
        f.seal()
    })
}

fn kitchen_docx() -> Vec<u8> {
    let pm = cdoc_pm::to_pm_json(&kitchen_sink());
    let mut media = facade::new_image_store();
    media.insert(ContentId::new("img-1"), "image/png", PNG.to_vec());
    facade::pm_to_docx(&pm, &media, DOCUMENT).unwrap().0
}

fn open(bytes: &[u8], kind: &str, options: &str) -> Result<(EditSession, Value), String> {
    EditSession::from_document(bytes, kind, options, 1, fonts(), LIMIT)
        .map(|(s, report)| (s, serde_json::from_str(&report).unwrap()))
}

fn png_id() -> ContentId {
    let mut blank = EditSession::blank(9, fonts()).unwrap();
    let record = blank.add_image(PNG, LIMIT).unwrap();
    cdoc_edit::wire::decode_added_image(&record)
        .unwrap()
        .content_id
}

/// The document a `.docx` holds, through the facade's own reader.
fn docx_model(bytes: &[u8]) -> (Document, cdoc_docx::MemoryMedia) {
    let mut media = facade::new_image_store();
    let read: Value =
        serde_json::from_str(&facade::docx_to_pm(bytes, &mut media).unwrap()).unwrap();
    let pm = serde_json::to_string(&read["pm"]).unwrap();
    (
        serde_json::from_str(&facade::pm_to_model(&pm).unwrap()).unwrap(),
        media,
    )
}

#[test]
fn a_docx_opened_exported_and_reopened_is_the_same_document() {
    let (session, report) = open(&kitchen_docx(), "docx", "{}").unwrap();
    assert_eq!(
        report,
        json!({"report": {"losses": []}, "problems": [], "kind": "document"})
    );
    let empty = facade::new_image_store();
    let (exported, losses) = session.export("docx", DOCUMENT, &empty).unwrap();
    assert_eq!(losses, r#"{"losses":[]}"#);
    let (doc, media) = docx_model(&exported);
    let (original, _) = docx_model(&kitchen_docx());
    assert_eq!(doc, original, "the export holds the imported document");
    let image = media.image(&media.ids()[0]).unwrap();
    assert_eq!(
        image.bytes, PNG,
        "the image came from the session's own images"
    );
    let (again, _) = open(&exported, "docx", "{}").unwrap();
    assert_eq!(again.export("docx", DOCUMENT, &empty).unwrap().0, exported);
    let pm = again.export("md", "{}", &empty).unwrap().0;
    assert!(String::from_utf8(pm).unwrap().contains(png_id().as_str()));
}

#[test]
fn every_kind_exports_from_the_live_replica() {
    let (mut session, _) = open(b"one\n\ntwo", "txt", r#"{"singleNewline":"break"}"#).unwrap();
    session
        .request(&cdoc_edit::wire::encode_request(
            &cdoc_edit::wire::Request::Command(cdoc_edit::Command::InsertText("zero ".to_owned())),
        ))
        .unwrap();
    let none = facade::new_image_store();
    let txt = session.export("txt", "{}", &none).unwrap().0;
    assert_eq!(String::from_utf8(txt).unwrap(), "zero one\n\ntwo\n");
    let md = session.export("md", "{}", &none).unwrap().0;
    assert!(String::from_utf8(md).unwrap().starts_with("zero one"));
    let odt = session
        .export("odt", r#"{"kind":"template"}"#, &none)
        .unwrap()
        .0;
    assert_eq!(&odt[..2], b"PK");
    let (pdf, report) = session
        .export("pdf", r#"{"now":"2026-10-18T00:00:00Z"}"#, &none)
        .unwrap();
    assert_eq!(
        (&pdf[..5], report.as_str()),
        (&b"%PDF-"[..], r#"{"losses":[]}"#)
    );
}

#[test]
fn bad_kinds_options_and_text_are_refused_by_name() {
    let (session, _) = open(b"x", "txt", r#"{"singleNewline":"space"}"#).unwrap();
    let none = facade::new_image_store();
    let err = |r: Result<(Vec<u8>, String), String>| r.unwrap_err();
    assert!(err(session.export("rtf", "{}", &none)).contains("\"rtf\""));
    assert!(err(session.export("md", r#"{"x":1}"#, &none)).contains("bad md options"));
    assert!(err(session.export("pdf", "{}", &none)).contains("bad pdf options"));
    assert!(err(session.export("docx", "{}", &none)).contains("bad package options"));
    let refused = |r: Result<(EditSession, Value), String>| r.err().expect("refused");
    assert!(refused(open(b"x", "rtf", "{}")).contains("\"rtf\""));
    assert!(
        refused(open(&[0xFF, 0xFE], "txt", r#"{"singleNewline":"space"}"#)).contains("not UTF-8")
    );
    assert!(
        refused(open(&kitchen_docx(), "docx", r#"{"kind":"document"}"#))
            .contains("bad docx options")
    );
    let mut cut = kitchen_docx();
    cut.truncate(cut.len() / 2);
    assert!(refused(open(&cut, "docx", "{}")).contains("not a zip archive"));
}

#[test]
fn an_embedded_image_over_the_limit_is_refused() {
    let err = EditSession::from_document(&kitchen_docx(), "docx", "{}", 1, fonts(), PNG.len() - 1)
        .err()
        .expect("refused");
    assert!(err.contains(&PNG.len().to_string()), "{err}");
}

/// A picture the editor does not draw never stops a file opening: it is kept, exported
/// unchanged, drawn as a placeholder in the PDF and named in both reports.
#[test]
fn a_gif_opens_round_trips_and_is_reported() {
    let gif = b"GIF89a\x01\x00\x01\x00\x00\x00\x00;".to_vec();
    let pm = cdoc_pm::to_pm_json(&kitchen_sink());
    let mut media = facade::new_image_store();
    media.insert(ContentId::new("img-1"), "image/gif", gif.clone());
    let docx = facade::pm_to_docx(&pm, &media, DOCUMENT).unwrap().0;
    let (session, report) = open(&docx, "docx", "{}").unwrap();
    let lost = json!([{"part": "images", "what": "image type GIF not rendered", "count": 1}]);
    assert_eq!(report["report"]["losses"], lost);
    let none = facade::new_image_store();
    let (exported, _) = session.export("docx", DOCUMENT, &none).unwrap();
    let (_, found) = docx_model(&exported);
    let image = found.image(&found.ids()[0]).unwrap();
    assert_eq!((image.mime, image.bytes), ("image/gif", &gif[..]));
    let (pdf, report) = session
        .export("pdf", r#"{"now":"2026-10-18T00:00:00Z"}"#, &none)
        .unwrap();
    assert_eq!(&pdf[..5], b"%PDF-");
    let report: Value = serde_json::from_str(&report).unwrap();
    assert_eq!(
        report["losses"],
        json!([{"part": "pdf", "what": "image type GIF not rendered", "count": 1}])
    );
}

/// The canvas painter reads the bytes of the images the session holds by content id.
#[test]
fn the_session_serves_its_images_by_content_id() {
    use cdoc_edit::wire::{decode_added_image, decode_image, decode_image_list};
    let mut session = EditSession::blank(1, fonts()).unwrap();
    assert_eq!(decode_image_list(&session.images()).unwrap(), []);
    let id = decode_added_image(&session.add_image(PNG, LIMIT).unwrap())
        .unwrap()
        .content_id;
    let record = session.image(id.as_str()).unwrap();
    assert_eq!(&record[..4], b"CDB\x01");
    assert_eq!(
        decode_image(&record).unwrap(),
        ("image/png".to_owned(), PNG.to_vec())
    );
    for k in 0..record.len() {
        assert!(decode_image(&record[..k]).is_err(), "cut at {k}");
    }
    let list = session.images();
    assert_eq!(&list[..4], b"CDL\x01");
    assert_eq!(
        decode_image_list(&list).unwrap(),
        [(id, "image/png".to_owned(), PNG.len())]
    );
    let err = session.image("sha256-nope").unwrap_err();
    assert!(err.contains("sha256-nope"), "{err}");
}

/// A footnote whose insertion was undone keeps its body in the session (redo needs it), but
/// no file the session writes carries it; one still referenced is written.
#[test]
fn no_export_writes_a_footnote_nothing_references() {
    use cdoc_edit::wire::{encode_request, Request};
    use cdoc_edit::Command;
    use cdoc_model::FootnoteId;
    let footnotes = |session: &EditSession, kind: &str| {
        let none = facade::new_image_store();
        let bytes = session.export(kind, DOCUMENT, &none).unwrap().0;
        let mut media = facade::new_image_store();
        let read = if kind == "docx" {
            facade::docx_to_pm(&bytes, &mut media)
        } else {
            facade::odt_to_pm(&bytes, &mut media)
        };
        let read: Value = serde_json::from_str(&read.unwrap()).unwrap();
        let pm = serde_json::to_string(&read["pm"]).unwrap();
        let doc: Document = serde_json::from_str(&facade::pm_to_model(&pm).unwrap()).unwrap();
        doc.footnotes.len()
    };
    let (mut session, _) = open(b"one", "txt", r#"{"singleNewline":"break"}"#).unwrap();
    let insert = Request::Command(Command::InsertFootnote(Some(FootnoteId(7))));
    session.request(&encode_request(&insert)).unwrap();
    assert_eq!(
        (footnotes(&session, "docx"), footnotes(&session, "odt")),
        (1, 1)
    );
    session.request(&encode_request(&Request::Undo)).unwrap();
    let replica = cdoc_crdt::Doc::decode_snapshot(&session.snapshot(), cdoc_crdt::ClientId(5));
    let held = cdoc_crdt::to_model(&replica.unwrap()).unwrap();
    assert_eq!(
        held.footnotes.len(),
        1,
        "the session keeps the body for redo"
    );
    assert_eq!(
        (footnotes(&session, "docx"), footnotes(&session, "odt")),
        (0, 0)
    );
    let none = facade::new_image_store();
    let md = session.export("md", "{}", &none).unwrap().0;
    assert!(!String::from_utf8(md).unwrap().contains("[^"));
}
