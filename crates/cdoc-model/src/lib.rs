//! The Citadel Docs document model: the single source of truth for what a document is.
//!
//! Pure data. No I/O, no clock, no randomness: dates arrive as text, images as content ids,
//! identifiers from the caller. Every type is `serde`-friendly with camelCase field names, and
//! the same spellings are the `ProseMirror` attribute names in `cdoc-pm`.
//!
//! The tree: a [`Document`] holds [`Section`]s; a section holds [`Block`]s; a block is a
//! [`Paragraph`] or a [`Table`]; a paragraph holds [`Inline`]s. Styles, numbering, footnotes
//! and comments hang off the document and are referenced by id from the tree.

#![forbid(unsafe_code)]

pub mod block;
pub mod borders;
pub mod builtin;
pub mod fields;
pub mod ids;
pub mod inline;
pub mod normalize;
pub mod notes;
pub mod numbering;
pub mod page;
pub mod paragraph;
pub mod run;
pub mod section;
pub mod style;
pub mod table;
pub mod units;
pub mod validate;
pub mod visit;

#[cfg(feature = "test-support")]
pub mod test_support;

pub use block::Block;
pub use borders::{Border, BorderStyle, Borders, Shading};
pub use fields::{FormField, FormFieldKind, PageAnchor, SignatureField};
pub use ids::{
    AbstractNumId, CommentId, ContentId, FieldId, FootnoteId, NumId, RevisionId, StyleId,
};
pub use inline::{
    BreakKind, Field, FieldKind, Inline, InlineContent, InlineImage, Link, Marks, Revision,
    RevisionKind, TextRun,
};
pub use notes::{Comment, Footnote};
pub use numbering::{
    AbstractNumbering, Level, LevelOverride, NumberFormat, Numbering, NumberingInstance, MAX_LEVEL,
};
pub use page::{Columns, Margins, Orientation, PageNumbering, PageSetup, PageSize};
pub use paragraph::{
    Alignment, FirstLine, Indent, LineSpacing, NumberingRef, Paragraph, ParagraphProperties,
    Spacing,
};
pub use run::{RunProperties, Underline, UnderlineStyle, VerticalAlign};
pub use section::{
    HeaderFooter, HeaderFooterKind, HeaderFooterSet, Section, SectionProperties, SectionStart,
};
pub use style::{DocumentDefaults, Style, StyleError, StyleKind, StyleSheet};
pub use table::{
    CellProperties, RowHeight, Table, TableCell, TableProperties, TableRow, VerticalCellAlign,
};
pub use units::{BorderWidth, Color, ColorParseError, FontSize, Length, TableWidth};
pub use validate::ValidationError;

use serde::{Deserialize, Serialize};

/// Descriptive metadata. The title also lives in the collaboration layer's metadata so it syncs
/// with the document; this copy is what an export writes into the file.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentMeta {
    /// The title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// BCP 47 language tag of the body text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
}

/// Document-wide settings.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentSettings {
    /// Whether even pages use the `even` header and footer.
    pub even_and_odd_headers: bool,
    /// Whether edits are being recorded as tracked changes.
    pub track_changes: bool,
    /// Distance between default tab stops.
    pub default_tab_stop: Length,
}

/// A document.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Document {
    /// Metadata.
    pub meta: DocumentMeta,
    /// Settings.
    pub settings: DocumentSettings,
    /// Styles.
    pub styles: StyleSheet,
    /// Lists.
    pub numbering: Numbering,
    /// Sections; never empty.
    pub sections: Vec<Section>,
    /// Footnote bodies.
    pub footnotes: Vec<Footnote>,
    /// Comments and replies.
    pub comments: Vec<Comment>,
}

impl Document {
    /// A one-section document with the built-in styles, A4 Normal page setup and a single
    /// empty `Normal` paragraph: what "New document" creates.
    #[must_use]
    pub fn blank() -> Self {
        let paragraph = Paragraph {
            properties: ParagraphProperties {
                style: Some(builtin::ids::normal()),
                ..ParagraphProperties::none()
            },
            inlines: Vec::new(),
        };
        Self {
            meta: DocumentMeta {
                title: None,
                language: None,
            },
            settings: DocumentSettings {
                even_and_odd_headers: false,
                track_changes: false,
                default_tab_stop: Length::from_twips(720),
            },
            styles: StyleSheet::word_builtin(),
            numbering: Numbering::empty(),
            sections: vec![Section {
                properties: SectionProperties {
                    start: SectionStart::NextPage,
                    page: PageSetup::a4_normal(),
                    headers: HeaderFooterSet::none(),
                    footers: HeaderFooterSet::none(),
                    title_page: false,
                },
                blocks: vec![Block::Paragraph(paragraph)],
            }],
            footnotes: Vec::new(),
            comments: Vec::new(),
        }
    }
}
