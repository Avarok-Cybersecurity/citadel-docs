//! Shared helpers: explicit reader options and a projection of a document onto what Markdown
//! can say, so two documents can be compared by meaning.

#![allow(dead_code)]

pub mod strategies;

use cdoc_md::MdReadOptions;
use cdoc_model::{
    Alignment, Block, Document, InlineContent, Length, NumberFormat, Paragraph, StyleId,
};

/// Test options (PCND): a 6 inch text area and 1 inch by 0.5 inch images; any values do, the
/// properties never depend on them.
pub fn options() -> MdReadOptions {
    MdReadOptions {
        text_width: Length::from_inches(6.0),
        image_width: Length::from_inches(1.0),
        image_height: Length::from_inches(0.5),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    pub text: String,
    pub flags: (bool, bool, bool, bool),
    pub link: Option<(String, Option<String>)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    Normal,
    Quote,
    Heading(u8),
    Item { ordered: bool, level: u8 },
    Rule,
}

/// One table cell: header flag, alignment, runs.
pub type Cell = (bool, Option<Alignment>, Vec<Run>);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    Para(Kind, Vec<Run>),
    Table(Vec<Vec<Cell>>),
}

fn runs(p: &Paragraph) -> Vec<Run> {
    let mut out: Vec<Run> = Vec::new();
    for inline in &p.inlines {
        let (text, flags) = match &inline.content {
            InlineContent::Text(t) => {
                let pr = &t.properties;
                let code = pr.font.as_deref() == Some(cdoc_md::CODE_FONT);
                (
                    t.text.clone(),
                    (
                        pr.bold == Some(true),
                        pr.italic == Some(true),
                        pr.strike == Some(true),
                        code,
                    ),
                )
            }
            InlineContent::Break { .. } => ("\n".to_owned(), (false, false, false, false)),
            InlineContent::Image(i) => (
                format!("[img:{}]", i.content_id.as_str()),
                (false, false, false, false),
            ),
            other => panic!("unexpected {other:?}"),
        };
        let link = inline
            .marks
            .link
            .as_ref()
            .map(|l| (l.href.clone(), l.title.clone()));
        match out.last_mut() {
            Some(last) if last.flags == flags && last.link == link => last.text.push_str(&text),
            _ => out.push(Run { text, flags, link }),
        }
    }
    out
}

fn kind(doc: &Document, p: &Paragraph) -> Kind {
    if let Some(r) = &p.properties.numbering {
        let def = doc
            .numbering
            .definition_for(r.num_id)
            .expect("list definition");
        let ordered = def.levels[usize::from(r.level)].format != NumberFormat::Bullet;
        return Kind::Item {
            ordered,
            level: r.level,
        };
    }
    let style = p
        .properties
        .style
        .as_ref()
        .map(StyleId::as_str)
        .unwrap_or_default();
    if let Some(n) = style.strip_prefix("Heading") {
        return Kind::Heading(n.parse().expect("level"));
    }
    if p.properties
        .borders
        .as_ref()
        .is_some_and(|b| b.bottom.is_some())
        && p.inlines.is_empty()
    {
        return Kind::Rule;
    }
    if style == "Quote" {
        Kind::Quote
    } else {
        Kind::Normal
    }
}

/// What a document says, in Markdown's terms.
pub fn project(doc: &Document) -> Vec<Item> {
    doc.sections
        .iter()
        .flat_map(|s| s.blocks.iter())
        .map(|b| match b {
            Block::Paragraph(p) => Item::Para(kind(doc, p), runs(p)),
            Block::Table(t) => Item::Table(
                t.rows
                    .iter()
                    .map(|r| {
                        r.cells
                            .iter()
                            .map(|c| {
                                let Block::Paragraph(p) = &c.blocks[0] else {
                                    panic!("paragraph cell")
                                };
                                (c.header, p.properties.alignment, runs(p))
                            })
                            .collect()
                    })
                    .collect(),
            ),
        })
        .collect()
}
