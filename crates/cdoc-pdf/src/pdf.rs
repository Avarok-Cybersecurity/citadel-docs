// Adapted from rMazing crates/cc-export/src/backends/pdf.rs
// Copyright (c) Thomas Braun. Relicensed MIT OR Apache-2.0 by the copyright holder, 2026-10-04.

//! A display list, as a PDF: every run in an embedded subset face, images as `XObject`s,
//! vector drawings as paths, links as URI annotations, signature fields as unsigned `/Sig`
//! form fields at their rectangles, the information dictionary, the document language and an
//! outline from the headings. Everything is read from the list (`cdoc-paint`), the same
//! description the canvas and the SVG preview paint; the layout IR is never consulted.

use crate::meta::{allowed_link, info, outline, pdf_date};
use crate::ops::{alpha_name, draw, translucency, Names};
use crate::ports::{ClockPort, ImagePort};
use crate::scan::scan;
use crate::{font, image, PdfError};
use cdoc_layout::ir::to_pt;
use cdoc_layout::{FontPort, Pt100, Rect as ListRect};
use cdoc_paint::{DisplayList, Op, PageOps};
use pdf_writer::types::{ActionType, AnnotationFlags, AnnotationType, FieldType};
use pdf_writer::{Content, Filter, Finish, Name, Pdf, Rect, Ref, Str, TextStr};
use std::collections::{BTreeMap, BTreeSet};

fn rect(r: ListRect, height: Pt100) -> Rect {
    Rect::new(
        to_pt(r.x),
        to_pt(height - r.y - r.height),
        to_pt(r.x + r.width),
        to_pt(height - r.y),
    )
}

/// Refuses a partial list (pages missing or out of order): a PDF has every page.
fn check_full(pages: &[PageOps]) -> Result<(), PdfError> {
    match pages
        .iter()
        .enumerate()
        .find(|(slot, p)| usize::try_from(p.number).ok() != Some(*slot))
    {
        Some((_, p)) => Err(PdfError::PartialList(p.number)),
        None => Ok(()),
    }
}

/// What a page asks of the annotation layer, collected while its content is drawn.
#[derive(Default)]
struct Areas<'a> {
    links: Vec<(ListRect, &'a str)>,
    fields: Vec<(ListRect, &'a str, &'a str)>,
}

/// Writes one page's annotations (links, then signature fields) and returns their objects;
/// the fields are also pushed onto `fields`, for the form.
fn annotations(
    pdf: &mut Pdf,
    alloc: &mut Ref,
    areas: &Areas<'_>,
    (id, height): (Ref, Pt100),
    fields: &mut Vec<Ref>,
) -> Vec<Ref> {
    let mut out = Vec::new();
    for &(r, href) in &areas.links {
        let a = alloc.bump();
        let mut annot = pdf.annotation(a);
        annot.subtype(AnnotationType::Link).rect(rect(r, height));
        annot.border(0.0, 0.0, 0.0, None);
        annot
            .action()
            .action_type(ActionType::Uri)
            .uri(Str(href.as_bytes()));
        annot.finish();
        out.push(a);
    }
    for &(r, name, role) in &areas.fields {
        let f = alloc.bump();
        let mut field = pdf.form_field(f);
        field.partial_name(TextStr(name));
        field.alternate_name(TextStr(role));
        field.field_type(FieldType::Signature);
        let mut widget = field.into_annotation();
        widget.rect(rect(r, height)).page(id);
        widget.flags(AnnotationFlags::PRINT);
        widget.finish();
        out.push(f);
        fields.push(f);
    }
    out
}

