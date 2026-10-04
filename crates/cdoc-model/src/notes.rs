//! Footnotes and comments: content that is referenced from the body but lives beside it.

use crate::block::Block;
use crate::ids::{CommentId, FootnoteId};
use serde::{Deserialize, Serialize};

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
