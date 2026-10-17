//! Local edits by visible index. Every edit is an operation stamped with this replica's next
//! clock; [`Doc::encode_diff`] ships it.

use super::{Doc, EditError, Entry};
use crate::attrs::{AttrKey, AttrWrite};
use crate::id::Id;
use crate::item::{AtomKind, Content, Item};
use crate::marks::{Anchor, MarkKey, MarkOp};
use std::ops::Range;

pub(crate) enum Body<'a> {
    Text(&'a str),
    Atom(AtomKind, Vec<u8>),
}

impl Doc {
    fn check(&self, index: usize, inclusive: bool) -> Result<u64, EditError> {
        let len = self.len();
        if index > len || (!inclusive && index == len) {
            return Err(EditError::OutOfRange { index, len });
        }
        Ok(index as u64)
    }

    /// The items either side of the gap before visible unit `index`.
    fn gap(&mut self, index: u64) -> (Option<u32>, Option<u32>) {
        if index == 0 {
            return (None, self.seq.first());
        }
        let (idx, off) = self.seq.find(index - 1).expect("index checked");
        if off + 1 < u64::from(self.seq.item(idx).len) {
            let right = self.split(idx, off + 1);
            (Some(idx), Some(right))
        } else {
            (Some(idx), self.seq.next(idx))
        }
    }

    /// The id of visible unit `index`.
    pub(crate) fn unit_id(&self, index: u64) -> Id {
        let (idx, off) = self.seq.find(index).expect("index checked");
        self.seq.item(idx).id.offset(off)
    }

    /// Appends to the text buffer; the new bytes' start and length.
    pub(crate) fn push_text(&mut self, text: &str) -> Result<(u32, u32), EditError> {
        let start = u32::try_from(self.text.len()).map_err(|_| EditError::TooLarge)?;
        let bytes = u32::try_from(text.len()).map_err(|_| EditError::TooLarge)?;
        start.checked_add(bytes).ok_or(EditError::TooLarge)?;
        self.text.push_str(text);
        Ok((start, bytes))
    }

    /// Inserts `text` before visible unit `index` (`index == len()` appends).
    pub fn insert_text(&mut self, index: usize, text: &str) -> Result<(), EditError> {
        if text.is_empty() {
            return Err(EditError::Empty);
        }
        let at = self.check(index, true)?;
        self.insert_body(at, Body::Text(text))
    }

    pub(crate) fn insert_atom_raw(
        &mut self,
        index: usize,
        kind: AtomKind,
        payload: Vec<u8>,
    ) -> Result<(), EditError> {
        let at = self.check(index, true)?;
        self.insert_body(at, Body::Atom(kind, payload))
    }

    fn insert_body(&mut self, at: u64, body: Body<'_>) -> Result<(), EditError> {
        let (left, right) = self.gap(at);
        let origin_left = left.map(|l| self.seq.item(l).last_id());
        let origin_right = right.map(|r| self.seq.item(r).id);
        let id = Id::new(self.client, self.next_clock(self.client));
        let (len, content) = match body {
            Body::Text(text) => {
                let chars = u32::try_from(text.chars().count()).map_err(|_| EditError::TooLarge)?;
                if let Some(l) = left {
                    if self.try_extend(l, id, origin_right, text)? {
                        return Ok(());
                    }
                }
                let (start, bytes) = self.push_text(text)?;
                (chars, Content::Text { start, bytes })
            }
            Body::Atom(kind, payload) => {
                let index = u32::try_from(self.atoms.len()).map_err(|_| EditError::TooLarge)?;
                self.atoms.push(payload);
                (1, Content::Atom { index, kind })
            }
        };
        let item = Item::new(id, len, origin_left, origin_right, content);
        let idx = self.seq.insert_after(left, item);
        self.add_entry(id, Entry::Item(idx), u64::from(len));
        Ok(())
    }

    /// Grows item `left` by `text` when the new units would be exactly its continuation: same
    /// client, next clock, same right origin, and its bytes end the text buffer. This is the
    /// run-length optimisation: sequential typing stays one item.
    pub(crate) fn try_extend(
        &mut self,
        left: u32,
        id: Id,
        origin_right: Option<Id>,
        text: &str,
    ) -> Result<bool, EditError> {
        let item = self.seq.item(left);
        let fits = item.id.client == id.client
            && item.end() == id.clock
            && item.origin_right == origin_right
            && matches!(item.content, Content::Text { start, bytes }
                if (start + bytes) as usize == self.text.len());
        if !fits {
            return Ok(false);
        }
        let chars = u32::try_from(text.chars().count()).map_err(|_| EditError::TooLarge)?;
        let (_, added) = self.push_text(text)?;
        self.seq.modify(left, |i| {
            i.len += chars;
            if let Content::Text { bytes, .. } = &mut i.content {
                *bytes += added;
            }
        });
        let log = self
            .logs
            .get_mut(&id.client)
            .expect("the item's client has a log");
        log.next = log.next.max(id.clock + u64::from(chars));
        Ok(true)
    }

    /// Deletes `len` visible units starting at `index`.
    ///
    /// # Panics
    /// Never: the range is checked before the sequence is searched.
    pub fn delete(&mut self, index: usize, len: usize) -> Result<(), EditError> {
        if len == 0 {
            return Err(EditError::Empty);
        }
        let at = self.check(index, false)?;
        self.check(index + len, true)?;
        let mut left = len as u64;
        while left > 0 {
            let (mut idx, off) = self.seq.find(at).expect("range checked");
            if off > 0 {
                idx = self.split(idx, off);
            }
            let item_len = u64::from(self.seq.item(idx).len);
            if item_len > left {
                self.split(idx, left);
            }
            left -= item_len.min(left);
            self.erase(idx);
        }
        Ok(())
    }

    pub(crate) fn mark_raw(
        &mut self,
        range: Range<usize>,
        key: MarkKey,
        value: Option<Vec<u8>>,
    ) -> Result<(), EditError> {
        if range.start >= range.end {
            return Err(EditError::Empty);
        }
        let start = self.check(range.start, false)?;
        let end = self.check(range.end, true)?;
        let start = Anchor::Before(self.unit_id(start));
        let end = if !key.expands() {
            Anchor::After(self.unit_id(end - 1))
        } else if end < self.seq.total().len {
            Anchor::Before(self.unit_id(end))
        } else {
            Anchor::End
        };
        let id = Id::new(self.client, self.next_clock(self.client));
        self.lamport += 1;
        let index = u32::try_from(self.marks.len()).map_err(|_| EditError::TooLarge)?;
        self.marks.push(MarkOp {
            id,
            lamport: self.lamport,
            start,
            end,
            key,
            value,
        });
        self.add_entry(id, Entry::Mark(index), 1);
        Ok(())
    }

    pub(crate) fn write_attr(&mut self, key: &AttrKey, value: Vec<u8>) {
        let id = Id::new(self.client, self.next_clock(self.client));
        let deps = self.state_vector();
        let k = self.attrs.intern(key);
        self.attrs.write(k, AttrWrite { id, deps, value });
        self.add_entry(id, Entry::Attr(k), 1);
    }

    /// The id and kind of the structural atom at visible unit `index`.
    pub(crate) fn structural_at(&self, index: usize) -> Result<(Id, AtomKind), EditError> {
        let at = self.check(index, false)?;
        let (idx, _) = self.seq.find(at).expect("index checked");
        let item = self.seq.item(idx);
        match item.content {
            Content::Atom { kind, .. } if kind.is_structural() => Ok((item.id, kind)),
            _ => Err(EditError::NotStructural { index }),
        }
    }
}
