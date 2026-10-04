//! Appending an incremental update to an existing PDF: annotations and unsigned signature
//! fields added without touching a byte of the original, which is what a signature over the
//! earlier revision needs. The original is a prefix of the result.
//!
//! This is the foundation of the annotate and sign layer; the signing itself lives in the
//! signing crate.

mod objects;
mod splice;
mod tail;
mod xref;

use crate::meta::pdf_date;
use crate::ports::ClockPort;
use crate::read::{PdfFile, ReadError};
use cdoc_model::Color;
use hayro_syntax::object::ObjectIdentifier;
use objects::{add_fields, page_ref, write_annotation};
use pdf_writer::{Chunk, Ref};
use splice::{append_refs, Spliced};

/// An annotation to add, on a zero-based page, at `rect` (`[x0, y0, x1, y1]` in PDF user
/// space, points, y up).
#[derive(Debug, Clone, PartialEq)]
pub struct Annotation {
    /// Zero-based page.
    pub page: usize,
    /// Where.
    pub rect: [f32; 4],
    /// What.
    pub kind: AnnotationKind,
}

/// What an annotation is.
#[derive(Debug, Clone, PartialEq)]
pub enum AnnotationKind {
    /// A sticky note.
    Note {
        /// Who wrote it.
        author: String,
        /// The text.
        contents: String,
    },
    /// A rectangle outline.
    Square {
        /// Its colour.
        colour: Color,
        /// Its line width in points.
        width: f32,
    },
    /// An unsigned signature field, for the signing track to fill.
    Signature {
        /// The field name.
        name: String,
        /// Who is meant to sign it.
        signer_role: String,
    },
}

/// Why an update could not be appended.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum UpdateError {
    /// The file did not parse.
    #[error(transparent)]
    Read(#[from] ReadError),
    /// Encrypted files are refused: new objects would have to be encrypted too.
    #[error("the PDF is encrypted")]
    Encrypted,
    /// No `startxref` at the end of the file.
    #[error("no startxref at the end of the file")]
    NoStartXref,
    /// The last trailer could not be read.
    #[error("the trailer could not be read")]
    BadTrailer,
    /// An annotation names a page the file does not have.
    #[error("the file has no page {0}")]
    NoPage(usize),
    /// A structure this writer does not handle yet.
    #[error("unsupported structure: {0}")]
    Unsupported(&'static str),
    /// The clock port's time is not RFC 3339.
    #[error("the clock gave {0:?}, which is not an RFC 3339 time")]
    Clock(String),
}

/// The update being assembled: bytes after the original and their xref entries.
struct Update {
    out: Vec<u8>,
    entries: Vec<(i32, usize)>,
    next: i32,
}

impl Update {
    fn alloc(&mut self) -> Ref {
        let r = Ref::new(self.next);
        self.next += 1;
        r
    }

    fn chunk(&mut self, id: Ref, chunk: &Chunk) {
        self.entries.push((id.get(), self.out.len()));
        self.out.extend_from_slice(chunk.as_bytes());
    }

    fn raw(&mut self, id: ObjectIdentifier, body: &[u8]) -> Result<(), UpdateError> {
        if id.gen_number != 0 {
            return Err(UpdateError::Unsupported("an object of a later generation"));
        }
        self.entries.push((id.obj_number, self.out.len()));
        self.out
            .extend_from_slice(format!("{} 0 obj\n", id.obj_number).as_bytes());
        self.out.extend_from_slice(body);
        self.out.extend_from_slice(b"\nendobj\n");
        Ok(())
    }

    fn spliced(&mut self, owner: ObjectIdentifier, spliced: Spliced) -> Result<(), UpdateError> {
        match spliced {
            Spliced::Inline(bytes) => self.raw(owner, &bytes),
            Spliced::Referenced(id, bytes) => self.raw(id, &bytes),
        }
    }
}

/// `original` with an incremental update adding `annotations` appended.
pub fn append_annotations(
    original: &[u8],
    annotations: &[Annotation],
    clock: &dyn ClockPort,
) -> Result<Vec<u8>, UpdateError> {
    let now = clock.now_rfc3339();
    let date = pdf_date(&now).ok_or(UpdateError::Clock(now))?;
    let file = PdfFile::parse(original)?;
    let tail = tail::read(original)?;
    if tail.encrypted {
        return Err(UpdateError::Encrypted);
    }
    let pages = file.pdf().pages();
    if let Some(a) = annotations.iter().find(|a| a.page >= pages.len()) {
        return Err(UpdateError::NoPage(a.page));
    }
    let mut update = Update {
        out: original.to_vec(),
        entries: Vec::new(),
        next: tail.size,
    };
    if !update.out.ends_with(b"\n") {
        update.out.push(b'\n');
    }
    let mut fields = Vec::new();
    for (index, page) in pages.iter().enumerate() {
        let mine: Vec<&Annotation> = annotations.iter().filter(|a| a.page == index).collect();
        if mine.is_empty() {
            continue;
        }
        let page_id = page
            .raw()
            .obj_id()
            .ok_or(UpdateError::Unsupported("a direct page"))?;
        let refs: Vec<Ref> = mine
            .iter()
            .map(|a| {
                let r = write_annotation(&mut update, a, page_ref(page_id), date);
                if matches!(a.kind, AnnotationKind::Signature { .. }) {
                    fields.push(r);
                }
                r
            })
            .collect();
        let spliced = append_refs(page.raw(), file.pdf().xref(), "Annots", &refs)?;
        update.spliced(page_id, spliced)?;
    }
    if !fields.is_empty() {
        add_fields(&mut update, &file, tail.root.into(), &fields)?;
    }
    let next = update.next;
    xref::finish(&mut update.out, &mut update.entries, &tail, next);
    Ok(update.out)
}
