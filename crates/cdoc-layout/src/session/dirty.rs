//! What changed since the previous layout, as the caller (the editor, which knows) describes
//! it. Nothing is assumed: a change the description leaves out is a change the session will
//! not see, and [`Dirty::all`] is the one way to say "assume nothing".

use crate::LayoutError;

/// One change to a section's block list: `removed` blocks at `at` (indices in the previous
/// document) replaced by `inserted` blocks. An edited block is one removed and one inserted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockEdit {
    /// The section.
    pub section: usize,
    /// The first block affected, in the previous document.
    pub at: usize,
    /// How many blocks of the previous document went.
    pub removed: usize,
    /// How many blocks of the new document replace them.
    pub inserted: usize,
}

impl BlockEdit {
    /// Block `index` of `section` changed in place.
    #[must_use]
    pub const fn changed(section: usize, index: usize) -> Self {
        Self {
            section,
            at: index,
            removed: 1,
            inserted: 1,
        }
    }

    /// `count` blocks inserted before block `at` of `section`.
    #[must_use]
    pub const fn inserted(section: usize, at: usize, count: usize) -> Self {
        Self {
            section,
            at,
            removed: 0,
            inserted: count,
        }
    }

    /// `count` blocks removed from block `at` of `section` on.
    #[must_use]
    pub const fn removed(section: usize, at: usize, count: usize) -> Self {
        Self {
            section,
            at,
            removed: count,
            inserted: 0,
        }
    }
}

/// What changed. Construct with [`Dirty::all`], [`Dirty::edits`] or a literal naming every
/// field.
#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "independent kinds of change, each named"
)]
pub struct Dirty {
    /// Block-list changes, sorted by section then position, not overlapping, in the previous
    /// document's indices.
    pub edits: Vec<BlockEdit>,
    /// The style sheet or the document settings (the default tab stop) changed.
    pub styles_changed: bool,
    /// The numbering definitions or instances changed.
    pub numbering_changed: bool,
    /// A section's page setup or start kind changed, or sections were added or removed.
    pub page_setup_changed: bool,
    /// Headers, footers, the title-page or even-and-odd settings, or page numbering changed.
    pub furniture_changed: bool,
    /// The footnotes' contents changed.
    pub footnotes_changed: bool,
    /// Anything may have changed: lay the whole document out afresh.
    pub everything: bool,
}

impl Dirty {
    /// Everything: a full layout, trusting nothing from before.
    #[must_use]
    pub const fn all() -> Self {
        Self {
            edits: Vec::new(),
            styles_changed: true,
            numbering_changed: true,
            page_setup_changed: true,
            furniture_changed: true,
            footnotes_changed: true,
            everything: true,
        }
    }

    /// Only these block edits (none: nothing that affects the layout changed).
    #[must_use]
    pub const fn edits(edits: Vec<BlockEdit>) -> Self {
        Self {
            edits,
            styles_changed: false,
            numbering_changed: false,
            page_setup_changed: false,
            furniture_changed: false,
            footnotes_changed: false,
            everything: false,
        }
    }

    /// Whether every block must be placed again from the top.
    #[must_use]
    pub const fn replaces_all(&self) -> bool {
        self.everything
            || self.styles_changed
            || self.numbering_changed
            || self.page_setup_changed
            || self.footnotes_changed
    }

    /// Whether a block measured before may have been measured differently now.
    #[must_use]
    pub const fn invalidates_measures(&self) -> bool {
        self.everything || self.styles_changed || self.numbering_changed
    }

    /// Checks the edits against the block counts before and after, per section.
    pub fn check(&self, before: &[usize], after: &[usize]) -> Result<(), LayoutError> {
        if self.everything {
            return Ok(());
        }
        if before.len() != after.len() {
            return if self.page_setup_changed {
                Ok(())
            } else {
                Err(LayoutError::DirtyMismatch)
            };
        }
        let mut counts = before.to_vec();
        let mut previous: Option<&BlockEdit> = None;
        for edit in &self.edits {
            let in_order =
                previous.is_none_or(|p| (p.section, p.at + p.removed) <= (edit.section, edit.at));
            let fits = before
                .get(edit.section)
                .is_some_and(|n| edit.at + edit.removed <= *n);
            if !in_order || !fits || (edit.removed == 0 && edit.inserted == 0) {
                return Err(LayoutError::DirtyMismatch);
            }
            counts[edit.section] = counts[edit.section] - edit.removed + edit.inserted;
            previous = Some(edit);
        }
        if counts == after {
            Ok(())
        } else {
            Err(LayoutError::DirtyMismatch)
        }
    }
}
