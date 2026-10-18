//! Opening and saving: `.docx` in and out, Markdown in, PDF out, and `.docx` to PDF end to end.

use super::{paged, BenchError, Ctx, Family, Fixture, Results};
use crate::schema::Measurement;
use cdoc_docx::{read_docx, write_docx, MemoryMedia, ReadLimits};
use cdoc_layout::test_support::default_fonts;
use cdoc_layout::{layout, LayoutOptions};
use cdoc_md::{md_to_document, MdReadOptions};
use cdoc_model::Length;
use cdoc_pdf::{render_pdf, FixedClock};
use std::hint::black_box;

/// The PDF's recorded time: fixed, so every export writes the same bytes.
const PDF_TIME: &str = "2026-01-01T00:00:00Z";

/// What Markdown leaves unsaid, said once: the default A4 text width and a 3 in × 2 in image.
pub const MD_OPTIONS: MdReadOptions = MdReadOptions {
    text_width: Length::from_twips(9_026),
    image_width: Length::from_twips(4_320),
    image_height: Length::from_twips(2_880),
};

/// `.docx` bytes to model, as every metric that opens a `.docx` does it.
pub fn open_docx(bytes: &[u8]) -> Result<cdoc_model::Document, BenchError> {
    let mut media = MemoryMedia::with_prefix("img");
    Ok(read_docx(bytes, &ReadLimits::generous(), &mut media)?.0)
}

pub(super) fn run(ctx: &Ctx<'_>, fx: &Fixture, out: &mut Results) -> Result<(), BenchError> {
    let fonts = default_fonts();
    let options = LayoutOptions::word_like();
    let media = MemoryMedia::with_prefix("img");
    let clock = FixedClock(PDF_TIME.to_owned());

    let key = paged(Family::DocxOpen, fx.pages);
    let t = ctx.time(&key, || {
        black_box(open_docx(black_box(&fx.docx))?);
        Ok(())
    })?;
    out.insert(key, Measurement::Time(t));

    let key = paged(Family::DocxSave, fx.pages);
    let t = ctx.time(&key, || {
        black_box(write_docx(black_box(&fx.doc), &media)?);
        Ok(())
    })?;
    out.insert(key, Measurement::Time(t));

    let key = paged(Family::MdOpen, fx.pages);
    let t = ctx.time(&key, || {
        black_box(md_to_document(black_box(&fx.md), &MD_OPTIONS));
        Ok(())
    })?;
    out.insert(key, Measurement::Time(t));

    let laid_out = layout(&fx.doc, &fonts, &options)?;
    let key = paged(Family::PdfExport, fx.pages);
    let t = ctx.time(&key, || {
        black_box(render_pdf(black_box(&laid_out), &fonts, &media, &clock)?);
        Ok(())
    })?;
    out.insert(key, Measurement::Time(t));

    let key = paged(Family::DocxToPdf, fx.pages);
    let t = ctx.time(&key, || {
        let doc = open_docx(black_box(&fx.docx))?;
        let pages = layout(&doc, &fonts, &options)?;
        black_box(render_pdf(&pages, &fonts, &media, &clock)?);
        Ok(())
    })?;
    out.insert(key, Measurement::Time(t));
    Ok(())
}
