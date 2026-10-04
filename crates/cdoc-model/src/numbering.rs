//! Multilevel lists. A paragraph refers to a [`NumberingInstance`], which refers to an
//! [`AbstractNumbering`] of up to nine levels and may override where levels start. This is
//! Word's model: lists are flat paragraphs with a level, not nested containers, so a `.docx`
//! round-trips and a paragraph can change level without restructuring the tree.

use crate::ids::{AbstractNumId, NumId};
use crate::paragraph::{Alignment, Indent};
use crate::run::RunProperties;
use serde::{Deserialize, Serialize};

/// The deepest list level, inclusive.
pub const MAX_LEVEL: u8 = 8;

/// How a number is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NumberFormat {
    /// 1, 2, 3.
    Decimal,
    /// a, b, c.
    LowerLetter,
    /// A, B, C.
    UpperLetter,
    /// i, ii, iii.
    LowerRoman,
    /// I, II, III.
    UpperRoman,
    /// A bullet glyph taken from the level text.
    Bullet,
    /// No number shown.
    None,
}

/// One level of a list definition.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Level {
    /// The first number at this level.
    pub start: u32,
    /// Number format.
    pub format: NumberFormat,
    /// The label template: `%1` to `%9` stand for the number at that level, so `%1.%2` gives
    /// `1.1`; for bullets the template is the glyph itself.
    pub text: String,
    /// Alignment of the label within its space.
    pub alignment: Alignment,
    /// Indent applied to paragraphs at this level.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub indent: Option<Indent>,
    /// Character formatting of the label (bullet font, number colour).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<RunProperties>,
}

/// A list definition.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AbstractNumbering {
    /// The id instances refer to.
    pub id: AbstractNumId,
    /// Levels, outermost first; between 1 and 9.
    pub levels: Vec<Level>,
}

/// A per-instance override of where a level starts counting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LevelOverride {
    /// The level, `0..=8`.
    pub level: u8,
    /// The new start value.
    pub start: u32,
}

/// A list as used in the body: the same definition can be instantiated several times so two
/// lists with the same look count independently.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NumberingInstance {
    /// The id paragraphs refer to.
    pub id: NumId,
    /// The definition.
    pub abstract_id: AbstractNumId,
    /// Start overrides.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub overrides: Vec<LevelOverride>,
}

/// All list definitions and instances in a document.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Numbering {
    /// Definitions.
    pub definitions: Vec<AbstractNumbering>,
    /// Instances.
    pub instances: Vec<NumberingInstance>,
}

impl Numbering {
    /// No lists.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            definitions: Vec::new(),
            instances: Vec::new(),
        }
    }

    /// The definition an instance uses, if both exist.
    #[must_use]
    pub fn definition_for(&self, num_id: NumId) -> Option<&AbstractNumbering> {
        let instance = self.instances.iter().find(|i| i.id == num_id)?;
        self.definitions
            .iter()
            .find(|d| d.id == instance.abstract_id)
    }
}
