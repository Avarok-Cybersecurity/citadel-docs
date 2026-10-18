//! The editor: a replica of the document with its model view, layout, line maps, selection,
//! undo history and composition, driven by commands and remote updates.

mod finish;
mod layout;
mod nav;
mod remote;
mod undo;
mod view;

use crate::commands::{self, Command, Ctx};
use crate::error::EditError;
use crate::geometry::Geometry;
use crate::history::History;
use crate::ime::Composition;
use crate::outline::Outline;
use crate::position::{caret_gap, is_caret_gap, Position, Selection};
use crate::tx::Tx;
use cdoc_crdt::{from_model, to_model, ClientId, Doc};
use cdoc_layout::{Dirty, FontPort, LayoutOptions, LayoutSession};
use cdoc_model::{Document, RunProperties};
use std::borrow::Cow;

use finish::Parts;
pub use remote::{PeerView, RemoteApplied};

/// What a local command did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Applied {
    /// The update to send to the relay; empty when the document did not change.
    pub update: Vec<u8>,
    /// Pages whose display list must be repainted, ascending.
    pub dirty_pages: Vec<usize>,
    /// The selection after the command.
    pub selection: Selection,
    /// Pages in the document now.
    pub page_count: usize,
}

/// An editing session over one document.
pub struct Editor<'f> {
    pub(crate) doc: Doc,
    pub(crate) model: Document,
    pub(crate) outline: Outline,
    pub(crate) session: LayoutSession<'f>,
    pub(crate) fonts: &'f dyn FontPort,
    pub(crate) geometry: Geometry,
    pub(crate) selection: Selection,
    pub(crate) pending: Option<RunProperties>,
    pub(crate) goal_x: Option<cdoc_layout::Pt100>,
    pub(crate) history: History,
    pub(crate) composition: Option<Composition>,
    /// The layout lost its baseline (a failed relayout): the next one starts afresh.
    pub(crate) relayout_all: bool,
}

impl std::fmt::Debug for Editor<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Editor")
            .field("client", &self.doc.client())
            .field("selection", &self.selection)
            .field("pages", &self.geometry.pages.len())
            .finish_non_exhaustive()
    }
}

impl<'f> Editor<'f> {
    /// Opens a snapshot (`Doc::encode_snapshot` bytes) as `client`, laid out with `fonts` and
    /// `options`.
    pub fn open(
        snapshot: &[u8],
        client: ClientId,
        fonts: &'f dyn FontPort,
        options: &'f LayoutOptions,
    ) -> Result<Self, EditError> {
        Self::start(Doc::decode_snapshot(snapshot, client)?, fonts, options)
    }

    /// A new blank document (`Document::blank()`) written by `client`. Peers must open its
    /// snapshot rather than create their own blank: two blanks are two documents.
    pub fn blank(
        client: ClientId,
        fonts: &'f dyn FontPort,
        options: &'f LayoutOptions,
    ) -> Result<Self, EditError> {
        Self::from_document(&Document::blank(), client, fonts, options)
    }

    /// A new document holding `document`, written by `client`.
    pub fn from_document(
        document: &Document,
        client: ClientId,
        fonts: &'f dyn FontPort,
        options: &'f LayoutOptions,
    ) -> Result<Self, EditError> {
        Self::start(from_model(document, client)?, fonts, options)
    }

    fn start(
        doc: Doc,
        fonts: &'f dyn FontPort,
        options: &'f LayoutOptions,
    ) -> Result<Self, EditError> {
        let model = to_model(&doc)?;
        let outline = Outline::build(&doc.structure());
        let first = caret_gap(&doc, 0)?;
        let at = Position::at(&doc, first).ok_or(EditError::UnknownPosition)?;
        let mut editor = Self {
            doc,
            model,
            outline,
            session: LayoutSession::new(fonts, options),
            fonts,
            geometry: Geometry::default(),
            selection: Selection::caret(at),
            pending: None,
            goal_x: None,
            history: History::default(),
            composition: None,
            relayout_all: true,
        };
        editor.relayout(&Dirty::all())?;
        Ok(editor)
    }

    /// Runs `command` as one transaction.
    pub fn apply(&mut self, command: Command) -> Result<Applied, EditError> {
        self.transact(|ctx| commands::run(ctx, command))
    }

    /// Runs `f` as one transaction: the selection, undo history, model and layout follow.
    pub(crate) fn transact(
        &mut self,
        f: impl FnOnce(&mut Ctx<'_, '_>) -> Result<commands::Outcome, EditError>,
    ) -> Result<Applied, EditError> {
        let sv = self.doc.state_vector();
        let before = self.selection;
        let mut ctx = Ctx {
            tx: Tx::new(&mut self.doc),
            model: &self.model,
            outline: Cow::Borrowed(&self.outline),
            selection: self.selection,
            pending: self.pending.clone(),
        };
        let result = f(&mut ctx);
        let parts = Parts::take(ctx);
        let steps = parts.steps.clone();
        let changed = parts.changed(&self.doc, &sv);
        let dirty_pages = self.finish(parts)?;
        let outcome = result?;
        let selection = self.normalise(outcome.selection)?;
        let moved = selection != before;
        self.history.record(steps, before, selection, outcome.group);
        if moved || outcome.pending.is_some() {
            self.pending = outcome.pending;
        }
        if moved {
            self.goal_x = None;
        }
        self.selection = selection;
        Ok(self.applied(changed.then_some(&sv), dirty_pages))
    }

    /// The record of a transaction; `since` is the state vector before it, when it changed
    /// the document (a deletion alone leaves the state vector as it was).
    pub(crate) fn applied(
        &self,
        since: Option<&cdoc_crdt::StateVector>,
        dirty_pages: Vec<usize>,
    ) -> Applied {
        Applied {
            update: since.map_or_else(Vec::new, |sv| self.doc.encode_diff(sv)),
            dirty_pages,
            selection: self.selection,
            page_count: self.geometry.pages.len(),
        }
    }

    /// `s` with each end moved to the nearest caret gap.
    pub(crate) fn normalise(&self, s: Selection) -> Result<Selection, EditError> {
        let fix = |p: Position| -> Result<Position, EditError> {
            let g = p.index(&self.doc)?;
            if is_caret_gap(&self.doc, g) {
                return Ok(p);
            }
            Position::at(&self.doc, caret_gap(&self.doc, g)?).ok_or(EditError::UnknownPosition)
        };
        Ok(Selection {
            anchor: fix(s.anchor)?,
            head: fix(s.head)?,
        })
    }
}
