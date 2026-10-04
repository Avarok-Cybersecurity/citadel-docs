//! Citadel Docs layout: a [`cdoc_model::Document`] paginated into a page IR ([`Layout`]) that
//! the PDF and SVG backends in `cdoc-pdf` both draw, so the preview cannot disagree with the
//! file.
//!
//! Pure: faces arrive through [`FontPort`]; the crate reads no file, clock or randomness.
//!
//! What is laid out: page geometry from each section's `PageSetup` (size, orientation,
//! margins, columns); measured line breaking in the faces the style cascade resolves; spacing,
//! alignment (justify included) and indents; list labels from the model's numbering; tables
//! with spans, borders, shading and repeated header rows; inline images; page and column
//! breaks, keep-with-next, keep-lines-together and widow/orphan control; headers and footers
//! with page-number fields; footnotes at the foot of the referencing column; signature fields,
//! whose rectangles are reported for `SignatureField.anchor`.

#![forbid(unsafe_code)]

mod anchors;
mod context;
mod flow;
pub mod fonts;
mod fragment;
mod furniture;
pub mod geometry;
pub mod ir;
pub mod labels;
mod line;
mod paragraph;
mod pieces;
mod setter;
mod stack;
mod style;
pub mod svg_path;
mod table;
#[cfg(feature = "test-support")]
pub mod test_support;
mod widgets;
mod wrap;

pub use anchors::write_anchors;
pub use context::{LayoutOptions, RevisionView};
pub use fonts::{FaceSource, FontError, FontLibrary, FontPort, Fonts};
pub use ir::{
    Draw, FaceId, FaceKey, Layout, LinkArea, OutlineEntry, Page, Pt100, Rect, Segment,
    SignaturePlacement, TextRun, Warning,
};
pub use labels::list_labels;

use cdoc_model::{Document, StyleError};
use context::Ctx;
use flow::Flow;
use geometry::Geometry;

/// Why a document could not be laid out.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LayoutError {
    /// The style cascade failed (unknown style or a `basedOn` cycle).
    #[error(transparent)]
    Style(#[from] StyleError),
    /// A face was missing or unreadable.
    #[error(transparent)]
    Font(#[from] FontError),
    /// A run resolved to no font family or no size: the document defaults must set both.
    #[error(
        "a run has no font family or size after the cascade; set them in the document defaults"
    )]
    NoFont,
    /// The document has no sections.
    #[error("the document has no sections")]
    NoSections,
}

/// Lays the document out.
pub fn layout(
    doc: &Document,
    fonts: &dyn FontPort,
    options: &LayoutOptions,
) -> Result<Layout, LayoutError> {
    let first = doc.sections.first().ok_or(LayoutError::NoSections)?;
    let mut ctx = Ctx::new(doc, Fonts::new(fonts), options);
    let mut counters = labels::Counters::new(&doc.numbering);
    let mut flow = Flow::new(Geometry::of(&first.properties.page));
    for (index, section) in doc.sections.iter().enumerate() {
        let geometry = Geometry::of(&section.properties.page);
        let width = geometry.columns[0].width;
        if index > 0 {
            flow.start_section(&mut ctx, index, geometry, section.properties.start)?;
        }
        let blocks = section
            .blocks
            .iter()
            .map(|b| stack::block(&mut ctx, b, width, &mut counters))
            .collect::<Result<Vec<_>, _>>()?;
        flow::place_blocks(&mut flow, &mut ctx, blocks)?;
    }
    let mut flow = flow.finish(&mut ctx)?;
    furniture::furnish(&mut ctx, &mut flow.sheets)?;
    let mut warnings = flow.warnings;
    for (requested, served) in ctx.fonts.substitutions() {
        warnings.push(Warning::Substituted {
            requested: requested.clone(),
            served: served.clone(),
        });
    }
    let faces = ctx.fonts.keys().to_vec();
    for (face, count) in &ctx.missing {
        warnings.push(Warning::MissingGlyphs {
            face: faces[usize::from(face.0)].clone(),
            count: *count,
        });
    }
    Ok(Layout {
        pages: flow.sheets.into_iter().map(|s| s.page).collect(),
        faces,
        outline: flow.outline,
        signatures: flow.signatures,
        title: doc.meta.title.clone(),
        language: doc.meta.language.clone(),
        warnings,
    })
}
