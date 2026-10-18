//! The document's own style sheet, for the styles gallery: every style with the run and
//! paragraph properties it resolves to through its `based_on` chain and the defaults, so a
//! gallery tile can preview it without a style engine of its own.

use super::Editor;
use crate::error::EditError;
use cdoc_model::{ParagraphProperties, RunProperties, StyleId, StyleKind};

/// A style as the gallery shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyleEntry {
    /// The id to apply it by.
    pub id: StyleId,
    /// The name users see.
    pub name: String,
    /// Paragraph, character or table.
    pub kind: StyleKind,
    /// The style it inherits from.
    pub based_on: Option<StyleId>,
    /// Character properties after the cascade: what its text looks like.
    pub run: RunProperties,
    /// Paragraph properties after the cascade (the defaults for a character style).
    pub paragraph: ParagraphProperties,
}

impl Editor<'_> {
    /// The document's styles, in style-sheet order, resolved.
    pub fn styles(&self) -> Result<Vec<StyleEntry>, EditError> {
        let sheet = &self.model.styles;
        sheet
            .styles
            .iter()
            .map(|s| {
                let (paragraph, run) = match s.kind {
                    StyleKind::Character => (
                        ParagraphProperties::none(),
                        RunProperties {
                            style: Some(s.id.clone()),
                            ..RunProperties::none()
                        },
                    ),
                    StyleKind::Paragraph | StyleKind::Table => (
                        ParagraphProperties {
                            style: Some(s.id.clone()),
                            ..ParagraphProperties::none()
                        },
                        RunProperties::none(),
                    ),
                };
                Ok(StyleEntry {
                    id: s.id.clone(),
                    name: s.name.clone(),
                    kind: s.kind,
                    based_on: s.based_on.clone(),
                    run: sheet.resolve_run(&paragraph, &run)?,
                    paragraph: sheet.resolve_paragraph(&paragraph)?,
                })
            })
            .collect()
    }
}
