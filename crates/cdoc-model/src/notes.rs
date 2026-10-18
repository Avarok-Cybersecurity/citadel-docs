//! Footnotes and comments: content that is referenced from the body but lives beside it.

use crate::block::Block;
use crate::ids::{CommentId, FootnoteId};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// A footnote body. The reference mark is an [`crate::InlineContent::FootnoteRef`] in the body.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Footnote {
    /// The id the reference mark names.
    pub id: FootnoteId,
    /// Content; never empty.
    pub blocks: Vec<Block>,
}

/// A comment or a reply. Anchors are [`crate::Marks::comments`] on inlines; a reply has no
/// anchor of its own and names its parent. The date is RFC 3339 text from the editor's clock
/// port.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Comment {
    /// The id anchors and replies name.
    pub id: CommentId,
    /// Author display name.
    pub author: String,
    /// When it was written, as RFC 3339.
    pub date: String,
    /// The comment text.
    pub text: String,
    /// Whether the thread is resolved; only meaningful on a root comment.
    pub resolved: bool,
    /// The comment this one replies to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<CommentId>,
}

fn references<'a>(
    blocks: impl Iterator<Item = &'a Block> + 'a,
) -> impl Iterator<Item = FootnoteId> + 'a {
    blocks
        .flat_map(Block::paragraphs)
        .flat_map(|p| &p.inlines)
        .filter_map(|i| match i.content {
            crate::InlineContent::FootnoteRef { id } => Some(id),
            _ => None,
        })
}

impl crate::Document {
    /// The footnotes nothing references, in order: no reference in the body, a header or a
    /// footer, or in a footnote that is itself referenced. The editor keeps such a body (undo
    /// may give its reference back); a file written from the document should not.
    #[must_use]
    pub fn unreferenced_footnotes(&self) -> Vec<FootnoteId> {
        let outside = self
            .blocks()
            .filter(|(c, _)| !matches!(c, crate::visit::Container::Footnote(_)))
            .map(|(_, b)| b);
        let mut named: HashSet<FootnoteId> = references(outside).collect();
        let mut live = vec![false; self.footnotes.len()];
        loop {
            let mut grew = false;
            for (i, f) in self.footnotes.iter().enumerate() {
                if !live[i] && named.contains(&f.id) {
                    live[i] = true;
                    grew = true;
                    named.extend(references(f.blocks.iter()));
                }
            }
            if !grew {
                break;
            }
        }
        self.footnotes
            .iter()
            .zip(live)
            .filter(|(_, live)| !live)
            .map(|(f, _)| f.id)
            .collect()
    }

    /// Removes [`Self::unreferenced_footnotes`] and returns their ids.
    pub fn prune_unreferenced_footnotes(&mut self) -> Vec<FootnoteId> {
        let gone = self.unreferenced_footnotes();
        self.footnotes.retain(|f| !gone.contains(&f.id));
        gone
    }
}
