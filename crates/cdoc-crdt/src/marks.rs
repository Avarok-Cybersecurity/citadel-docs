//! Formatting as Peritext-style mark ranges.
//!
//! A mark operation names a range by two *anchors* on units, each on one side of its unit
//! (`Before` or `After`). Because anchors name units by id, not by index, the range moves with
//! the text: a concurrent insertion lands inside or outside it according to which gap it falls
//! in, identically on every replica. A mark that grows when typing at its end (bold) ends
//! `Before` the next unit; one that must not (a link) ends `After` its last unit.
//!
//! For a single-valued key (bold, a link, a revision) the covering operation with the greatest
//! Lamport timestamp (ties broken by client) decides; a value of `None` removes the key. A
//! comment key is multi-valued: each covering operation is one anchoring, in timestamp order,
//! and a removal hides the same comment's earlier anchorings.

use crate::id::Id;

/// One field of `cdoc_model::RunProperties`; each is its own mark key, so concurrent "bold
/// this" and "italicise that" both survive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RunKey {
    /// `style`.
    Style,
    /// `font`.
    Font,
    /// `size`.
    Size,
    /// `bold`.
    Bold,
    /// `italic`.
    Italic,
    /// `underline`.
    Underline,
    /// `strike`.
    Strike,
    /// `vertical_align`.
    VerticalAlign,
    /// `color`.
    Color,
    /// `highlight`.
    Highlight,
    /// `small_caps`.
    SmallCaps,
    /// `all_caps`.
    AllCaps,
    /// `kern`. Codes 12 to 14 are the link, revision and comment keys on the wire, so the
    /// run keys added after them continue at 15.
    Kern = 15,
    /// `ligatures`.
    Ligatures = 16,
    /// `lang`.
    Lang = 17,
}

impl RunKey {
    /// Every key, in field order.
    pub const ALL: [Self; 15] = [
        Self::Style,
        Self::Font,
        Self::Size,
        Self::Bold,
        Self::Italic,
        Self::Underline,
        Self::Strike,
        Self::VerticalAlign,
        Self::Color,
        Self::Highlight,
        Self::SmallCaps,
        Self::AllCaps,
        Self::Kern,
        Self::Ligatures,
        Self::Lang,
    ];

    /// The key whose wire code is `code`.
    #[must_use]
    pub fn from_code(code: u8) -> Option<Self> {
        Self::ALL.into_iter().find(|k| *k as u8 == code)
    }
}

/// Where a range starts or ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Anchor {
    /// Before everything.
    Start,
    /// After everything.
    End,
    /// The gap just before this unit.
    Before(Id),
    /// The gap just after this unit.
    After(Id),
}

impl Anchor {
    pub(crate) const fn id(self) -> Option<Id> {
        match self {
            Self::Before(id) | Self::After(id) => Some(id),
            Self::Start | Self::End => None,
        }
    }
}

/// What a mark sets.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum MarkKey {
    /// A character property.
    Run(RunKey),
    /// The hyperlink.
    Link,
    /// The tracked change.
    Revision,
    /// An anchoring of the comment with this id.
    Comment(String),
}

impl MarkKey {
    /// Whether typing at the end of the range extends it.
    pub(crate) const fn expands(&self) -> bool {
        matches!(self, Self::Run(_))
    }
}

/// A mark operation. `value` is the codec-encoded model value; `None` removes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MarkOp {
    pub(crate) id: Id,
    pub(crate) lamport: u64,
    pub(crate) start: Anchor,
    pub(crate) end: Anchor,
    pub(crate) key: MarkKey,
    pub(crate) value: Option<Vec<u8>>,
}

impl MarkOp {
    /// The total order marks are resolved in.
    pub(crate) const fn stamp(&self) -> (u64, u64) {
        (self.lamport, self.id.client.0)
    }
}
