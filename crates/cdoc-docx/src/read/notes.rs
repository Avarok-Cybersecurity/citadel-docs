//! Footnotes and comments.

use super::block::bare_paragraph;
use super::Reader;
use crate::error::DocxError;
use crate::package::rel_type;
use crate::xml::{self, El};
use cdoc_model::{Comment, CommentId, Footnote, FootnoteId};

/// The text of a comment paragraph: runs' text, with tabs and breaks as characters.
fn paragraph_text(p: El<'_>, plain: &mut bool) -> String {
    let mut out = String::new();
    for run in p.elems().filter(|n| n.name() == "w:r") {
        if run
            .find("w:rPr")
            .is_some_and(|r| r.elems().any(|e| e.name() != "w:rStyle"))
        {
            *plain = false;
        }
        for item in run.elems() {
            match item.name() {
                "w:t" => out.push_str(&item.content()),
                "w:tab" => out.push('\t'),
                "w:br" | "w:cr" => out.push('\n'),
                _ => {}
            }
        }
    }
    out
}

impl Reader<'_> {
    pub(super) fn footnotes(&mut self) -> Result<Vec<Footnote>, DocxError> {
        let Some(name) = self.target_of(rel_type::FOOTNOTES) else {
            return Ok(Vec::new());
        };
        let tree = self.load(&name)?;
        self.in_part(&name, |r| {
            let mut notes = Vec::new();
            for note in tree.root().elems() {
                if note.name() != "w:footnote" {
                    r.log.unknown(&format!("w:footnotes/{}", note.name()));
                    continue;
                }
                if matches!(
                    note.get("w:type"),
                    Some("separator" | "continuationSeparator" | "continuationNotice")
                ) {
                    continue;
                }
                let Some(id) = note.get("w:id").and_then(|n| n.parse().ok()) else {
                    r.log.lose("a footnote without a usable id");
                    continue;
                };
                let mut blocks = r.container_blocks(note)?;
                if blocks.is_empty() {
                    blocks.push(bare_paragraph());
                }
                notes.push(Footnote {
                    id: FootnoteId(id),
                    blocks,
                });
            }
            Ok(notes)
        })
    }

    pub(super) fn comments(&mut self) -> Result<Vec<Comment>, DocxError> {
        let Some(name) = self.target_of(rel_type::COMMENTS) else {
            return Ok(Vec::new());
        };
        let ext = match self.target_of(rel_type::COMMENTS_EXTENDED) {
            Some(n) => Some(self.load(&n)?),
            None => None,
        };
        let tree = self.load(&name)?;
        self.in_part(&name, |r| Ok(r.read_comments(tree.root(), ext.as_ref().map(xml::Tree::root))))
    }

    fn read_comments(&mut self, root: El<'_>, ext: Option<El<'_>>) -> Vec<Comment> {
        let mut comments = Vec::new();
        let mut para_ids: Vec<Vec<String>> = Vec::new();
        for c in root.elems() {
            if c.name() != "w:comment" {
                self.log.unknown(&format!("w:comments/{}", c.name()));
                continue;
            }
            let n: usize = c.get("w:id").and_then(|n| n.parse().ok()).unwrap_or(0);
            let mut plain = true;
            let paragraphs: Vec<El<'_>> = c.elems().filter(|p| p.name() == "w:p").collect();
            let text = paragraphs
                .iter()
                .map(|p| paragraph_text(*p, &mut plain))
                .collect::<Vec<_>>()
                .join("\n");
            if !plain {
                self.log.lose("comment text formatting");
            }
            para_ids.push(
                paragraphs
                    .iter()
                    .filter_map(|p| p.get("w14:paraId").map(str::to_owned))
                    .collect(),
            );
            comments.push(Comment {
                id: CommentId::new(self.ids.comments.name(n)),
                author: c.get("w:author").unwrap_or_default().to_owned(),
                date: c.get("w:date").unwrap_or_default().to_owned(),
                text,
                resolved: false,
                parent: None,
            });
        }
        let Some(ext) = ext else { return comments };
        for entry in ext.elems().filter(|e| e.name() == "w15:commentEx") {
            let Some(para) = entry.get("w15:paraId") else {
                continue;
            };
            let Some(at) = para_ids
                .iter()
                .position(|ids| ids.iter().any(|p| p == para))
            else {
                continue;
            };
            comments[at].resolved = entry.get("w15:done") == Some("1");
            if let Some(parent) = entry.get("w15:paraIdParent") {
                comments[at].parent = para_ids
                    .iter()
                    .position(|ids| ids.iter().any(|p| p == parent))
                    .map(|i| comments[i].id.clone());
            }
        }
        comments
    }
}
