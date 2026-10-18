//! List membership and indentation, as Word's numbering buttons and Tab / Shift+Tab work: a
//! list item moves between levels, any other paragraph by one default tab stop.

use super::format::paragraphs;
use super::{Ctx, Outcome};
use crate::error::EditError;
use cdoc_model::{Indent, Length, NumId, NumberingRef, ParagraphProperties};

/// The highest level `num_id`'s definition has, if it is defined.
fn max_level(ctx: &Ctx<'_, '_>, num_id: NumId) -> Option<u8> {
    let def = ctx.model.numbering.definition_for(num_id)?;
    u8::try_from(def.levels.len().checked_sub(1)?).ok()
}

/// Makes the selected paragraphs items of `num_id` at `level`.
pub(crate) fn set(ctx: &mut Ctx<'_, '_>, num_id: NumId, level: u8) -> Result<Outcome, EditError> {
    let max = max_level(ctx, num_id).ok_or(EditError::InvalidArgument("unknown numbering"))?;
    if level > max {
        return Err(EditError::InvalidArgument("list level out of range"));
    }
    paragraphs(ctx, |p| ParagraphProperties {
        numbering: Some(NumberingRef { num_id, level }),
        ..p.clone()
    })
}

/// Indent (`deeper`) or outdent each selected paragraph.
pub(crate) fn indent(ctx: &mut Ctx<'_, '_>, deeper: bool) -> Result<Outcome, EditError> {
    let tab = ctx.model.settings.default_tab_stop.0;
    let maxes: Vec<(NumId, u8)> = ctx
        .model
        .numbering
        .instances
        .iter()
        .filter_map(|i| Some((i.id, max_level(ctx, i.id)?)))
        .collect();
    paragraphs(ctx, |p| {
        if let Some(n) = p.numbering {
            let max = maxes
                .iter()
                .find(|(id, _)| *id == n.num_id)
                .map_or(0, |m| m.1);
            let level = if deeper {
                (n.level + 1).min(max)
            } else {
                n.level.saturating_sub(1)
            };
            return ParagraphProperties {
                numbering: Some(NumberingRef { level, ..n }),
                ..p.clone()
            };
        }
        let old = p.indent.clone();
        if old.is_none() && !deeper {
            return p.clone();
        }
        let left = old.as_ref().and_then(|i| i.left).map_or(0, |l| l.0);
        let left = if deeper {
            left + tab
        } else {
            (left - tab).max(0)
        };
        let base = old.unwrap_or(Indent {
            left: None,
            right: None,
            first_line: None,
        });
        ParagraphProperties {
            indent: Some(Indent {
                left: Some(Length(left)),
                ..base
            }),
            ..p.clone()
        }
    })
}
