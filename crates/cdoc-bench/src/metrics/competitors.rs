//! In-process competitors (feature `competitors`, CI only): `docx-rs` opening the same `.docx`
//! fixtures, rustybuzz shaping the same text with the same features. Each run is written in the
//! competitor-file shape every runner shares, with its non-comparable cells said honestly.

use super::text::{shape_params, SHAPE_FACE};
use super::{paged, BenchError, Ctx, Family, Fixture, SHAPE_CHARS, SHAPE_LATIN};
use crate::corpus::latin_text;
use crate::schema::{CompetitorResult, CompetitorRun};
use cdoc_shape::{Feature, HARFBUZZ_DEFAULTS_NOT_IMPLEMENTED};
use std::collections::BTreeMap;
use std::hint::black_box;

/// The pinned `docx-rs` version (Cargo.toml pins the same with `=`).
pub const DOCX_RS_VERSION: &str = "0.4.22";
/// The pinned rustybuzz version.
pub const RUSTYBUZZ_VERSION: &str = "0.20.1";

/// `docx-rs` on every fixture: `docx_open` measured; the rest it cannot do.
pub fn docx_rs(ctx: &Ctx<'_>, fixtures: &[Fixture]) -> Result<CompetitorRun, BenchError> {
    let mut results = BTreeMap::new();
    for fx in fixtures {
        let key = paged(Family::DocxOpen, fx.pages);
        let t = ctx.time(&format!("docx-rs/{key}"), || {
            let doc =
                docx_rs::read_docx(black_box(&fx.docx)).map_err(|e| BenchError::Competitor {
                    tool: "docx-rs",
                    message: e.to_string(),
                })?;
            black_box(doc);
            Ok(())
        })?;
        results.insert(key, CompetitorResult::Measured(t));
        for family in [Family::PdfExport, Family::DocxToPdf, Family::LayoutFull] {
            results.insert(
                paged(family, fx.pages),
                CompetitorResult::NotComparable {
                    reason: "docx-rs has no layout engine and no PDF export".to_owned(),
                },
            );
        }
    }
    Ok(CompetitorRun {
        tool: "docx-rs".to_owned(),
        version: DOCX_RS_VERSION.to_owned(),
        results,
    })
}

fn rb_features() -> Vec<rustybuzz::Feature> {
    let features = shape_params().features;
    let tag = rustybuzz::ttf_parser::Tag::from_bytes;
    HARFBUZZ_DEFAULTS_NOT_IMPLEMENTED
        .iter()
        .map(|t| rustybuzz::Feature::new(tag(t), 0, ..))
        .chain(Feature::ALL.iter().map(|f| {
            let on = u32::from(features.contains(*f));
            rustybuzz::Feature::new(tag(&f.tag().to_bytes()), on, ..)
        }))
        .collect()
}

/// rustybuzz shaping the shaping metric's text with a cached plan and a reused buffer.
pub fn rustybuzz(ctx: &Ctx<'_>) -> Result<CompetitorRun, BenchError> {
    let text = latin_text(SHAPE_CHARS);
    let face = rustybuzz::Face::from_slice(SHAPE_FACE, 0).ok_or(BenchError::Competitor {
        tool: "rustybuzz",
        message: "Carlito did not parse".to_owned(),
    })?;
    let plan = rustybuzz::ShapePlan::new(
        &face,
        rustybuzz::Direction::LeftToRight,
        Some(rustybuzz::script::LATIN),
        None,
        &rb_features(),
    );
    let mut spare = Some(rustybuzz::UnicodeBuffer::new());
    let t = ctx.time(&format!("rustybuzz/{SHAPE_LATIN}"), || {
        let mut buffer = spare.take().unwrap_or_default();
        buffer.push_str(black_box(&text));
        buffer.set_direction(rustybuzz::Direction::LeftToRight);
        buffer.set_script(rustybuzz::script::LATIN);
        let output = rustybuzz::shape_with_plan(&face, &plan, buffer);
        black_box(output.len());
        spare = Some(output.clear());
        Ok(())
    })?;
    let mut results = BTreeMap::new();
    results.insert(SHAPE_LATIN.to_owned(), CompetitorResult::Measured(t));
    Ok(CompetitorRun {
        tool: "rustybuzz".to_owned(),
        version: RUSTYBUZZ_VERSION.to_owned(),
        results,
    })
}
