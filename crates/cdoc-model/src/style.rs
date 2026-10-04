//! Named styles and the cascade that resolves them.
//!
//! Resolution order, as in Word: document defaults, then the paragraph style chain (base
//! first), then the character style chain, then direct formatting. A paragraph style's run
//! half applies to runs in paragraphs of that style; a character style applies to runs that
//! name it.

use crate::ids::StyleId;
use crate::paragraph::ParagraphProperties;
use crate::run::RunProperties;
use serde::{Deserialize, Serialize};

/// What a style can be applied to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StyleKind {
    /// Applies to paragraphs; may carry run properties for their text.
    Paragraph,
    /// Applies to runs.
    Character,
    /// Applies to tables.
    Table,
}

/// A named style.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Style {
    /// The id paragraphs and runs refer to.
    pub id: StyleId,
    /// The name shown in the styles gallery.
    pub name: String,
    /// What it applies to.
    pub kind: StyleKind,
    /// The style this one inherits from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub based_on: Option<StyleId>,
    /// The style of the paragraph created by pressing Enter at the end of one in this style.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<StyleId>,
    /// Paragraph formatting this style contributes.
    pub paragraph: ParagraphProperties,
    /// Run formatting this style contributes.
    pub run: RunProperties,
}

/// Formatting every run and paragraph starts from, before any style.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentDefaults {
    /// Default run formatting; must set at least font and size.
    pub run: RunProperties,
    /// Default paragraph formatting.
    pub paragraph: ParagraphProperties,
}

/// All styles in a document.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StyleSheet {
    /// Document defaults.
    pub defaults: DocumentDefaults,
    /// Styles, in gallery order.
    pub styles: Vec<Style>,
}

/// A failure to resolve a style.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StyleError {
    /// A referenced style does not exist.
    #[error("style {0:?} is not defined")]
    Unknown(StyleId),
    /// Following `based_on` from this style never terminates.
    #[error("style {0:?} is part of a basedOn cycle")]
    Cycle(StyleId),
}

impl StyleSheet {
    /// Look up a style by id.
    #[must_use]
    pub fn get(&self, id: &StyleId) -> Option<&Style> {
        self.styles.iter().find(|s| &s.id == id)
    }

    /// The chain from the root base style down to `id`, inclusive.
    pub fn chain(&self, id: &StyleId) -> Result<Vec<&Style>, StyleError> {
        let mut chain = Vec::new();
        let mut current = Some(id);
        while let Some(current_id) = current {
            if chain.iter().any(|s: &&Style| &s.id == current_id) {
                return Err(StyleError::Cycle(id.clone()));
            }
            let style = self
                .get(current_id)
                .ok_or_else(|| StyleError::Unknown(current_id.clone()))?;
            chain.push(style);
            current = style.based_on.as_ref();
        }
        chain.reverse();
        Ok(chain)
    }

    /// Paragraph properties for a paragraph with the given direct formatting, after the cascade.
    pub fn resolve_paragraph(
        &self,
        direct: &ParagraphProperties,
    ) -> Result<ParagraphProperties, StyleError> {
        let mut resolved = self.defaults.paragraph.clone();
        if let Some(style_id) = &direct.style {
            for style in self.chain(style_id)? {
                resolved = resolved.overlay(&style.paragraph);
            }
        }
        Ok(resolved.overlay(direct))
    }

    /// Run properties for a run with the given direct formatting inside a paragraph with the
    /// given direct formatting, after the cascade.
    pub fn resolve_run(
        &self,
        paragraph: &ParagraphProperties,
        direct: &RunProperties,
    ) -> Result<RunProperties, StyleError> {
        let mut resolved = self.defaults.run.clone();
        if let Some(style_id) = &paragraph.style {
            for style in self.chain(style_id)? {
                resolved = resolved.overlay(&style.run);
            }
        }
        if let Some(style_id) = &direct.style {
            for style in self.chain(style_id)? {
                resolved = resolved.overlay(&style.run);
            }
        }
        Ok(resolved.overlay(direct))
    }
}
