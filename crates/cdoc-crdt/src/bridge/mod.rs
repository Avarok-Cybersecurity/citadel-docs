//! The bridge between the flat CRDT sequence and the `cdoc-model` tree. The model stays the
//! single source of truth for what the values mean: atoms, marks and attributes carry model
//! values (codec-encoded), never copies of their types.

mod atoms;
mod build;
mod edit;
mod entries;
mod fields;
mod formats;
mod from_model;
mod resolve;
mod run_props;
mod slice;
mod to_model;

use crate::codec::{from_bytes, to_bytes, CodecError};
use crate::doc::EditError;
use crate::item::AtomKind;
use cdoc_model::{
    CellProperties, Comment, DocumentMeta, DocumentSettings, FootnoteId, HeaderFooterKind,
    InlineContent, Length, Link, Numbering, ParagraphProperties, Revision, RowHeight,
    SectionProperties, StyleSheet, TableProperties,
};
use cdoc_model::{CommentId, RunProperties};

pub use formats::{formats, Formats};
pub use from_model::{blank, from_model};
pub use slice::{atom_at, fold_blocks, slice, Piece};
pub use to_model::to_model;

/// Header or footer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Furniture {
    /// A header.
    Header,
    /// A footer.
    Footer,
}

/// A unit that is not a character. Structural atoms delimit blocks:
///
/// ```text
/// document  := section+ footnote*
/// section   := Section furniture* block*
/// furniture := HeaderFooter block* End
/// footnote  := Footnote block* End
/// block     := Paragraph inline* | Table row* End
/// row       := Row cell*
/// cell      := Cell block*
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Atom {
    /// An inline object; never text (text is characters).
    Inline(InlineContent),
    /// A section start; its headers and footers must be empty (they are blocks).
    Section(SectionProperties),
    /// The start of a header or footer of the current section.
    HeaderFooter {
        /// Header or footer.
        furniture: Furniture,
        /// Which pages.
        kind: HeaderFooterKind,
    },
    /// The start of a footnote body.
    Footnote(FootnoteId),
    /// A paragraph start.
    Paragraph(ParagraphProperties),
    /// A table start.
    Table {
        /// Table properties.
        properties: TableProperties,
        /// Grid column widths.
        column_widths: Vec<Length>,
    },
    /// A row start.
    Row {
        /// Height rule.
        height: Option<RowHeight>,
        /// Never split across pages.
        cant_split: bool,
    },
    /// A cell start.
    Cell {
        /// Cell properties.
        properties: CellProperties,
        /// Whether it is a header cell.
        header: bool,
    },
    /// Closes the innermost open table, header, footer or footnote.
    End,
}

impl Atom {
    pub(crate) fn encode(&self) -> Result<(AtomKind, Vec<u8>), EditError> {
        Ok(match self {
            Self::Inline(InlineContent::Text(_)) => return Err(EditError::TextAtom),
            Self::Inline(c) => (AtomKind::Inline, to_bytes(c)?),
            Self::Section(p) => {
                let none = cdoc_model::HeaderFooterSet::none();
                if p.headers != none || p.footers != none {
                    return Err(EditError::FurnitureInSection);
                }
                (AtomKind::Section, to_bytes(p)?)
            }
            Self::HeaderFooter { furniture, kind } => {
                (AtomKind::HeaderFooter, to_bytes(&(furniture, kind))?)
            }
            Self::Footnote(id) => (AtomKind::Footnote, to_bytes(id)?),
            Self::Paragraph(p) => (AtomKind::Paragraph, to_bytes(p)?),
            Self::Table {
                properties,
                column_widths,
            } => (AtomKind::Table, to_bytes(&(properties, column_widths))?),
            Self::Row { height, cant_split } => (AtomKind::Row, to_bytes(&(height, cant_split))?),
            Self::Cell { properties, header } => (AtomKind::Cell, to_bytes(&(properties, header))?),
            Self::End => (AtomKind::End, Vec::new()),
        })
    }

    pub(crate) fn decode(kind: AtomKind, payload: &[u8]) -> Result<Self, CodecError> {
        Ok(match kind {
            AtomKind::Inline => Self::Inline(from_bytes(payload)?),
            AtomKind::Section => Self::Section(from_bytes(payload)?),
            AtomKind::HeaderFooter => {
                let (furniture, kind) = from_bytes(payload)?;
                Self::HeaderFooter { furniture, kind }
            }
            AtomKind::Footnote => Self::Footnote(from_bytes(payload)?),
            AtomKind::Paragraph => Self::Paragraph(from_bytes(payload)?),
            AtomKind::Table => {
                let (properties, column_widths) = from_bytes(payload)?;
                Self::Table {
                    properties,
                    column_widths,
                }
            }
            AtomKind::Row => {
                let (height, cant_split) = from_bytes(payload)?;
                Self::Row { height, cant_split }
            }
            AtomKind::Cell => {
                let (properties, header) = from_bytes(payload)?;
                Self::Cell { properties, header }
            }
            AtomKind::End if payload.is_empty() => Self::End,
            AtomKind::End => {
                return Err(CodecError::Message("an end atom has no payload".to_owned()))
            }
        })
    }
}

/// A document attribute: a last-writer-wins value of the document as a whole.
// A short-lived argument passed by reference; boxing the style sheet would only add noise.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocAttr {
    /// `meta`.
    Meta(DocumentMeta),
    /// `settings`.
    Settings(DocumentSettings),
    /// `styles`.
    Styles(StyleSheet),
    /// `numbering`.
    Numbering(Numbering),
    /// `comments`.
    Comments(Vec<Comment>),
}

impl DocAttr {
    /// The attribute's name, as reported in [`crate::Changes::attrs`].
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Meta(_) => names::META,
            Self::Settings(_) => names::SETTINGS,
            Self::Styles(_) => names::STYLES,
            Self::Numbering(_) => names::NUMBERING,
            Self::Comments(_) => names::COMMENTS,
        }
    }
}

/// Attribute names.
pub mod names {
    /// Document metadata.
    pub const META: &str = "meta";
    /// Document settings.
    pub const SETTINGS: &str = "settings";
    /// The style sheet.
    pub const STYLES: &str = "styles";
    /// List definitions.
    pub const NUMBERING: &str = "numbering";
    /// Comments and replies.
    pub const COMMENTS: &str = "comments";
}

/// Formatting applied to a range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mark {
    /// Set every character property that is `Some` here; the others are left alone.
    Run(RunProperties),
    /// Remove one character property (back to inheriting).
    ClearRun(crate::marks::RunKey),
    /// Set or remove the hyperlink.
    Link(Option<Link>),
    /// Set or remove the tracked change.
    Revision(Option<Revision>),
    /// Anchor a comment on the range, or remove its anchoring.
    Comment {
        /// The comment.
        id: CommentId,
        /// `true` anchors, `false` removes.
        anchored: bool,
    },
}

/// The model and the CRDT disagree.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BridgeError {
    /// A stored value does not decode as the model type it should be.
    #[error(transparent)]
    Codec(#[from] CodecError),
    /// Building the CRDT from a model failed.
    #[error(transparent)]
    Edit(#[from] EditError),
    /// The sequence holds no section atom, so there is nowhere to put the content.
    #[error("the document has no section")]
    NoSection,
    /// Client 0 ([`ClientId::BLANK`](crate::ClientId::BLANK)) authors the blank document's
    /// base; no replica writes as it.
    #[error("client 0 is reserved for the blank document's base items")]
    ReservedClient,
    /// A document attribute was never written.
    #[error("the document attribute {0:?} was never set")]
    MissingAttr(&'static str),
}
