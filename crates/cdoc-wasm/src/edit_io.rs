//! Files in and out of an editing session: the live replica exported through the same
//! writers and options as the ProseMirror exports, and a session opened on an imported file
//! with the import report. Byte level, tested natively; `edit_bindings.rs` adds the
//! wasm-bindgen signatures.

use crate::convert::{
    document_md, document_to_docx, document_to_odt, document_txt, json, md_read_options, options,
    problems, txt_read_options,
};
use crate::edit_facade::{text, EditSession, SealedFonts};
use cdoc_crdt::ClientId;
use cdoc_docx::{read_docx, Imported, Kind, MemoryMedia, ReadLimits};
use cdoc_edit::Editor;
use cdoc_model::{ContentId, Document, ImageBytes, ImageError, ImagePort, Report};
use cdoc_odt::read_odt;
#[cfg(feature = "pdf")]
use cdoc_pdf::{render_pdf, unrendered, FixedClock};
use serde::Deserialize;

/// Options of a format that has none: `{}`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NoOptions {}

#[cfg(feature = "pdf")]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PdfOptions {
    /// The time the file records, RFC 3339.
    now: String,
}

/// The editor's own images first, then the caller's.
struct Images<'a>(&'a dyn ImagePort, &'a dyn ImagePort);

impl ImagePort for Images<'_> {
    fn image(&self, id: &ContentId) -> Result<ImageBytes<'_>, ImageError> {
        self.0.image(id).or_else(|_| self.1.image(id))
    }
}

impl EditSession {
    /// The document as a file of `kind` and the loss report as JSON. Kinds and options:
    /// `docx` and `odt` take `{"kind": "document" | "template"}`, `md` and `txt` take `{}`,
    /// `pdf` takes `{"now": <RFC 3339>}` and draws the editor's own layout (its report
    /// names each image drawn as a placeholder). Image bytes come from the images added in this session, then
    /// from `media`.
    pub fn export(
        &self,
        kind: &str,
        options_json: &str,
        media: &MemoryMedia,
    ) -> Result<(Vec<u8>, String), String> {
        let doc = self.0.model();
        let images = Images(self.0.images(), media);
        match kind {
            "docx" => document_to_docx(doc, &images, options_json),
            "odt" => document_to_odt(doc, &images, options_json),
            "md" | "txt" => {
                let _: NoOptions = options(options_json, kind)?;
                let (text, report) = if kind == "md" {
                    document_md(doc)
                } else {
                    document_txt(doc)
                };
                Ok((text.into_bytes(), json(&report)))
            }
            #[cfg(not(feature = "pdf"))]
            "pdf" => {
                Err("this artefact has no PDF writer: export pdf from the pdf artefact".to_owned())
            }
            #[cfg(feature = "pdf")]
            "pdf" => {
                let o: PdfOptions = options(options_json, kind)?;
                let layout = self.0.layout();
                let bytes = render_pdf(layout, self.0.fonts(), &images, &FixedClock(o.now))
                    .map_err(|e| e.to_string())?;
                let mut report = Report::empty();
                for why in unrendered(layout, &images)
                    .map_err(|e| e.to_string())?
                    .values()
                {
                    report.note("pdf", why);
                }
                Ok((bytes, json(&report)))
            }
            other => Err(format!(
                "cannot export {other:?}: docx, odt, md, txt or pdf"
            )),
        }
    }

    /// A new document from a file of `kind` as client `client`, and `{"report", "problems",
    /// "kind"}` as JSON. Kinds and options: `docx` (also `.dotx`, `.docm`, `.dotm`) and `odt`
    /// take `{}`, `md` takes `{"textWidth", "imageWidth", "imageHeight"}`, `txt` takes
    /// `{"singleNewline": "break" | "space"}`; text must be UTF-8. Embedded images of at
    /// most `max_image_bytes` each are held by their content ids; one of a type the editor
    /// does not draw is kept, drawn as a placeholder and named in the report.
    pub fn from_document(
        bytes: &[u8],
        kind: &str,
        options_json: &str,
        client: u64,
        fonts: SealedFonts,
        max_image_bytes: usize,
    ) -> Result<(Self, String), String> {
        let mut media = crate::convert::new_image_store();
        let mut read = read(bytes, kind, options_json, &mut media)?;
        let files = media
            .ids()
            .iter()
            .map(|id| {
                let image = media.image(id).map_err(|e| e.to_string())?;
                Ok((id.clone(), image.mime.to_owned(), image.bytes.to_vec()))
            })
            .collect::<Result<Vec<_>, String>>()?;
        let (library, layout) = fonts.0;
        let editor = Editor::from_imported(
            read.document,
            files,
            max_image_bytes,
            &mut read.report,
            ClientId(client),
            library,
            layout,
        )
        .map_err(|e| text(&e))?;
        let report = format!(
            "{{\"report\":{},\"problems\":{},\"kind\":{}}}",
            json(&read.report),
            json(&problems(editor.model())),
            json(&read.kind)
        );
        Ok((Self(editor), report))
    }
}

fn utf8<'a>(bytes: &'a [u8], kind: &str) -> Result<&'a str, String> {
    std::str::from_utf8(bytes).map_err(|e| format!("the {kind} file is not UTF-8: {e}"))
}

fn plain(document: Document, report: Report) -> Imported {
    Imported {
        document,
        report,
        kind: Kind::Document,
    }
}

fn read(
    bytes: &[u8],
    kind: &str,
    options_json: &str,
    media: &mut MemoryMedia,
) -> Result<Imported, String> {
    match kind {
        "docx" | "odt" => {
            let _: NoOptions = options(options_json, kind)?;
            if kind == "docx" {
                read_docx(bytes, &ReadLimits::generous(), media).map_err(|e| e.to_string())
            } else {
                read_odt(bytes, &ReadLimits::generous(), media).map_err(|e| e.to_string())
            }
        }
        "md" => {
            let (doc, report) =
                cdoc_md::md_to_document(utf8(bytes, kind)?, &md_read_options(options_json)?);
            Ok(plain(doc, report))
        }
        "txt" => {
            let (doc, report) =
                cdoc_txt::txt_to_document(utf8(bytes, kind)?, &txt_read_options(options_json)?);
            Ok(plain(doc, report))
        }
        other => Err(format!("cannot open {other:?}: docx, odt, md or txt")),
    }
}
