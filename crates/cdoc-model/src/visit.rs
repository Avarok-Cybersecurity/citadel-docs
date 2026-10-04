//! Walking a document: every block, paragraph, inline and run-properties set, wherever it
//! lives (body, headers, footers, footnotes, table cells). Validation, word counts and the
//! exporters all use these rather than re-deriving the containment rules.

use crate::block::Block;
use crate::inline::{Inline, InlineContent};
use crate::paragraph::Paragraph;
use crate::run::RunProperties;
use crate::Document;

/// Where a block was found, for error messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Container {
    /// The body of section `index`.
    Section(usize),
    /// A header of section `index`.
    Header(usize),
    /// A footer of section `index`.
    Footer(usize),
    /// Footnote `index` (position, not id).
    Footnote(usize),
}

impl Document {
    /// Every top-level block with the container it belongs to, in document order.
    pub fn blocks(&self) -> impl Iterator<Item = (Container, &Block)> + '_ {
        let sections = self.sections.iter().enumerate().flat_map(|(i, s)| {
            let body = s.blocks.iter().map(move |b| (Container::Section(i), b));
            let headers = s
                .properties
                .headers
                .present()
                .into_iter()
                .flat_map(move |(_, hf)| hf.blocks.iter().map(move |b| (Container::Header(i), b)));
            let footers = s
                .properties
                .footers
                .present()
                .into_iter()
                .flat_map(move |(_, hf)| hf.blocks.iter().map(move |b| (Container::Footer(i), b)));
            body.chain(headers).chain(footers)
        });
        let footnotes = self
            .footnotes
            .iter()
            .enumerate()
            .flat_map(|(i, f)| f.blocks.iter().map(move |b| (Container::Footnote(i), b)));
        sections.chain(footnotes)
    }

    /// Every paragraph anywhere, descending into table cells.
    pub fn paragraphs(&self) -> impl Iterator<Item = &Paragraph> + '_ {
        self.blocks().flat_map(|(_, b)| b.paragraphs())
    }

    /// Every inline anywhere.
    pub fn inlines(&self) -> impl Iterator<Item = &Inline> + '_ {
        self.paragraphs().flat_map(|p| p.inlines.iter())
    }

    /// Every run-properties set anywhere: direct formatting on text, style halves, document
    /// defaults and list-level labels.
    pub fn run_properties(&self) -> impl Iterator<Item = &RunProperties> + '_ {
        let direct = self.inlines().filter_map(|i| match &i.content {
            InlineContent::Text(run) => Some(&run.properties),
            _ => None,
        });
        let styles = self.styles.styles.iter().map(|s| &s.run);
        let levels = self
            .numbering
            .definitions
            .iter()
            .flat_map(|d| d.levels.iter())
            .filter_map(|l| l.run.as_ref());
        std::iter::once(&self.styles.defaults.run)
            .chain(styles)
            .chain(direct)
            .chain(levels)
    }

    /// Words in the body text, counted as whitespace-separated tokens across all text runs
    /// of each paragraph (runs are joined, so a word split across two runs counts once).
    #[must_use]
    pub fn word_count(&self) -> usize {
        self.sections
            .iter()
            .flat_map(|s| s.blocks.iter())
            .flat_map(Block::paragraphs)
            .map(|p| {
                let text: String = p
                    .inlines
                    .iter()
                    .map(|i| match &i.content {
                        InlineContent::Text(run) => run.text.as_str(),
                        InlineContent::Tab | InlineContent::Break { .. } => " ",
                        _ => "",
                    })
                    .collect();
                text.split_whitespace().count()
            })
            .sum()
    }
}
