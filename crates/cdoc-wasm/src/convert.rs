//! Conversions to and from other formats, as plain functions over strings and bytes so they
//! are tested natively. A converted document crosses the boundary as ProseMirror JSON, the
//! editor's own shape; the report of what the format could not carry and the model's
//! validation problems travel beside it.
//!
//! Options are JSON with every field required and unknown fields rejected: there are no
//! defaults to forget about.

use cdoc_docx::{read_docx, write_docx, MemoryMedia, ReadLimits};
use cdoc_md::{document_to_md, md_to_document, MdReadOptions};
use cdoc_model::{Document, Length, Report};
use cdoc_pm::{from_pm_json, to_pm_json};
use cdoc_txt::{document_to_txt, txt_to_document, SingleNewline, TxtReadOptions};
use serde::Deserialize;

/// Prefix of the provisional content ids the reader gives embedded images; the caller moves
/// the bytes to the content store and rewrites the ids.
const IMAGE_PREFIX: &str = "docx-image-";

/// An empty image store: where `docx_to_pm` puts the images it finds and `pm_to_docx` reads
/// the ones a document names.
#[must_use]
pub fn new_image_store() -> MemoryMedia {
    MemoryMedia::with_prefix(IMAGE_PREFIX)
}

fn json<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_string(value).expect("reports and strings serialise")
}

fn problems(doc: &Document) -> Vec<String> {
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

fn document_from_pm(pm_json: &str) -> Result<Document, String> {
    from_pm_json(pm_json).map_err(|e| e.to_string())
}

/// A `.docx` to `{"pm", "report", "problems"}`. Embedded images are added to `media` under
/// provisional ids.
pub fn docx_to_pm(bytes: &[u8], media: &mut MemoryMedia) -> Result<String, String> {
    let (doc, report) =
        read_docx(bytes, &ReadLimits::generous(), media).map_err(|e| e.to_string())?;
    Ok(imported(&doc, &report))
}

/// ProseMirror JSON to `.docx` bytes and the report as JSON. Image bytes come from `media`.
pub fn pm_to_docx(pm_json: &str, media: &MemoryMedia) -> Result<(Vec<u8>, String), String> {
    let doc = document_from_pm(pm_json)?;
    let (bytes, report) = write_docx(&doc, media).map_err(|e| e.to_string())?;
    Ok((bytes, json(&report)))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct MdOptions {
    text_width: i32,
    image_width: i32,
    image_height: i32,
}

fn options<'a, T: Deserialize<'a>>(text: &'a str, what: &str) -> Result<T, String> {
    serde_json::from_str(text).map_err(|e| format!("bad {what} options: {e}"))
}

/// Markdown to `{"pm", "report", "problems"}`. Options: `{"textWidth", "imageWidth",
/// "imageHeight"}` in hundredths of a point.
pub fn md_to_pm(md: &str, options_json: &str) -> Result<String, String> {
    let o: MdOptions = options(options_json, "markdown")?;
    let opts = MdReadOptions {
        text_width: Length(o.text_width),
        image_width: Length(o.image_width),
        image_height: Length(o.image_height),
    };
    let (doc, report) = md_to_document(md, &opts);
    Ok(imported(&doc, &report))
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
    let o: TxtOptions = options(options_json, "text")?;
    let opts = TxtReadOptions {
        single_newline: match o.single_newline {
            Newline::Break => SingleNewline::Break,
            Newline::Space => SingleNewline::Space,
        },
    };
    let (doc, report) = txt_to_document(text, &opts);
    Ok(imported(&doc, &report))
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
