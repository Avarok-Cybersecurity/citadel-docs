//! Conversions to and from other formats, as plain functions over strings and bytes so they
//! are tested natively. A converted document crosses the boundary as ProseMirror JSON, the
//! editor's own shape; the report of what the format could not carry and the model's
//! validation problems travel beside it.
//!
//! Options are JSON with every field required and unknown fields rejected: there are no
//! defaults to forget about.

use cdoc_docx::{read_docx, write_docx, Imported, Kind, MemoryMedia, ReadLimits, WriteOptions};
use cdoc_md::{document_to_md, md_to_document, MdReadOptions};
use cdoc_model::{Document, ImagePort, Length, Report};
use cdoc_odt::{read_odt, write_odt};
use cdoc_pm::{from_pm_json, to_pm_json};
use cdoc_txt::{document_to_txt, txt_to_document, SingleNewline, TxtReadOptions};
use serde::Deserialize;

/// Prefix of the provisional content ids the reader gives embedded images; the caller moves
/// the bytes to the content store and rewrites the ids.
const IMAGE_PREFIX: &str = "docx-image-";

/// An empty image store: where `docx_to_pm` / `odt_to_pm` put the images they find and
/// `pm_to_docx` / `pm_to_odt` read the ones a document names.
#[must_use]
pub fn new_image_store() -> MemoryMedia {
    MemoryMedia::with_prefix(IMAGE_PREFIX)
}

pub(crate) fn json<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_string(value).expect("reports and strings serialise")
}

pub(crate) fn problems(doc: &Document) -> Vec<String> {
    doc.validate()
        .err()
        .map(|errors| errors.iter().map(ToString::to_string).collect())
        .unwrap_or_default()
}

/// `{"pm": <ProseMirror>, "report": {...}, "problems": [..]}`.
fn imported(doc: &Document, report: &Report) -> String {
    format!(
        "{{\"pm\":{},\"report\":{},\"problems\":{}}}",
        to_pm_json(doc),
        json(report),
        json(&problems(doc))
    )
}

