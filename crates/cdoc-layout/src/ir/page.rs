// Adapted from rMazing crates/cc-export/src/layout/ir.rs
// Copyright (c) Thomas Braun. Relicensed MIT OR Apache-2.0 by the copyright holder, 2026-10-04.

//! A laid-out sheet, and the document of sheets with what the backends need besides drawings.

use super::{Draw, FaceKey, GlyphRun, Pt100, Rect};
use cdoc_model::FieldId;

/// A clickable area on a page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkArea {
    /// Where.
    pub rect: Rect,
    /// The target URL, exactly as the model holds it; backends decide which schemes to honour.
    pub href: String,
}

/// One sheet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    /// Width.
    pub width: Pt100,
    /// Height.
    pub height: Pt100,
    /// Drawings, in painting order.
    pub draws: Vec<Draw>,
    /// Links.
    pub links: Vec<LinkArea>,
}

impl Page {
    /// An empty sheet.
    #[must_use]
    pub const fn new(width: Pt100, height: Pt100) -> Self {
        Self {
            width,
            height,
            draws: Vec::new(),
            links: Vec::new(),
        }
    }

    /// The runs on the page, in drawing order.
    pub fn runs(&self) -> impl Iterator<Item = &GlyphRun> {
        self.draws.iter().filter_map(|d| match d {
            Draw::Text(run) => Some(run),
            _ => None,
        })
    }

    /// The page's text, one run per line of output: what a test reads.
    #[must_use]
    pub fn text(&self) -> String {
        self.runs()
            .map(|r| r.text.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// A heading, for the PDF outline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutlineEntry {
    /// The outline level, 0 outermost.
    pub level: u8,
    /// The heading text.
    pub title: String,
    /// Zero-based page.
    pub page: u32,
    /// Top of the heading's first line.
    pub y: Pt100,
}

/// Where a signature field landed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignaturePlacement {
    /// The field.
    pub id: FieldId,
    /// The signer role.
    pub signer_role: String,
    /// Zero-based page.
    pub page: u32,
    /// The rectangle, in page space (the model's `PageAnchor` space).
    pub rect: Rect,
}

/// Something the layout had to approximate, reported rather than hidden.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Warning {
    /// Hyphenation was on for text in a language the font port has no patterns registered for
    /// (none shipped, or the caller did not register them); that text was not hyphenated.
    NoHyphenationPatterns {
        /// The BCP 47 tag, as the document gives it.
        language: String,
    },
    /// A face was asked for and a different one served.
    Substituted {
        /// What the document asked for.
        requested: FaceKey,
        /// What the port served.
        served: FaceKey,
    },
    /// Characters the face could not draw, replaced by a visible marker.
    MissingGlyphs {
        /// The face.
        face: FaceKey,
        /// How many characters.
        count: usize,
    },
    /// A block taller than a whole column was placed anyway and runs past the foot.
    Oversized {
        /// Zero-based page.
        page: u32,
    },
}

/// The laid-out document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    /// The sheets.
    pub pages: Vec<Page>,
    /// The faces runs refer to by [`FaceId`](super::FaceId).
    pub faces: Vec<FaceKey>,
    /// Headings, in document order.
    pub outline: Vec<OutlineEntry>,
    /// Signature fields, in document order.
    pub signatures: Vec<SignaturePlacement>,
    /// The document title, for the file's metadata.
    pub title: Option<String>,
    /// The document language (BCP 47).
    pub language: Option<String>,
    /// What had to be approximated.
    pub warnings: Vec<Warning>,
}
