//! A partial page setup: what the Layout ribbon's page-setup controls change at once.

use crate::page::{Columns, Margins, Orientation, PageSetup, PageSize};
use serde::{Deserialize, Serialize};

/// The page-setup fields to change on a section; `None` keeps the section's value. Page
/// numbering is not part of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageSetupPatch {
    /// The paper, replaced whole.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<PageSize>,
    /// Turns the paper (after `size`) when it has the other orientation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub orientation: Option<Orientation>,
    /// Margins, replaced whole.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub margins: Option<Margins>,
    /// Columns, replaced whole.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub columns: Option<Columns>,
}

impl PageSetupPatch {
    /// A patch that changes nothing.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            size: None,
            orientation: None,
            margins: None,
            columns: None,
        }
    }

    /// Whether no field is set.
    #[must_use]
    pub fn is_none(&self) -> bool {
        *self == Self::none()
    }

    /// `page` with the patch applied.
    #[must_use]
    pub fn apply(&self, page: &PageSetup) -> PageSetup {
        let mut size = self.size.unwrap_or(page.size);
        if self.orientation.is_some_and(|o| o != size.orientation) {
            size = size.rotated();
        }
        PageSetup {
            size,
            margins: self.margins.unwrap_or(page.margins),
            columns: self.columns.unwrap_or(page.columns),
            page_numbering: page.page_numbering,
        }
    }
}
