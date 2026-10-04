//! Page setup: size, orientation, margins, columns and page numbering.

use crate::numbering::NumberFormat;
use crate::units::Length;
use serde::{Deserialize, Serialize};

/// Portrait or landscape. The model stores the page size as it prints; orientation is kept
/// so the Layout ribbon can show which radio is selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Orientation {
    /// Taller than wide.
    Portrait,
    /// Wider than tall.
    Landscape,
}

/// The paper.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageSize {
    /// Width as printed.
    pub width: Length,
    /// Height as printed.
    pub height: Length,
    /// Orientation.
    pub orientation: Orientation,
}

impl PageSize {
    /// ISO A4, portrait.
    pub const A4_PORTRAIT: Self = Self {
        width: Length::from_twips(11906),
        height: Length::from_twips(16838),
        orientation: Orientation::Portrait,
    };
    /// US Letter, portrait.
    pub const LETTER_PORTRAIT: Self = Self {
        width: Length::from_twips(12240),
        height: Length::from_twips(15840),
        orientation: Orientation::Portrait,
    };

    /// The same paper turned the other way.
    #[must_use]
    pub const fn rotated(self) -> Self {
        Self {
            width: self.height,
            height: self.width,
            orientation: match self.orientation {
                Orientation::Portrait => Orientation::Landscape,
                Orientation::Landscape => Orientation::Portrait,
            },
        }
    }
}

/// Page margins; the header and footer distances are measured from the page edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Margins {
    /// Top.
    pub top: Length,
    /// Bottom.
    pub bottom: Length,
    /// Left (inside, for mirrored margins).
    pub left: Length,
    /// Right (outside).
    pub right: Length,
    /// Header distance from the top edge.
    pub header: Length,
    /// Footer distance from the bottom edge.
    pub footer: Length,
    /// Extra binding space.
    pub gutter: Length,
}

impl Margins {
    /// Word's "Normal" margins: one inch on every side, half an inch to headers and footers.
    pub const NORMAL: Self = Self {
        top: Length::from_twips(1440),
        bottom: Length::from_twips(1440),
        left: Length::from_twips(1440),
        right: Length::from_twips(1440),
        header: Length::from_twips(720),
        footer: Length::from_twips(720),
        gutter: Length(0),
    };

    /// All margin lengths, for validation.
    #[must_use]
    pub const fn all(&self) -> [Length; 7] {
        [
            self.top,
            self.bottom,
            self.left,
            self.right,
            self.header,
            self.footer,
            self.gutter,
        ]
    }
}

/// Text columns within the section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Columns {
    /// Number of columns, `>= 1`.
    pub count: u16,
    /// Space between columns.
    pub gap: Length,
    /// Draw a vertical rule between columns.
    pub separator: bool,
}

impl Columns {
    /// A single column.
    pub const ONE: Self = Self {
        count: 1,
        gap: Length::from_twips(720),
        separator: false,
    };
}

/// How page numbers are shown and where they start in this section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageNumbering {
    /// Number format.
    pub format: NumberFormat,
    /// Restart at this number; `None` continues from the previous section.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<u32>,
}

/// Everything the Layout ribbon sets.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageSetup {
    /// The paper.
    pub size: PageSize,
    /// Margins.
    pub margins: Margins,
    /// Columns.
    pub columns: Columns,
    /// Page numbering.
    pub page_numbering: PageNumbering,
}

impl PageSetup {
    /// A4, Normal margins, one column, decimal page numbers continuing from the previous
    /// section. A named preset, chosen by the caller, not an implicit default.
    #[must_use]
    pub const fn a4_normal() -> Self {
        Self {
            size: PageSize::A4_PORTRAIT,
            margins: Margins::NORMAL,
            columns: Columns::ONE,
            page_numbering: PageNumbering {
                format: NumberFormat::Decimal,
                start: None,
            },
        }
    }
}
