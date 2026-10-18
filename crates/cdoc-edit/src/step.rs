//! What a transaction records so it can be undone: the inverse of each primitive edit.

use cdoc_crdt::{Atom, DocAttr, Id, Mark, Piece, RunKey};
use cdoc_model::{CommentId, Marks, RunProperties};

/// The formatting of a run of units before an edit changed it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Segment {
    pub(crate) first: Id,
    pub(crate) last: Id,
    /// `None` for an inline atom (only its marks apply).
    pub(crate) props: Option<RunProperties>,
    pub(crate) marks: Marks,
}

/// What a format edit set: the keys its undo restores.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Target {
    Run(Vec<RunKey>),
    Link,
    Revision,
    Comment(CommentId),
}

impl Target {
    pub(crate) fn of(mark: &Mark) -> Self {
        match mark {
            Mark::Run(p) => Self::Run(crate::run_keys::keys(p)),
            Mark::ClearRun(k) => Self::Run(vec![*k]),
            Mark::Link(_) => Self::Link,
            Mark::Revision(_) => Self::Revision,
            Mark::Comment { id, .. } => Self::Comment(id.clone()),
        }
    }
}

/// One recorded edit and what it replaced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Step {
    /// Units inserted, as runs of consecutive ids.
    Inserted { runs: Vec<(Id, u64)> },
    /// Units deleted, as they were: the unit they followed (where they go back), their ids in
    /// order, and their content.
    Deleted {
        after: Id,
        ids: Vec<Id>,
        pieces: Vec<Piece>,
    },
    /// Formatting of `target` replaced; `segments` hold the previous values.
    Formatted {
        target: Target,
        segments: Vec<Segment>,
    },
    /// A block's properties replaced.
    Block { atom: Id, previous: Atom },
    /// A document attribute replaced.
    Attr { previous: DocAttr },
}
