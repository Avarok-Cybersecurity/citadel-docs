//! Block containers: paragraphs and headings, lists (each paragraph taking its list's instance
//! and its nesting depth as level), tables, and the second halves of paragraphs the writer split
//! at a page or column break, joined back.

use super::ctx::{Ctx, Story};
use super::Reader;
use crate::text::Space;
use cdoc_model::{Block, Inline, InlineContent, NumberingRef, Paragraph, MAX_LEVEL};
use cdoc_pkg::xml::Node;
use cdoc_pkg::PackageError;

/// Where a block starts a master page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Start {
    /// The master page.
    pub master: String,
    /// The page number it restarts at.
    pub page_number: Option<u32>,
}

/// A container's blocks and the master pages its blocks start, by block index.
#[derive(Debug, Default)]
pub struct Blocks {
    /// The blocks.
    pub blocks: Vec<Block>,
    /// Section starts.
    pub starts: Vec<(usize, Start)>,
}

impl Reader<'_> {
    /// Read a container's children.
    pub(super) fn container<'n>(
        &mut self,
        children: impl Iterator<Item = &'n Node>,
    ) -> Result<Blocks, PackageError> {
        let mut story = Story::new();
        let mut out = Blocks::default();
        for n in children {
            self.block(n, &mut story, &mut out)?;
        }
        Ok(out)
    }

    pub(super) fn block(
        &mut self,
        n: &Node,
        story: &mut Story,
        out: &mut Blocks,
    ) -> Result<(), PackageError> {
        match n.name.as_str() {
            "text:p" | "text:h" => self.paragraph(n, story, out, None)?,
            "text:list" => self.list(n, story, out, 0, None)?,
            "table:table" => {
                let (table, start) = self.table(n)?;
                if let Some(start) = start {
                    out.starts.push((out.blocks.len(), start));
                }
                out.blocks.push(Block::Table(table));
            }
            "text:tracked-changes" | "text:sequence-decls" => {}
            "text:section" => {
                self.log.lose("a text section read as its content");
                for c in n.elems() {
                    self.block(c, story, out)?;
                }
            }
            "text:table-of-content"
            | "text:alphabetical-index"
            | "text:illustration-index"
            | "text:table-index"
            | "text:object-index"
            | "text:user-index"
            | "text:bibliography" => {
                self.log.lose("an index read as its current text");
                for c in n.find("text:index-body").into_iter().flat_map(Node::elems) {
                    self.block(c, story, out)?;
                }
            }
            "text:soft-page-break" => self.log.lose("a soft page break (ignored)"),
            other => self.log.lose(&format!("{other} between paragraphs")),
        }
        Ok(())
    }

    pub(super) fn paragraph(
        &mut self,
        n: &Node,
        story: &mut Story,
        out: &mut Blocks,
        numbering: Option<NumberingRef>,
    ) -> Result<(), PackageError> {
        let name = n.get("text:style-name");
        let continuation = name
            .and_then(|s| self.sidecar.continuations.get(s))
            .cloned();
        let style = match &continuation {
            Some(_) => self.para_style(None),
            None => self.para_style(name),
        };
        let mut inlines = Vec::new();
        let mut ctx = Ctx {
            story,
            space: Space::start(),
            run: style.run.clone(),
            link: None,
            deletion: None,
        };
        self.inlines(n, &mut ctx, &mut inlines)?;
        if let (Some((kind, marks)), Some(Block::Paragraph(last))) =
            (continuation, out.blocks.last_mut())
        {
            last.inlines.push(Inline {
                content: InlineContent::Break { kind },
                marks,
            });
            last.inlines.extend(inlines);
            return Ok(());
        }
        let mut props = style.props;
        if n.name == "text:h" {
            let level = n
                .get("text:outline-level")
                .and_then(|l| l.parse::<u8>().ok())
                .unwrap_or(1);
            if !(1..=MAX_LEVEL + 1).contains(&level) {
                self.log
                    .lose("a heading level beyond the ninth read as the ninth");
            }
            props.outline_level = Some(level.clamp(1, MAX_LEVEL + 1) - 1);
        }
        if style.list && numbering.is_none() {
            self.log
                .lose("a paragraph's list through its style (style:list-style-name)");
        }
        props.numbering = numbering;
        if let Some(master) = style.master {
            out.starts.push((
                out.blocks.len(),
                Start {
                    master,
                    page_number: style.page_number,
                },
            ));
        }
        out.blocks.push(Block::Paragraph(Paragraph {
            properties: props,
            inlines,
        }));
        Ok(())
    }
}
