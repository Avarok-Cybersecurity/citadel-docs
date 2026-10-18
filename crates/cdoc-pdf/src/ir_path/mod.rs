// Adapted from rMazing crates/cc-export/src/backends/pdf.rs
// Copyright (c) Thomas Braun. Relicensed MIT OR Apache-2.0 by the copyright holder, 2026-10-04.

//! The IR→PDF path exactly as main had it, kept only until the display-list path is proven
//! to write the same bytes; removed in the commit after.
//!
//! The layout, as a PDF: every run in an embedded subset face, images as `XObject`s, vector
//! drawings as paths, links as URI annotations, signature fields as unsigned `/Sig` form
//! fields at their laid-out rectangles, the information dictionary, the document language and
//! an outline from the headings.

mod font;
mod image;
mod meta;
mod ops;
mod text;

use crate::meta::pdf_date;
use crate::ports::{ClockPort, ImagePort};
use crate::PdfError;
use cdoc_layout::ir::to_pt;
use cdoc_layout::{FontPort, Layout, Rect as IrRect};
use meta::{allowed_link, info, outline};
use ops::{draw, Names};
use pdf_writer::types::{ActionType, AnnotationFlags, AnnotationType, FieldType};
use pdf_writer::{Content, Filter, Finish, Name, Pdf, Rect, Ref, Str, TextStr};

fn rect(r: IrRect, height: i32) -> Rect {
    Rect::new(
        to_pt(r.x),
        to_pt(height - r.y - r.height),
        to_pt(r.x + r.width),
        to_pt(height - r.y),
    )
}

/// Writes the layout as a PDF file.
pub fn render_pdf_from_ir(
    layout: &Layout,
    fonts: &dyn FontPort,
    images: &dyn ImagePort,
    clock: &dyn ClockPort,
) -> Result<Vec<u8>, PdfError> {
    let now = clock.now_rfc3339();
    let date = pdf_date(&now).ok_or(PdfError::Clock(now))?;
    let mut pdf = Pdf::new();
    let mut alloc = Ref::new(1);
    let catalog = alloc.bump();
    let tree = alloc.bump();
    let info_id = alloc.bump();
    let page_ids: Vec<Ref> = layout.pages.iter().map(|_| alloc.bump()).collect();
    let faces = font::embed(&mut pdf, &mut alloc, layout, fonts)?;
    let placed = image::place(&mut pdf, &mut alloc, layout, images)?;
    let names = Names {
        fonts: &faces,
        images: &placed.names,
    };
    let mut fields = Vec::new();
    for (index, (page, id)) in layout.pages.iter().zip(&page_ids).enumerate() {
        let mut content = Content::new();
        for d in &page.draws {
            draw(&mut content, d, page.height, &names);
        }
        let stream = miniz_oxide::deflate::compress_to_vec_zlib(&content.finish(), 6);
        let content_id = alloc.bump();
        pdf.stream(content_id, &stream).filter(Filter::FlateDecode);
        let mut annotations = Vec::new();
        for link in page.links.iter().filter(|l| allowed_link(&l.href)) {
            let a = alloc.bump();
            let mut annot = pdf.annotation(a);
            annot
                .subtype(AnnotationType::Link)
                .rect(rect(link.rect, page.height));
            annot.border(0.0, 0.0, 0.0, None);
            annot
                .action()
                .action_type(ActionType::Uri)
                .uri(Str(link.href.as_bytes()));
            annot.finish();
            annotations.push(a);
        }
        let page_number = u32::try_from(index).unwrap_or(u32::MAX);
        for sig in layout.signatures.iter().filter(|s| s.page == page_number) {
            let f = alloc.bump();
            let mut field = pdf.form_field(f);
            field.partial_name(TextStr(sig.id.as_str()));
            field.alternate_name(TextStr(&sig.signer_role));
            field.field_type(FieldType::Signature);
            let mut widget = field.into_annotation();
            widget.rect(rect(sig.rect, page.height)).page(*id);
            widget.flags(AnnotationFlags::PRINT);
            widget.finish();
            annotations.push(f);
            fields.push(f);
        }
        let mut sheet = pdf.page(*id);
        sheet
            .parent(tree)
            .media_box(Rect::new(0.0, 0.0, to_pt(page.width), to_pt(page.height)))
            .contents(content_id);
        if !annotations.is_empty() {
            sheet.annotations(annotations);
        }
        let mut resources = sheet.resources();
        let mut font_dict = resources.fonts();
        for (face, embedded) in &faces {
            font_dict.pair(Name(format!("F{face}").as_bytes()), embedded.reference);
        }
        font_dict.finish();
        let mut xobjects = resources.x_objects();
        for (name, xobject) in &placed.refs {
            xobjects.pair(Name(name.as_bytes()), *xobject);
        }
        xobjects.finish();
        resources.finish();
        sheet.finish();
    }
    pdf.pages(tree)
        .kids(page_ids.iter().copied())
        .count(i32::try_from(page_ids.len()).unwrap_or(i32::MAX));
    let outline_root = outline(&mut pdf, &mut alloc, layout, &page_ids);
    info(&mut pdf, info_id, layout, Some(date));
    let mut cat = pdf.catalog(catalog);
    cat.pages(tree);
    if let Some(root) = outline_root {
        cat.outlines(root);
    }
    if let Some(lang) = &layout.language {
        cat.lang(TextStr(lang));
    }
    if !fields.is_empty() {
        cat.form().fields(fields);
    }
    cat.finish();
    pdf.set_file_id((
        file_id(layout, b"cdoc-original"),
        file_id(layout, b"cdoc-revision"),
    ));
    Ok(pdf.finish())
}

/// A file identifier derived from the layout (no randomness): the same document gives the same
/// id, which is what an incremental update later keeps.
fn file_id(layout: &Layout, salt: &[u8]) -> Vec<u8> {
    fn eat(h: &mut u64, bytes: &[u8]) {
        for b in bytes {
            *h = (*h ^ u64::from(*b)).wrapping_mul(0x0000_0100_0000_01B3);
        }
    }
    let mut h: u64 = 0xCBF2_9CE4_8422_2325;
    eat(&mut h, salt);
    for page in &layout.pages {
        for run in page.runs() {
            eat(&mut h, run.text.as_bytes());
            eat(&mut h, &run.x.to_be_bytes());
            eat(&mut h, &run.y.to_be_bytes());
        }
    }
    let first = h;
    eat(&mut h, b"second half");
    [first.to_be_bytes(), h.to_be_bytes()].concat()
}
