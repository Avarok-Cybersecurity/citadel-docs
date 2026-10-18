//! Tombstone collection, only when the caller asks: the engine cannot know which peers are
//! caught up, the collaboration session does (it calls this right before storing a snapshot).

use super::Editor;
use crate::error::EditError;
use crate::history::History;
use crate::position::{Position, Selection};
use cdoc_crdt::{Collected, StateVector};

impl Editor<'_> {
    /// Collects the deleted units and delete operations every peer in `known` (each one's
    /// acknowledged state vector) has seen; see `cdoc_crdt::Doc::gc` for when that is safe.
    /// The document and the selection stay as they were. The undo history ends here: undoing
    /// a deletion puts the text back after units that collection may have removed. Refused
    /// while a composition is open.
    pub fn gc(&mut self, known: &[StateVector]) -> Result<Collected, EditError> {
        if self.composition.is_some() {
            return Err(EditError::NotApplicable("a composition is open"));
        }
        let anchor = self.selection.anchor.index(&self.doc)?;
        let head = self.selection.head.index(&self.doc)?;
        let collected = self.doc.gc(known)?;
        let at = |g| Position::at(&self.doc, g).ok_or(EditError::UnknownPosition);
        self.selection = Selection {
            anchor: at(anchor)?,
            head: at(head)?,
        };
        self.history = History::default();
        Ok(collected)
    }
}
