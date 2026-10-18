//! After a transaction: the touched blocks re-read into the model, the outline rebuilt when
//! the structure changed, and the layout told exactly what is dirty.

use super::Editor;
use crate::commands::Ctx;
use crate::error::EditError;
use crate::outline::Outline;
use crate::sync::{ordinals, sync};
use crate::tx::{Step, Tx};
use cdoc_crdt::Id;
use std::borrow::Cow;

impl Editor<'_> {
    /// Brings model, outline, layout and line maps up to date after a transaction.
    pub(super) fn finish(&mut self, parts: Parts) -> Result<Vec<usize>, EditError> {
        if parts.steps.is_empty() {
            return Ok(Vec::new());
        }
        let new = match (parts.structural, parts.outline) {
            (true, _) => Some(Outline::build(&self.doc.structure())),
            (false, owned) => owned,
        };
        let ranges = ordinals(&self.doc, &parts.touched);
        let attrs: Vec<String> = parts.attrs.iter().map(|a| (*a).to_owned()).collect();
        let dirty = sync(
            &self.doc,
            &mut self.model,
            &self.outline,
            new.as_ref().unwrap_or(&self.outline),
            &ranges,
            &attrs,
        )?;
        if let Some(new) = new {
            self.outline = new;
        }
        self.relayout(&dirty)
    }
}

/// What a transaction leaves for [`Editor::finish`].
pub(super) struct Parts {
    pub(super) steps: Vec<Step>,
    pub(super) touched: Vec<(Id, Id)>,
    pub(super) attrs: Vec<&'static str>,
    pub(super) structural: bool,
    pub(super) outline: Option<Outline>,
}

impl Parts {
    pub(super) fn take(ctx: Ctx<'_, '_>) -> Self {
        let outline = match ctx.outline {
            Cow::Owned(o) => Some(o),
            Cow::Borrowed(_) => None,
        };
        let Tx {
            steps,
            touched,
            attrs,
            structural,
            ..
        } = ctx.tx;
        Self {
            steps,
            touched,
            attrs,
            structural,
            outline,
        }
    }

    pub(super) fn changed(&self, doc: &cdoc_crdt::Doc, before: &cdoc_crdt::StateVector) -> bool {
        !self.touched.is_empty() || !self.attrs.is_empty() || doc.state_vector() != *before
    }
}
