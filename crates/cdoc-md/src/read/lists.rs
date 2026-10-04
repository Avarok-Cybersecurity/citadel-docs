//! The numbering definitions Markdown lists need: one bullet definition shared by every
//! bullet list, and one decimal definition instantiated once per ordered list so each counts
//! from its own start.

use cdoc_model::{
    AbstractNumId, AbstractNumbering, Alignment, FirstLine, Indent, Length, Level, LevelOverride,
    NumId, NumberFormat, Numbering, NumberingInstance, MAX_LEVEL,
};

const BULLET: AbstractNumId = AbstractNumId(1);
const DECIMAL: AbstractNumId = AbstractNumId(2);
const GLYPHS: [&str; 3] = ["\u{2022}", "\u{25E6}", "\u{25AA}"];

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
            left: Some(Length::from_twips(720 * (i32::from(n) + 1))),
            right: None,
            first_line: Some(FirstLine::Hanging {
                by: Length::from_twips(360),
            }),
        }),
        run: None,
    }
}

fn definition(id: AbstractNumId, format: NumberFormat) -> AbstractNumbering {
    AbstractNumbering {
        id,
        levels: (0..=MAX_LEVEL).map(|n| level(n, format)).collect(),
    }
}

/// Builds [`Numbering`] as lists appear.
pub(super) struct ListBuilder {
    numbering: Numbering,
    bullets: Option<NumId>,
}

impl ListBuilder {
    pub(super) const fn new() -> Self {
        Self {
            numbering: Numbering::empty(),
            bullets: None,
        }
    }

    fn next_id(&self) -> NumId {
        NumId(u32::try_from(self.numbering.instances.len()).unwrap_or(u32::MAX) + 1)
    }

    fn ensure(&mut self, id: AbstractNumId, format: NumberFormat) {
        if !self.numbering.definitions.iter().any(|d| d.id == id) {
            self.numbering.definitions.push(definition(id, format));
        }
    }

    /// The instance for a new bullet list (shared).
    pub(super) fn bullet_list(&mut self) -> NumId {
        if let Some(id) = self.bullets {
            return id;
        }
        self.ensure(BULLET, NumberFormat::Bullet);
        let id = self.next_id();
        self.numbering.instances.push(NumberingInstance {
            id,
            abstract_id: BULLET,
            overrides: Vec::new(),
        });
        self.bullets = Some(id);
        id
    }

    /// A fresh instance for an ordered list starting at `start` on `level`.
    pub(super) fn ordered_list(&mut self, level: u8, start: u32) -> NumId {
        self.ensure(DECIMAL, NumberFormat::Decimal);
        let id = self.next_id();
        let overrides = if start == 1 {
            Vec::new()
        } else {
            vec![LevelOverride { level, start }]
        };
        self.numbering.instances.push(NumberingInstance {
            id,
            abstract_id: DECIMAL,
            overrides,
        });
        id
    }

    pub(super) fn finish(self) -> Numbering {
        self.numbering
    }
}
