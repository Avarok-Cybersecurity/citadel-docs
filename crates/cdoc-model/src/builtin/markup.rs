//! What the markup readers (Markdown, the chat editor's TipTap JSON) build their documents
//! from, so both read a list, a rule or code the same way: one bullet list shared by every
//! bullet list, one list per ordered list counting from its own start, every level of a list
//! in the same format (as a browser draws nested HTML lists), a rule as an empty paragraph
//! with a bottom border, and code in one monospace family.

use crate::borders::{Border, BorderStyle, Borders};
use crate::ids::{AbstractNumId, NumId};
use crate::numbering::{
    AbstractNumbering, Level, LevelOverride, NumberFormat, Numbering, NumberingInstance, MAX_LEVEL,
};
use crate::paragraph::{Alignment, FirstLine, Indent, Paragraph, ParagraphProperties};
use crate::units::{BorderWidth, Length};

/// Monospace families that read as code.
pub const MONOSPACE: [&str; 5] = ["Courier New", "Consolas", "Courier", "Menlo", "Monaco"];

/// The family code is written in.
pub const CODE_FONT: &str = "Courier New";

const GLYPHS: [&str; 3] = ["\u{2022}", "\u{25E6}", "\u{25AA}"];

/// Formats in the order their definitions are numbered: bullets are definition 1 and decimal
/// numbers definition 2 whatever a document uses first.
const FORMATS: [NumberFormat; 6] = [
    NumberFormat::Bullet,
    NumberFormat::Decimal,
    NumberFormat::LowerLetter,
    NumberFormat::UpperLetter,
    NumberFormat::LowerRoman,
    NumberFormat::UpperRoman,
];

/// The left indent of list level `level` (0-based): a further half inch per level.
#[must_use]
pub fn list_indent(level: u8) -> Length {
    Length::from_twips(720 * (i32::from(level) + 1))
}

fn level(n: u8, format: NumberFormat) -> Level {
    let text = if format == NumberFormat::Bullet {
        GLYPHS[usize::from(n) % GLYPHS.len()].to_owned()
    } else {
        format!("%{}.", n + 1)
    };
    Level {
        start: 1,
        format,
        text,
        alignment: Alignment::Left,
        indent: Some(Indent {
            left: Some(list_indent(n)),
            right: None,
            first_line: Some(FirstLine::Hanging {
                by: Length::from_twips(360),
            }),
        }),
        run: None,
    }
}

fn abstract_id(format: NumberFormat) -> AbstractNumId {
    let at = FORMATS.iter().position(|f| *f == format).unwrap_or(1);
    AbstractNumId(u32::try_from(at).unwrap_or(1) + 1)
}

/// Builds the [`Numbering`] of a document read from markup, as its lists appear.
#[derive(Debug, Clone)]
pub struct UniformLists {
    numbering: Numbering,
    bullets: Option<NumId>,
}

impl UniformLists {
    /// No lists yet.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            numbering: Numbering::empty(),
            bullets: None,
        }
    }

    fn next_id(&self) -> NumId {
        NumId(u32::try_from(self.numbering.instances.len()).unwrap_or(u32::MAX) + 1)
    }

    fn ensure(&mut self, format: NumberFormat) -> AbstractNumId {
        let id = abstract_id(format);
        if !self.numbering.definitions.iter().any(|d| d.id == id) {
            self.numbering.definitions.push(AbstractNumbering {
                id,
                levels: (0..=MAX_LEVEL).map(|n| level(n, format)).collect(),
            });
        }
        id
    }

    /// The instance for a bullet list: the same one for every bullet list.
    pub fn bullet_list(&mut self) -> NumId {
        if let Some(id) = self.bullets {
            return id;
        }
        let abstract_id = self.ensure(NumberFormat::Bullet);
        let id = self.next_id();
        self.numbering.instances.push(NumberingInstance {
            id,
            abstract_id,
            overrides: Vec::new(),
        });
        self.bullets = Some(id);
        id
    }

    /// A fresh instance for an ordered list in `format` starting at `start` on `level`. A
    /// `format` of `Bullet` or `None` is read as decimal.
    pub fn ordered_list(&mut self, format: NumberFormat, level: u8, start: u32) -> NumId {
        let format = match format {
            NumberFormat::Bullet | NumberFormat::None => NumberFormat::Decimal,
            other => other,
        };
        let abstract_id = self.ensure(format);
        let id = self.next_id();
        let overrides = if start == 1 {
            Vec::new()
        } else {
            vec![LevelOverride { level, start }]
        };
        self.numbering.instances.push(NumberingInstance {
            id,
            abstract_id,
            overrides,
        });
        id
    }

    /// The lists built.
    #[must_use]
    pub fn finish(self) -> Numbering {
        self.numbering
    }
}

impl Default for UniformLists {
    fn default() -> Self {
        Self::new()
    }
}

/// A horizontal rule: an empty paragraph with a single bottom border.
#[must_use]
pub fn rule_paragraph() -> Paragraph {
    let border = Border {
        style: BorderStyle::Single,
        width: BorderWidth(6),
        color: None,
        space: Length::from_points(1),
    };
    Paragraph {
        properties: ParagraphProperties {
            borders: Some(Borders {
                bottom: Some(border),
                ..Borders::none()
            }),
            ..ParagraphProperties::none()
        },
        inlines: Vec::new(),
    }
}
