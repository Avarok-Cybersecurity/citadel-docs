//! The objects an update adds: annotation dictionaries, and the form entry new signature
//! fields join.

use super::splice::{self, append_refs, insert, replace, Spliced};
use super::{Annotation, AnnotationKind, Update, UpdateError};
use crate::read::PdfFile;
use hayro_syntax::object::{Dict, MaybeRef, ObjectIdentifier};
use pdf_writer::types::{AnnotationFlags, AnnotationIcon, AnnotationType, FieldType};
use pdf_writer::{Chunk, Content, Finish, Rect, Ref, TextStr};

/// Yellow, for a note's appearance.
const NOTE_FILL: (f32, f32, f32) = (1.0, 0.85, 0.2);

/// A normal appearance stream (a form `XObject` the size of the rectangle), so a viewer that
/// draws only appearances (hayro, many mobile viewers) shows the annotation too.
fn appearance(
    update: &mut Update,
    width: f32,
    height: f32,
    draw: impl FnOnce(&mut Content),
) -> Ref {
    let id = update.alloc();
    let mut content = Content::new();
    draw(&mut content);
    let bytes = content.finish();
    let mut chunk = Chunk::new();
    chunk
        .form_xobject(id, &bytes)
        .bbox(Rect::new(0.0, 0.0, width.max(0.0), height.max(0.0)));
    update.chunk(id, &chunk);
    id
}

pub(super) fn write_annotation(
    update: &mut Update,
    a: &Annotation,
    page: Ref,
    date: pdf_writer::Date,
) -> Ref {
    let [x0, y0, x1, y1] = a.rect;
    let rect = Rect::new(x0, y0, x1, y1);
    let (w, h) = (x1 - x0, y1 - y0);
    let unit = |c: u8| f32::from(c) / 255.0;
    let look = match &a.kind {
        AnnotationKind::Note { .. } => Some(appearance(update, w, h, |c| {
            c.set_fill_rgb(NOTE_FILL.0, NOTE_FILL.1, NOTE_FILL.2);
            c.set_stroke_rgb(0.4, 0.3, 0.0);
            c.rect(0.5, 0.5, w - 1.0, h - 1.0);
            c.fill_nonzero_and_stroke();
        })),
        AnnotationKind::Square { colour, width } => Some(appearance(update, w, h, |c| {
            c.set_stroke_rgb(unit(colour.r), unit(colour.g), unit(colour.b));
            c.set_line_width(*width);
            c.rect(width / 2.0, width / 2.0, w - width, h - width);
            c.stroke();
        })),
        AnnotationKind::Signature { .. } => Some(appearance(update, w, h, |_| {})),
    };
    let id = update.alloc();
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
            if let Some(ap) = look {
                annot.appearance().normal().stream(ap);
            }
            annot.finish();
        }
        AnnotationKind::Square { colour, width } => {
            let mut annot = chunk.annotation(id);
            annot
                .subtype(AnnotationType::Square)
                .rect(rect)
                .page(page)
                .modified(date);
            annot.color_rgb(unit(colour.r), unit(colour.g), unit(colour.b));
            annot.border_style().width(*width);
            annot.flags(AnnotationFlags::PRINT);
            if let Some(ap) = look {
                annot.appearance().normal().stream(ap);
            }
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
            if let Some(ap) = look {
                widget.appearance().normal().stream(ap);
            }
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