/// A page's resources: every embedded face, every image, and a graphics state per alpha the
/// page paints at.
fn resources(
    resources: &mut pdf_writer::writers::Resources<'_>,
    names: &Names<'_>,
    images: &BTreeMap<String, Ref>,
    alphas: BTreeSet<u8>,
) {
    let mut font_dict = resources.fonts();
    for (face, embedded) in names.fonts {
        font_dict.pair(Name(format!("F{face}").as_bytes()), embedded.reference);
    }
    font_dict.finish();
    let mut xobjects = resources.x_objects();
    for (name, xobject) in images {
        xobjects.pair(Name(name.as_bytes()), *xobject);
    }
    xobjects.finish();
    if !alphas.is_empty() {
        let mut states = resources.ext_g_states();
        for a in alphas {
            let alpha = f32::from(a) / 255.0;
            states
                .insert(Name(alpha_name(a).as_bytes()))
                .start::<pdf_writer::writers::ExtGraphicsState>()
                .non_stroking_alpha(alpha)
                .stroking_alpha(alpha);
        }
    }
}

/// Writes a full display list as a PDF file.
pub fn render(
    list: &DisplayList,
    fonts: &dyn FontPort,
    images: &dyn ImagePort,
    clock: &dyn ClockPort,
) -> Result<Vec<u8>, PdfError> {
    let pages = &list.pages;
    check_full(pages)?;
    let now = clock.now_rfc3339();
    let date = pdf_date(&now).ok_or(PdfError::Clock(now))?;
    let mut pdf = Pdf::new();
    let mut alloc = Ref::new(1);
    let catalog = alloc.bump();
    let tree = alloc.bump();
    let info_id = alloc.bump();
    let page_ids: Vec<Ref> = pages.iter().map(|_| alloc.bump()).collect();
    let scanned = scan(pages);
    let faces = font::embed(&mut pdf, &mut alloc, &list.faces, scanned.faces, fonts)?;
    let placed = image::place(&mut pdf, &mut alloc, &scanned.images, images)?;
    let names = Names {
        fonts: &faces,
        images: &placed.names,
    };
    let mut fields = Vec::new();
    for (page, id) in pages.iter().zip(&page_ids) {
        let mut content = Content::new();
        let mut alphas = BTreeSet::new();
        let mut areas = Areas::default();
        for op in &page.ops {
            match op {
                Op::Link { rect, href } if allowed_link(href) => {
                    areas.links.push((*rect, href));
                }
                Op::SigField { rect, id, role } => areas.fields.push((*rect, id, role)),
                _ => {}
            }
            alphas.extend(translucency(op));
            draw(&mut content, op, page.height, &names);
        }
        let stream = miniz_oxide::deflate::compress_to_vec_zlib(&content.finish(), 6);
        let content_id = alloc.bump();
        pdf.stream(content_id, &stream).filter(Filter::FlateDecode);
        let annotations = annotations(
            &mut pdf,
            &mut alloc,
            &areas,
            (*id, page.height),
            &mut fields,
        );
        let mut sheet = pdf.page(*id);
        sheet
            .parent(tree)
            .media_box(Rect::new(0.0, 0.0, to_pt(page.width), to_pt(page.height)))
            .contents(content_id);
        if !annotations.is_empty() {
            sheet.annotations(annotations);
        }
        resources(&mut sheet.resources(), &names, &placed.refs, alphas);
        sheet.finish();
    }
    pdf.pages(tree)
        .kids(page_ids.iter().copied())
        .count(i32::try_from(page_ids.len()).unwrap_or(i32::MAX));
    let doc = &list.document;
    let heights: Vec<(Ref, Pt100)> = page_ids
        .iter()
        .zip(pages)
        .map(|(id, p)| (*id, p.height))
        .collect();
    let outline_root = outline(&mut pdf, &mut alloc, doc, &heights)?;
    info(&mut pdf, info_id, doc, Some(date));
    let mut cat = pdf.catalog(catalog);
    cat.pages(tree);
    if let Some(root) = outline_root {
        cat.outlines(root);
    }
    if let Some(lang) = &doc.language {
        cat.lang(TextStr(lang));
    }
    if !fields.is_empty() {
        cat.form().fields(fields);
    }
    cat.finish();
    pdf.set_file_id(scanned.file_id);
    Ok(pdf.finish())
}
