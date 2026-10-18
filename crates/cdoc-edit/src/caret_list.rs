//! The list at the caret, for the Bullets and Numbering buttons: which built-in template the
//! paragraph's numbering definition is (or that it is none of them), and the level.

use cdoc_model::builtin::ListKind;
use cdoc_model::{NumId, Numbering, ParagraphProperties};

/// What kind of list a numbering definition is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CaretListKind {
    /// One of the built-in templates (`Command::SetNumberingKind` applies them).
    Builtin(ListKind),
    /// A definition no template matches (imported, or edited level by level).
    Custom,
}

/// The list a paragraph is an item of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CaretList {
    /// The definition's kind.
    pub kind: CaretListKind,
    /// The paragraph's level, `0..=8`.
    pub level: u8,
}

/// The lists of paragraphs, reading each numbering instance's definition once (a template
/// match compares nine levels).
pub(crate) struct Lists<'n> {
    numbering: &'n Numbering,
    kinds: Vec<(NumId, Option<CaretListKind>)>,
}

impl<'n> Lists<'n> {
    pub(crate) const fn new(numbering: &'n Numbering) -> Self {
        Self {
            numbering,
            kinds: Vec::new(),
        }
    }

    /// The list of a paragraph with these (style-resolved) properties; `None` when it is not
    /// a list item or names an instance the document does not define.
    pub(crate) fn of(&mut self, p: &ParagraphProperties) -> Option<CaretList> {
        let r = p.numbering.as_ref()?;
        let kind = if let Some((_, kind)) = self.kinds.iter().find(|(id, _)| *id == r.num_id) {
            *kind
        } else {
            let kind = self.numbering.definition_for(r.num_id).map(|d| {
                ListKind::of(&d.levels).map_or(CaretListKind::Custom, CaretListKind::Builtin)
            });
            self.kinds.push((r.num_id, kind));
            kind
        };
        Some(CaretList {
            kind: kind?,
            level: r.level,
        })
    }
}
