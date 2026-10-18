//! Incremental layout. A [`LayoutSession`] keeps its last layout with what produced it: every
//! block measured and broken into strips (the paragraph cache), where each block was placed,
//! what each page's body was. Told what changed ([`Dirty`]), it measures only the blocks whose
//! measurement can differ, resumes placement at the first changed block from the state
//! recorded there, and stops as soon as the flow stands where it stood before; the untouched
//! tail is moved into place, and headers and footers are set again only where what they print
//! changed.
//!
//! The result is the layout a fresh [`crate::layout`] produces, exactly: [`crate::layout`] is
//! this session's full relayout, so there is one way to lay a document out.
//!
//! The session is bound to one font port and one set of options: measurements are only valid
//! for the faces and options they were made with, so a mismatch is not representable.

mod attempt;
mod canon;
mod dirty;
mod edit;
mod finalize;
mod furnish;
mod hash;
mod measure;
mod record;
mod run;
mod splice;

pub use dirty::{BlockEdit, Dirty};

use crate::context::LayoutOptions;
use crate::fonts::{FontPort, Fonts};
use crate::hyphens::Hyphenators;
use crate::ir::Layout;
use crate::LayoutError;
use cdoc_model::{Block, Document, InlineContent};
use measure::Cache;
use record::Baseline;

/// A layout kept up to date across edits.
pub struct LayoutSession<'s> {
    port: &'s dyn FontPort,
    options: &'s LayoutOptions,
    /// `None` only while a relayout holds them.
    fonts: Option<Fonts<'s>>,
    /// `None` only while a relayout holds them.
    hyphenators: Option<Hyphenators>,
    epoch: u64,
    cache: Cache,
    base: Option<Baseline>,
}

impl std::fmt::Debug for LayoutSession<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LayoutSession")
            .field("epoch", &self.epoch)
            .field("cached", &self.cache.len())
            .field("pages", &self.base.as_ref().map(|b| b.layout.pages.len()))
            .finish_non_exhaustive()
    }
}

/// The outcome of a relayout: the layout, and the pages a painter must repaint.
#[derive(Debug)]
pub struct Relayout<'a> {
    layout: &'a Layout,
    changed: Vec<usize>,
}

impl Relayout<'_> {
    /// The whole layout.
    #[must_use]
    pub const fn layout(&self) -> &Layout {
        self.layout
    }

    /// Indices of the pages whose contents differ from the previous layout's page at the same
    /// index, ascending. Pages beyond the new page count are gone.
    #[must_use]
    pub fn changed_pages(&self) -> &[usize] {
        &self.changed
    }
}

/// Whether a footnote's body references a footnote: it is numbered while the referencing
/// note is placed, which ties numbering to placement and rules out resuming in the middle.
fn notes_number_notes(doc: &Document) -> bool {
    doc.footnotes
        .iter()
        .flat_map(|n| n.blocks.iter().flat_map(Block::paragraphs))
        .flat_map(|p| &p.inlines)
        .any(|i| matches!(i.content, InlineContent::FootnoteRef { .. }))
}

impl<'s> LayoutSession<'s> {
    /// A session with no layout yet: the first relayout must be [`Dirty::all`].
    #[must_use]
    pub fn new(fonts: &'s dyn FontPort, options: &'s LayoutOptions) -> Self {
        Self {
            port: fonts,
            options,
            fonts: Some(Fonts::new(fonts)),
            hyphenators: Some(Hyphenators::new()),
            epoch: 0,
            cache: Cache::new(),
            base: None,
        }
    }

    /// Lays `doc` out again, given what changed since the previous relayout.
    ///
    /// On an error the session forgets everything (the next relayout must be [`Dirty::all`])
    /// and the error is the one a fresh layout reports.
    pub fn relayout(&mut self, doc: &Document, dirty: &Dirty) -> Result<Relayout<'_>, LayoutError> {
        if doc.sections.is_empty() {
            return Err(LayoutError::NoSections);
        }
        let Some(base) = &self.base else {
            if !dirty.everything {
                return Err(LayoutError::NoBaseline);
            }
            return self.finish(doc, dirty, false);
        };
        let before: Vec<usize> = base.sections.iter().map(|s| s.blocks.len()).collect();
        let after: Vec<usize> = doc.sections.iter().map(|s| s.blocks.len()).collect();
        dirty.check(&before, &after)?;
        let incremental = !dirty.replaces_all() && !notes_number_notes(doc);
        self.finish(doc, dirty, incremental)
    }

    fn finish(
        &mut self,
        doc: &Document,
        dirty: &Dirty,
        incremental: bool,
    ) -> Result<Relayout<'_>, LayoutError> {
        let changed = match self.attempt(doc, dirty, incremental) {
            Ok(changed) => changed,
            Err(error) => {
                self.reset();
                if !incremental {
                    return Err(error);
                }
                // A partial relayout meets blocks in another order than a fresh one, so it may
                // fail on another of several faulty blocks: report the fresh layout's error.
                let all = Dirty::all();
                let changed = self.attempt(doc, &all, false);
                changed.inspect_err(|_| self.reset())?
            }
        };
        let layout = &self
            .base
            .as_ref()
            .expect("an attempt that succeeds sets it")
            .layout;
        Ok(Relayout { layout, changed })
    }

    fn reset(&mut self) {
        self.fonts = Some(Fonts::new(self.port));
        self.hyphenators.get_or_insert_with(Hyphenators::new);
        self.cache.clear();
        self.base = None;
        self.epoch += 1;
    }

    /// The current layout, if a relayout has succeeded.
    #[must_use]
    pub fn layout(&self) -> Option<&Layout> {
        self.base.as_ref().map(|b| &b.layout)
    }

    /// The current layout, taken.
    pub fn into_layout(self) -> Result<Layout, LayoutError> {
        self.base.map(|b| b.layout).ok_or(LayoutError::NoBaseline)
    }
}
