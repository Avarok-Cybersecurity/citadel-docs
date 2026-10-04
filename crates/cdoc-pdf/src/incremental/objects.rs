//! The objects an update adds: annotation dictionaries, and the form entry new signature
//! fields join.

use super::splice::{self, append_refs, insert, replace, Spliced};
use super::{Annotation, AnnotationKind, Update, UpdateError};
use crate::read::PdfFile;
use hayro_syntax::object::{Dict, MaybeRef, ObjectIdentifier};
use pdf_writer::types::{AnnotationFlags, AnnotationIcon, AnnotationType, FieldType};
use pdf_writer::{Chunk, Finish, Rect, Ref, TextStr};

pub(super) fn write_annotation(
    update: &mut Update,
    a: &Annotation,
    page: Ref,
    date: pdf_writer::Date,
) -> Ref {
    let id = update.alloc();
    let [x0, y0, x1, y1] = a.rect;
    let rect = Rect::new(x0, y0, x1, y1);
    let mut chunk = Chunk::new();
    match &a.kind {
        AnnotationKind::Note { author, contents } => {
            let mut annot = chunk.annotation(id);
            annot.subtype(AnnotationType::Text).rect(rect).page(page);
            annot
                .contents(TextStr(contents))
                .author(TextStr(author))
                .modified(date);
            annot
                .icon(AnnotationIcon::Comment)
                .flags(AnnotationFlags::PRINT);
            annot.finish();
        }
        AnnotationKind::Square { colour, width } => {
            let unit = |c: u8| f32::from(c) / 255.0;
            let mut annot = chunk.annotation(id);
            annot
                .subtype(AnnotationType::Square)
                .rect(rect)
                .page(page)
                .modified(date);
            annot.color_rgb(unit(colour.r), unit(colour.g), unit(colour.b));
            annot.border_style().width(*width);
            annot.flags(AnnotationFlags::PRINT);
            annot.finish();
        }
        AnnotationKind::Signature { name, signer_role } => {
            let mut field = chunk.form_field(id);
            field
                .partial_name(TextStr(name))
                .alternate_name(TextStr(signer_role));
            field.field_type(FieldType::Signature);
            let mut widget = field.into_annotation();
            widget.rect(rect).page(page).flags(AnnotationFlags::PRINT);
            widget.finish();
        }
    }
    update.chunk(id, &chunk);
    id
}

pub(super) fn page_ref(id: ObjectIdentifier) -> Ref {
    Ref::new(id.obj_number)
}

/// Adds the new signature fields to the catalog's form.
pub(super) fn add_fields(
    update: &mut Update,
    file: &PdfFile,
    root: ObjectIdentifier,
    fields: &[Ref],
) -> Result<(), UpdateError> {
    let xref = file.pdf().xref();
    let catalog = xref.get::<Dict<'_>>(root).ok_or(UpdateError::BadTrailer)?;
    match catalog.get_raw::<Dict<'_>>("AcroForm") {
        None => {
            let form = update.alloc();
            let body = format!("<</Fields [{}]>>", splice::ref_list(fields).trim_start());
            update.raw(ObjectIdentifier::new(form.get(), 0), body.as_bytes())?;
            let new = insert(catalog.data(), "AcroForm", &format!("{} 0 R", form.get()))?;
            update.raw(root, &new)
        }
        Some(MaybeRef::Ref(r)) => {
            let id: ObjectIdentifier = r.into();
            let form = xref
                .get::<Dict<'_>>(id)
                .ok_or(UpdateError::Unsupported("an unreadable form"))?;
            let spliced = append_refs(&form, xref, "Fields", fields)?;
            update.spliced(id, spliced)
        }
        Some(MaybeRef::NotRef(form)) => match append_refs(&form, xref, "Fields", fields)? {
            Spliced::Inline(bytes) => {
                update.raw(root, &replace(catalog.data(), form.data(), &bytes)?)
            }
            referenced @ Spliced::Referenced(..) => update.spliced(root, referenced),
        },
    }
}
