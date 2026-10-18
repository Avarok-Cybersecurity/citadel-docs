//! Undo and redo: a history entry's inverse steps replayed as a new transaction.

use super::{Applied, Editor, Parts};
use crate::commands::Group;
use crate::error::EditError;
use crate::history::{map_selection, map_step, Entry};
use crate::restore::invert;
use crate::tx::Tx;

impl Editor<'_> {
    /// Undoes the last local change; `None` when there is nothing to undo.
    pub fn undo(&mut self) -> Result<Option<Applied>, EditError> {
        self.replay(true)
    }

    /// Redoes the last undone change; `None` when there is nothing to redo.
    pub fn redo(&mut self) -> Result<Option<Applied>, EditError> {
        self.replay(false)
    }

    fn replay(&mut self, undo: bool) -> Result<Option<Applied>, EditError> {
        let stack = if undo {
            &mut self.history.undo
        } else {
            &mut self.history.redo
        };
        let Some(entry) = stack.pop() else {
            return Ok(None);
        };
        let sv = self.doc.state_vector();
        let mut tx = Tx::new(&mut self.doc);
        let mut failure = None;
        for step in entry.steps.iter().rev() {
            // Steps later in this replay may name units an earlier one just restored as copies.
            let mut step = step.clone();
            map_step(&tx.remap.iter().copied().collect(), &mut step);
            if let Err(e) = invert(&mut tx, &step, &self.model) {
                failure = Some(e);
                break;
            }
        }
        let remap = std::mem::take(&mut tx.remap);
        let parts = Parts {
            steps: tx.steps,
            touched: tx.touched,
            attrs: tx.attrs,
            structural: tx.structural,
            outline: None,
        };
        let steps = parts.steps.clone();
        let changed = parts.changed(&self.doc, &sv);
        let pages = self.finish(parts)?;
        if let Some(e) = failure {
            return Err(e);
        }
        self.history.remap(&remap);
        let mut entry = entry;
        let map = remap.iter().copied().collect();
        map_selection(&map, &mut entry.before);
        map_selection(&map, &mut entry.after);
        self.selection = self.normalise(entry.before)?;
        self.pending = None;
        self.goal_x = None;
        self.history.break_group();
        let back = Entry {
            steps,
            before: entry.after,
            after: entry.before,
            group: Group::Own,
        };
        if undo {
            self.history.redo.push(back);
        } else {
            self.history.undo.push(back);
        }
        Ok(Some(self.applied(changed.then_some(&sv), pages)))
    }

    /// Ends the current undo group (the next typing starts a new undo step).
    pub fn break_undo_group(&mut self) {
        self.history.break_group();
    }
}
