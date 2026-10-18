//! Local undo and redo. An entry is the steps of one command (or of a run of typing), with the
//! selection before and after. Grouping uses no clock: consecutive typing of the same class
//! (word characters, or anything else) at the caret where the last typing ended joins one
//! entry; any other command, a caret move or [`History::break_group`] ends the group. Remote
//! updates never enter the history; undo inverts this replica's own steps through the CRDT,
//! so concurrent remote text survives it.

use crate::commands::Group;
use crate::position::Selection;
use crate::tx::Step;
use cdoc_crdt::Id;
use std::collections::BTreeMap;

/// One undoable change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Entry {
    pub(crate) steps: Vec<Step>,
    pub(crate) before: Selection,
    pub(crate) after: Selection,
    pub(crate) group: Group,
}

/// The undo and redo stacks.
#[derive(Debug, Clone, Default)]
pub(crate) struct History {
    pub(crate) undo: Vec<Entry>,
    pub(crate) redo: Vec<Entry>,
    /// Whether the next typing may join the last entry.
    open: bool,
}

fn join(into: &mut Vec<Step>, steps: Vec<Step>) {
    for step in steps {
        match (into.last_mut(), step) {
            (Some(Step::Inserted { runs }), Step::Inserted { runs: more }) => {
                for (first, len) in more {
                    crate::tx::add_run(runs, first, len);
                }
            }
            (_, step) => into.push(step),
        }
    }
}

fn map_id(map: &BTreeMap<Id, Id>, id: &mut Id) {
    if let Some(new) = map.get(id) {
        *id = *new;
    }
}

pub(crate) fn map_selection(map: &BTreeMap<Id, Id>, s: &mut Selection) {
    map_id(map, &mut s.anchor.after);
    map_id(map, &mut s.head.after);
}

pub(crate) fn map_step(map: &BTreeMap<Id, Id>, step: &mut Step) {
    match step {
        Step::Inserted { runs } => {
            let mut out = Vec::new();
            for &(first, len) in runs.iter() {
                for k in 0..len {
                    let mut id = Id::new(first.client, first.clock + k);
                    map_id(map, &mut id);
                    crate::tx::add_run(&mut out, id, 1);
                }
            }
            *runs = out;
        }
        Step::Deleted { after, ids, .. } => {
            map_id(map, after);
            for id in ids.iter_mut() {
                map_id(map, id);
            }
        }
        Step::Formatted { segments, .. } => {
            for s in segments {
                map_id(map, &mut s.first);
                map_id(map, &mut s.last);
            }
        }
        Step::Block { atom, .. } => map_id(map, atom),
        Step::Attr { .. } => {}
    }
}

impl History {
    /// Records a command's steps (nothing when it changed nothing).
    pub(crate) fn record(
        &mut self,
        steps: Vec<Step>,
        before: Selection,
        after: Selection,
        group: Group,
    ) {
        if steps.is_empty() {
            self.open = false;
            return;
        }
        self.redo.clear();
        let joins = self.open
            && matches!(group, Group::Typing { .. })
            && self
                .undo
                .last()
                .is_some_and(|last| last.group == group && last.after == before);
        if joins {
            let last = self.undo.last_mut().expect("checked above");
            join(&mut last.steps, steps);
            last.after = after;
        } else {
            self.undo.push(Entry {
                steps,
                before,
                after,
                group,
            });
        }
        self.open = matches!(group, Group::Typing { .. });
    }

    /// Points every entry at the copies undo made of deleted units, so older entries undo
    /// and redo the copies.
    pub(crate) fn remap(&mut self, pairs: &[(Id, Id)]) {
        if pairs.is_empty() {
            return;
        }
        let map: BTreeMap<Id, Id> = pairs.iter().copied().collect();
        for entry in self.undo.iter_mut().chain(self.redo.iter_mut()) {
            entry.steps.iter_mut().for_each(|s| map_step(&map, s));
            map_selection(&map, &mut entry.before);
            map_selection(&map, &mut entry.after);
        }
    }

    /// Ends the current typing group.
    pub(crate) fn break_group(&mut self) {
        self.open = false;
    }
}
