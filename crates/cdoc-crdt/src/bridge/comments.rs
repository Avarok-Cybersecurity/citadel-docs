//! The comment list as a map keyed by comment id: adding, editing (resolving) or removing one
//! comment is one register write, so concurrent changes to different comments all survive and
//! concurrent changes to one comment resolve like any attribute.
//!
//! A value is `rank:varint born.client:varint born.clock:varint comment`, where `comment` is
//! the encoded model comment, `born` the id of the write that added it and `rank` the number
//! of operations its writer held then; both are kept by every later edit, and an empty value
//! removes the comment. The list is ordered by `(rank, born)`: a comment added after seeing
//! another ranks after it, so for a document written by [`from_model`](super::from_model) the
//! order is the model's, and concurrent additions order the same way on every replica. A
//! reordering alone is not an edit and is not stored.

use super::BridgeError;
use crate::attrs::AttrKey;
use crate::bytes::{DecodeError, Reader, Writer};
use crate::codec::{from_bytes, to_bytes};
use crate::doc::{Doc, EditError};
use crate::id::{ClientId, Id};
use cdoc_model::Comment;
use std::collections::{BTreeMap, BTreeSet};

/// When a comment was added: its place in the list.
type Born = (u64, Id);

/// `None` for a removal, else when the comment was added and its encoded value.
fn read(value: &[u8]) -> Result<Option<(Born, &[u8])>, DecodeError> {
    if value.is_empty() {
        return Ok(None);
    }
    let mut r = Reader::new(value);
    let rank = r.uvar()?;
    let born = Id::new(ClientId(r.uvar()?), r.uvar()?);
    Ok(Some(((rank, born), &value[r.position()..])))
}

impl Doc {
    /// The comments held: id to (born, encoded comment).
    fn held_comments(&self) -> Result<BTreeMap<&str, (Born, &[u8])>, DecodeError> {
        let mut out = BTreeMap::new();
        for (id, write) in self.attrs.comments() {
            if let Some(held) = read(&write.value)? {
                out.insert(id, held);
            }
        }
        Ok(out)
    }

    /// Makes the comment list `comments`: one write per comment added, changed or removed.
    pub(crate) fn set_comments(&mut self, comments: &[Comment]) -> Result<(), EditError> {
        let mut seen = BTreeSet::new();
        if let Some(dup) = comments.iter().find(|c| !seen.insert(c.id.as_str())) {
            return Err(EditError::DuplicateComment(dup.id.clone()));
        }
        let held: BTreeMap<String, (Born, Vec<u8>)> = self
            .held_comments()
            .map_err(|e| EditError::Codec(e.into()))?
            .into_iter()
            .map(|(id, (born, bytes))| (id.to_owned(), (born, bytes.to_vec())))
            .collect();
        for c in comments {
            let bytes = to_bytes(c)?;
            let born = match held.get(c.id.as_str()) {
                Some((_, old)) if *old == bytes => continue,
                Some((born, _)) => *born,
                None => {
                    let held = self.state_vector().iter().map(|(_, n)| n).sum::<u64>();
                    (held, Id::new(self.client, self.next_clock(self.client)))
                }
            };
            let (rank, id) = born;
            let mut w = Writer::new();
            w.uvar(rank);
            w.uvar(id.client.0);
            w.uvar(id.clock);
            w.raw(&bytes);
            self.write_attr(&AttrKey::Comment(c.id.as_str().to_owned()), w.finish());
        }
        for id in held.keys().filter(|id| !seen.contains(id.as_str())) {
            // An empty value is a removal.
            self.write_attr(&AttrKey::Comment(id.clone()), Vec::new());
        }
        Ok(())
    }

    /// The comment list, in the order the comments were added.
    pub(crate) fn comment_list(&self) -> Result<Vec<Comment>, BridgeError> {
        let mut held: Vec<(Born, &[u8])> = self
            .held_comments()
            .map_err(|e| BridgeError::Codec(e.into()))?
            .into_values()
            .collect();
        held.sort_by_key(|&(born, _)| born);
        held.into_iter()
            .map(|(_, bytes)| Ok(from_bytes(bytes)?))
            .collect()
    }
}
