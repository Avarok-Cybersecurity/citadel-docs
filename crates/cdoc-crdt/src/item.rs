//! Items: runs of the sequence. An item is `len` consecutive units from one client, inserted
//! one after the other (each unit's left origin is the unit before it, all share one right
//! origin), so a typed word or a pasted page is one item until something lands inside it.

use crate::bytes::DecodeError;
use crate::id::Id;

/// What kind of atom a single-unit item is. Every kind but [`AtomKind::Inline`] is a
/// *structural* atom: it opens or closes a block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AtomKind {
    /// An inline object (image, break, tab, footnote reference, field, ...).
    Inline,
    /// The start of a section.
    Section,
    /// The start of a header or footer.
    HeaderFooter,
    /// The start of a footnote body.
    Footnote,
    /// The start of a paragraph.
    Paragraph,
    /// The start of a table.
    Table,
    /// The start of a table row.
    Row,
    /// The start of a table cell.
    Cell,
    /// The end of the innermost open table, header, footer or footnote.
    End,
}

impl AtomKind {
    const ALL: [Self; 9] = [
        Self::Inline,
        Self::Section,
        Self::HeaderFooter,
        Self::Footnote,
        Self::Paragraph,
        Self::Table,
        Self::Row,
        Self::Cell,
        Self::End,
    ];

    /// Whether the atom delimits blocks.
    #[must_use]
    pub const fn is_structural(self) -> bool {
        !matches!(self, Self::Inline)
    }

    pub(crate) const fn code(self) -> u8 {
        self as u8
    }

    pub(crate) fn from_code(code: u8) -> Result<Self, DecodeError> {
        Self::ALL
            .get(usize::from(code))
            .copied()
            .ok_or(DecodeError::BadTag {
                what: "atom kind",
                tag: code,
            })
    }
}

/// What an item holds. A deleted item holds nothing: tombstones keep their ids and origins
/// (concurrent edits may still name them) but drop their content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Content {
    /// `len` characters at `start..start + bytes` of the document's text buffer.
    Text { start: u32, bytes: u32 },
    /// One atom, payload at this index of the document's atom table.
    Atom { index: u32, kind: AtomKind },
    /// Deleted.
    Gone,
}

/// The visible weight of an item: units, and structural atoms among them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Weight {
    pub(crate) len: u64,
    pub(crate) blocks: u64,
}

impl Weight {
    pub(crate) const ZERO: Self = Self { len: 0, blocks: 0 };

    pub(crate) const fn plus(self, o: Self) -> Self {
        Self {
            len: self.len.wrapping_add(o.len),
            blocks: self.blocks.wrapping_add(o.blocks),
        }
    }

    pub(crate) const fn minus(self, o: Self) -> Self {
        Self {
            len: self.len.wrapping_sub(o.len),
            blocks: self.blocks.wrapping_sub(o.blocks),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Item {
    pub(crate) id: Id,
    pub(crate) len: u32,
    pub(crate) origin_left: Option<Id>,
    pub(crate) origin_right: Option<Id>,
    pub(crate) content: Content,
    /// The leaf of the sequence index that holds this item.
    pub(crate) leaf: u32,
}

impl Item {
    pub(crate) const fn new(
        id: Id,
        len: u32,
        origin_left: Option<Id>,
        origin_right: Option<Id>,
        content: Content,
    ) -> Self {
        Self {
            id,
            len,
            origin_left,
            origin_right,
            content,
            leaf: 0,
        }
    }

    pub(crate) const fn end(&self) -> u64 {
        self.id.clock + self.len as u64
    }

    pub(crate) const fn last_id(&self) -> Id {
        self.id.offset(self.len as u64 - 1)
    }

    pub(crate) const fn is_deleted(&self) -> bool {
        matches!(self.content, Content::Gone)
    }

    pub(crate) const fn weight(&self) -> Weight {
        match self.content {
            Content::Gone => Weight::ZERO,
            Content::Text { .. } => Weight {
                len: self.len as u64,
                blocks: 0,
            },
            Content::Atom { kind, .. } => Weight {
                len: 1,
                blocks: kind.is_structural() as u64,
            },
        }
    }
}

/// The byte length of the first `chars` characters of `text`.
pub(crate) fn byte_prefix(text: &str, chars: usize) -> usize {
    if chars == 0 {
        return 0;
    }
    if text.is_ascii() {
        return chars;
    }
    text.char_indices()
        .nth(chars)
        .map_or(text.len(), |(at, _)| at)
}
