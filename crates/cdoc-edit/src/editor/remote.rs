//! Remote collaboration: updates from peers (never entering the local history) and awareness.

use super::Editor;
use crate::awareness::Awareness;
use crate::error::EditError;
use crate::outline::Outline;
use crate::position::Selection;
use crate::sync::sync;
use crate::wire::{Outcome, Remote};
use cdoc_crdt::{Id, PendingLimits};
use cdoc_layout::Rect;

/// What a remote update did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteApplied {
    /// Pages to repaint.
    pub dirty_pages: Vec<usize>,
    /// Units the update's remainder waits for (ask the peer for a diff when non-empty).
    pub missing: Vec<Id>,
    /// Buffered operations dropped to stay within the limits (a later diff sends them again).
    pub dropped: u64,
    /// The local selection, still anchored to the same text.
    pub selection: Selection,
    /// Pages in the document now.
    pub page_count: usize,
}

/// A peer's caret and selection as rectangles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerView {
    /// The peer.
    pub client: cdoc_crdt::ClientId,
    /// The caret, if this replica holds the text it is anchored to.
    pub caret: Option<(usize, Rect)>,
    /// The selection highlight.
    pub rects: Vec<(usize, Rect)>,
}

impl Editor<'_> {
    /// Applies a peer's update, buffering at most `limits` of operations that wait for a
    /// dependency.
    pub fn apply_remote(
        &mut self,
        update: &[u8],
        limits: PendingLimits,
    ) -> Result<RemoteApplied, EditError> {
        let applied = self.doc.apply_update(update, limits)?;
        let mut pages = Vec::new();
        if !applied.changes.is_empty() {
            let new = Outline::build(&self.doc.structure());
            let dirty = sync(
                &self.doc,
                &mut self.model,
                &self.outline,
                &new,
                &applied.changes.blocks,
                &applied.changes.attrs,
            )?;
            self.outline = new;
            pages = self.relayout(&dirty)?;
            self.selection = self.normalise(self.selection)?;
            self.goal_x = None;
        }
        Ok(RemoteApplied {
            dirty_pages: pages,
            missing: applied.missing,
            dropped: applied.dropped,
            selection: self.selection,
            page_count: self.page_count(),
        })
    }

    /// A peer's `applied` as the wire carries it: [`Self::outcome`] with what it left waiting
    /// (the first unit lacked, and what the limits dropped).
    pub fn remote_outcome(&self, applied: RemoteApplied) -> Result<Outcome, EditError> {
        let outcome = self.outcome(Vec::new(), applied.dirty_pages, Vec::new())?;
        Ok(Outcome {
            remote: Some(Remote {
                missing: applied.missing.first().copied(),
                dropped: applied.dropped,
            }),
            ..outcome
        })
    }

    /// This replica's awareness record (its client and selection).
    #[must_use]
    pub fn awareness(&self) -> Vec<u8> {
        Awareness {
            client: self.doc.client(),
            selection: self.selection,
        }
        .encode()
    }

    /// A peer's awareness record as rectangles to draw.
    pub fn peer_view(&self, record: &[u8]) -> Result<PeerView, EditError> {
        let a = Awareness::decode(record)?;
        let caret = self.caret_rect(a.selection.head).ok();
        let rects = self.selection_rects(a.selection).unwrap_or_default();
        Ok(PeerView {
            client: a.client,
            caret,
            rects,
        })
    }
}
