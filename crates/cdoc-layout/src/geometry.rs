// Adapted from rMazing crates/cc-export/src/layout/flow.rs (`Geometry`)
// Copyright (c) Thomas Braun. Relicensed MIT OR Apache-2.0 by the copyright holder, 2026-10-04.

//! The sheet and its columns, from a section's `PageSetup`.
//!
//! The model stores the size as it prints (a landscape page is already wider than tall), so
//! orientation needs no arithmetic here. The gutter is added to the left margin, Word's default
//! gutter position.

use crate::ir::Pt100;
use cdoc_model::PageSetup;

/// One column's horizontal extent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Column {
    /// Left edge, from the page's left edge.
    pub x: Pt100,
    /// Width.
    pub width: Pt100,
}

/// The sheet, its text area and its columns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Geometry {
    /// Sheet width.
    pub page_width: Pt100,
    /// Sheet height.
    pub page_height: Pt100,
    /// Left edge of the text area.
    pub left: Pt100,
    /// Width of the text area (all columns and gaps).
    pub width: Pt100,
    /// Top of the text area.
    pub top: Pt100,
    /// Bottom of the text area: nothing in the body is set below it.
    pub bottom: Pt100,
    /// Where the header's first line starts, from the top edge.
    pub header_top: Pt100,
    /// Where the footer's last line ends, from the top edge.
    pub footer_bottom: Pt100,
    /// The columns, left to right; never empty.
    pub columns: Vec<Column>,
    /// Whether a rule is drawn between columns.
    pub separator: bool,
}

impl Geometry {
    /// The geometry of a section's pages.
    #[must_use]
    pub fn of(setup: &PageSetup) -> Self {
        let m = &setup.margins;
        let page_width = setup.size.width.0;
        let page_height = setup.size.height.0;
        let left = m.left.0 + m.gutter.0;
        let width = (page_width - left - m.right.0).max(1);
        let count = i32::from(setup.columns.count.max(1));
        let gap = setup.columns.gap.0.max(0);
        let column_width = ((width - gap * (count - 1)) / count).max(1);
        let columns = (0..count)
            .map(|i| Column {
                x: left + i * (column_width + gap),
                width: column_width,
            })
            .collect();
        Self {
            page_width,
            page_height,
            left,
            width,
            top: m.top.0,
            bottom: (page_height - m.bottom.0).max(m.top.0 + 1),
            header_top: m.header.0,
            footer_bottom: page_height - m.footer.0,
            columns,
            separator: setup.columns.separator && count > 1,
        }
    }

    /// The height of the body area.
    #[must_use]
    pub const fn body_height(&self) -> Pt100 {
        self.bottom - self.top
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cdoc_model::Length;

    #[test]
    fn a4_normal_has_one_inch_margins_and_one_column() {
        let g = Geometry::of(&PageSetup::a4_normal());
        assert_eq!((g.page_width, g.page_height), (59530, 84190));
        assert_eq!((g.left, g.top), (7200, 7200));
        assert_eq!(g.width, 59530 - 14400);
        assert_eq!(g.bottom, 84190 - 7200);
        assert_eq!(g.columns.len(), 1);
        assert_eq!(g.columns[0].width, g.width);
    }

    #[test]
    fn columns_share_the_width_minus_their_gaps() {
        let mut setup = PageSetup::a4_normal();
        setup.columns.count = 3;
        setup.columns.gap = Length::from_points(20);
        setup.size = setup.size.rotated();
        let g = Geometry::of(&setup);
        assert_eq!(g.page_width, 84190, "landscape is stored as printed");
        let w = g.columns[0].width;
        assert_eq!(w * 3 + 2 * 2000, g.width - (g.width - 4000) % 3);
        assert_eq!(g.columns[1].x, g.columns[0].x + w + 2000);
    }
}
