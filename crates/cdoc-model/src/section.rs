//! Sections: a run of blocks sharing one page setup and one set of headers and footers.

use crate::block::Block;
use crate::page::PageSetup;
use serde::{Deserialize, Serialize};

/// Where a section starts relative to the previous one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SectionStart {
    /// On the next page.
    NextPage,
    /// On the same page, directly after the previous section (column changes).
    Continuous,
    /// On the next even page.
    EvenPage,
    /// On the next odd page.
    OddPage,
}

/// A header or footer: block content set in the page margin.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeaderFooter {
    /// Content; never empty.
    pub blocks: Vec<Block>,
}

/// Which page a header or footer applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HeaderFooterKind {
    /// Every page not covered by `first` or `even`.
    Default,
    /// The section's first page; only used when `title_page` is set.
    First,
    /// Even pages; only used when the document's `even_and_odd_headers` is set.
    Even,
}

/// The up-to-three variants of a header (or footer) a section can have.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeaderFooterSet {
    /// Default pages.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<HeaderFooter>,
    /// First page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first: Option<HeaderFooter>,
    /// Even pages.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub even: Option<HeaderFooter>,
}

impl HeaderFooterSet {
    /// No header or footer.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            default: None,
            first: None,
            even: None,
        }
    }

    /// The variants that are present, in a fixed order.
    #[must_use]
    pub fn present(&self) -> Vec<(HeaderFooterKind, &HeaderFooter)> {
        [
            (HeaderFooterKind::Default, self.default.as_ref()),
            (HeaderFooterKind::First, self.first.as_ref()),
            (HeaderFooterKind::Even, self.even.as_ref()),
        ]
        .into_iter()
        .filter_map(|(kind, hf)| hf.map(|hf| (kind, hf)))
        .collect()
    }

    /// The slot for a kind, for building a set one variant at a time.
    pub fn slot(&mut self, kind: HeaderFooterKind) -> &mut Option<HeaderFooter> {
        match kind {
            HeaderFooterKind::Default => &mut self.default,
            HeaderFooterKind::First => &mut self.first,
            HeaderFooterKind::Even => &mut self.even,
        }
    }
}

/// Section-level properties.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SectionProperties {
    /// How the section starts.
    pub start: SectionStart,
    /// Page setup.
    pub page: PageSetup,
    /// Headers.
    pub headers: HeaderFooterSet,
    /// Footers.
    pub footers: HeaderFooterSet,
    /// Whether the first page uses the `first` header and footer.
    pub title_page: bool,
}

/// A section.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Section {
    /// Properties.
    pub properties: SectionProperties,
    /// Body content; never empty.
    pub blocks: Vec<Block>,
}
