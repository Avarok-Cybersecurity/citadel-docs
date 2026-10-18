//! Word's built-in list templates, the ones the Bullets and Numbering buttons apply: nine
//! levels, each indented a further half inch with a quarter-inch hanging label.

use crate::numbering::{Level, NumberFormat, MAX_LEVEL};
use crate::paragraph::{Alignment, FirstLine, Indent};
use crate::units::Length;
use serde::{Deserialize, Serialize};

/// A list template, by the format of its first level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ListKind {
    /// •, ◦, ▪, repeating.
    Bullet,
    /// 1. then a. then i., repeating.
    Decimal,
    /// a. then i. then 1., repeating.
    LowerLetter,
    /// A. then a. then i., repeating.
    UpperLetter,
    /// i. then 1. then a., repeating.
    LowerRoman,
    /// I. then A. then 1., repeating.
    UpperRoman,
}

impl ListKind {
    /// Every kind, in record order.
    pub const ALL: [Self; 6] = [
        Self::Bullet,
        Self::Decimal,
        Self::LowerLetter,
        Self::UpperLetter,
        Self::LowerRoman,
        Self::UpperRoman,
    ];

    /// The formats of levels 0, 1 and 2; deeper levels repeat them.
    const fn cycle(self) -> [NumberFormat; 3] {
        use NumberFormat::{Bullet, Decimal, LowerLetter, LowerRoman, UpperLetter, UpperRoman};
        match self {
            Self::Bullet => [Bullet, Bullet, Bullet],
            Self::Decimal => [Decimal, LowerLetter, LowerRoman],
            Self::LowerLetter => [LowerLetter, LowerRoman, Decimal],
            Self::UpperLetter => [UpperLetter, LowerLetter, LowerRoman],
            Self::LowerRoman => [LowerRoman, Decimal, LowerLetter],
            Self::UpperRoman => [UpperRoman, UpperLetter, Decimal],
        }
    }

    /// The template whose nine levels `levels` are; `None` for any other definition.
    #[must_use]
    pub fn of(levels: &[Level]) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.levels() == levels)
    }

    /// The template's nine levels.
    #[must_use]
    pub fn levels(self) -> Vec<Level> {
        const GLYPHS: [&str; 3] = ["\u{2022}", "\u{25E6}", "\u{25AA}"];
        (0..=MAX_LEVEL)
            .map(|level| {
                let k = usize::from(level % 3);
                let format = self.cycle()[k];
                let text = if format == NumberFormat::Bullet {
                    GLYPHS[k].to_owned()
                } else {
                    format!("%{}.", level + 1)
                };
                Level {
                    start: 1,
                    format,
                    text,
                    alignment: Alignment::Left,
                    indent: Some(Indent {
                        left: Some(Length::from_twips(720 * (i32::from(level) + 1))),
                        right: None,
                        first_line: Some(FirstLine::Hanging {
                            by: Length::from_twips(360),
                        }),
                    }),
                    run: None,
                }
            })
            .collect()
    }
}
