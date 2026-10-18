//! Formatting at the caret, read where it is cheapest: a body paragraph's characters from the
//! model view (kept equal to the replica), anything else from the replica.

use super::Editor;
use crate::address::ModelPath;
use crate::caret::{caret_formatting, CaretFormatting};
use crate::commands::props_at;
use crate::error::EditError;
use crate::tx::inline_len;
use cdoc_model::{Block, InlineContent, Paragraph, RunProperties};

/// The direct character properties of unit `unit` of `p`, if it is text.
fn unit_props(p: &Paragraph, unit: usize) -> Option<RunProperties> {
    let mut at = 0;
    for inline in &p.inlines {
        at += inline_len(inline);
        if unit < at {
            return match &inline.content {
                InlineContent::Text(run) => Some(run.properties.clone()),
                _ => None,
            };
        }
    }
    None
}

impl Editor<'_> {
    /// The model paragraph `path` names.
    fn paragraph_at(&self, path: &ModelPath) -> Option<&Paragraph> {
        let mut block = self
            .model
            .sections
            .get(path.section)?
            .blocks
            .get(path.block)?;
        for &(row, cell, inner) in &path.cells {
            let Block::Table(table) = block else {
                return None;
            };
            block = table.rows.get(row)?.cells.get(cell)?.blocks.get(inner)?;
        }
        match block {
            Block::Paragraph(p) => Some(p),
            Block::Table(_) => None,
        }
    }

    /// Formatting where the selection starts, for the ribbons.
    pub fn caret_formatting(&self) -> Result<CaretFormatting, EditError> {
        let start = self.selection.range(&self.doc)?.start;
        let at =
            crate::position::Position::at(&self.doc, start).ok_or(EditError::UnknownPosition)?;
        let path = self.model_path(at)?;
        let model = path.as_ref().and_then(|p| Some((p, self.paragraph_at(p)?)));
        caret_formatting(
            &self.doc,
            &self.outline,
            &self.model.styles,
            self.selection,
            self.pending.as_ref(),
            |i| match model {
                Some((path, para)) => Ok(i
                    .checked_sub(start - path.offset)
                    .and_then(|unit| unit_props(para, unit))),
                None => props_at(&self.doc, i),
            },
        )
    }
}