/// A package import: `{"pm", "report", "problems", "kind"}`, the kind being `"document"` or
/// `"template"`.
fn imported_package(read: &Imported) -> String {
    format!(
        "{{\"pm\":{},\"report\":{},\"problems\":{},\"kind\":{}}}",
        to_pm_json(&read.document),
        json(&read.report),
        json(&problems(&read.document)),
        json(&read.kind)
    )
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct PackageOptions {
    kind: FileKind,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
enum FileKind {
    Document,
    Template,
}

/// Package export options: `{"kind": "document" | "template"}`.
pub(crate) fn package_options(options_json: &str) -> Result<WriteOptions, String> {
    let o: PackageOptions = options(options_json, "package")?;
    Ok(WriteOptions {
        kind: match o.kind {
            FileKind::Document => Kind::Document,
            FileKind::Template => Kind::Template,
        },
    })
}

fn document_from_pm(pm_json: &str) -> Result<Document, String> {
    from_pm_json(pm_json).map_err(|e| e.to_string())
}

/// A `.docx`, `.dotx`, `.docm` or `.dotm` to `{"pm", "report", "problems", "kind"}`. Embedded
/// images are added to `media` under provisional ids; macros are refused and reported.
pub fn docx_to_pm(bytes: &[u8], media: &mut MemoryMedia) -> Result<String, String> {
    let read = read_docx(bytes, &ReadLimits::generous(), media).map_err(|e| e.to_string())?;
    Ok(imported_package(&read))
}

/// A `.odt` or `.ott` to `{"pm", "report", "problems", "kind"}`. Embedded images are added
/// to `media` under provisional ids.
pub fn odt_to_pm(bytes: &[u8], media: &mut MemoryMedia) -> Result<String, String> {
    let read = read_odt(bytes, &ReadLimits::generous(), media).map_err(|e| e.to_string())?;
    Ok(imported_package(&read))
}

/// ProseMirror JSON to `.odt` (or `.ott`) bytes and the report as JSON. Image bytes come
/// from `media`. Options: `{"kind": "document" | "template"}`.
pub fn pm_to_odt(
    pm_json: &str,
    media: &MemoryMedia,
    options_json: &str,
) -> Result<(Vec<u8>, String), String> {
    document_to_odt(&document_from_pm(pm_json)?, media, options_json)
}

/// [`pm_to_odt`] from a model document.
pub(crate) fn document_to_odt(
    doc: &Document,
    media: &dyn ImagePort,
    options_json: &str,
) -> Result<(Vec<u8>, String), String> {
    let options = package_options(options_json)?;
    let (bytes, report) = write_odt(doc, media, options).map_err(|e| e.to_string())?;
    Ok((bytes, json(&report)))
}

/// ProseMirror JSON to `.docx` (or `.dotx`) bytes and the report as JSON. Image bytes come
/// from `media`. Options: `{"kind": "document" | "template"}`.
pub fn pm_to_docx(
    pm_json: &str,
    media: &MemoryMedia,
    options_json: &str,
) -> Result<(Vec<u8>, String), String> {
    document_to_docx(&document_from_pm(pm_json)?, media, options_json)
}

/// [`pm_to_docx`] from a model document.
pub(crate) fn document_to_docx(
    doc: &Document,
    media: &dyn ImagePort,
    options_json: &str,
) -> Result<(Vec<u8>, String), String> {
    let options = package_options(options_json)?;
    let (bytes, report) = write_docx(doc, media, options).map_err(|e| e.to_string())?;
    Ok((bytes, json(&report)))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct MdOptions {
    text_width: i32,
    image_width: i32,
    image_height: i32,
}

pub(crate) fn options<'a, T: Deserialize<'a>>(text: &'a str, what: &str) -> Result<T, String> {
    serde_json::from_str(text).map_err(|e| format!("bad {what} options: {e}"))
}

/// Markdown to `{"pm", "report", "problems"}`. Options: `{"textWidth", "imageWidth",
/// "imageHeight"}` in hundredths of a point.
pub fn md_to_pm(md: &str, options_json: &str) -> Result<String, String> {
    let (doc, report) = md_to_document(md, &md_read_options(options_json)?);
    Ok(imported(&doc, &report))
}

/// Markdown read options from `{"textWidth", "imageWidth", "imageHeight"}`.
pub(crate) fn md_read_options(options_json: &str) -> Result<MdReadOptions, String> {
    let o: MdOptions = options(options_json, "markdown")?;
    Ok(MdReadOptions {
        text_width: Length(o.text_width),
        image_width: Length(o.image_width),
        image_height: Length(o.image_height),
    })
}

/// ProseMirror JSON to `{"text", "report"}`.
pub fn pm_to_md(pm_json: &str) -> Result<String, String> {
    let (text, report) = document_to_md(&document_from_pm(pm_json)?);
    Ok(format!(
        "{{\"text\":{},\"report\":{}}}",
        json(&text),
        json(&report)
    ))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct TxtOptions {
    single_newline: Newline,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
enum Newline {
    Break,
    Space,
}

/// Plain text to `{"pm", "report", "problems"}`. Options: `{"singleNewline": "break" |
/// "space"}`.
pub fn txt_to_pm(text: &str, options_json: &str) -> Result<String, String> {
    let (doc, report) = txt_to_document(text, &txt_read_options(options_json)?);
    Ok(imported(&doc, &report))
}

/// Plain-text read options from `{"singleNewline": "break" | "space"}`.
pub(crate) fn txt_read_options(options_json: &str) -> Result<TxtReadOptions, String> {
    let o: TxtOptions = options(options_json, "text")?;
    Ok(TxtReadOptions {
        single_newline: match o.single_newline {
            Newline::Break => SingleNewline::Break,
            Newline::Space => SingleNewline::Space,
        },
    })
}

/// ProseMirror JSON to `{"text", "report"}`.
pub fn pm_to_txt(pm_json: &str) -> Result<String, String> {
    let (text, report) = document_to_txt(&document_from_pm(pm_json)?);
    Ok(format!(
        "{{\"text\":{},\"report\":{}}}",
        json(&text),
        json(&report)
    ))
}
