//! Building a [`Tree`] from a tokeniser's events, without recursion and without an allocation
//! per element.

use super::tree::{Item, Slot, Tree, NONE};
use super::Whitespace;
use std::borrow::Cow;

/// Up to this many attributes a new one is checked against the others directly; past it the
/// element's keys are sorted once when it opens, so many attributes cost n log n, not n².
const LINEAR_ATTRS: usize = 16;

/// An element being filled: its slot, its last child, and what its children are so far.
struct Open {
    at: u32,
    last: u32,
    elems: bool,
    blank: bool,
}

/// Builds a [`Tree`] from a tokeniser's events; the shape rules (text merging, layout
/// whitespace) live here so every tokeniser gets them the same.
pub(super) struct Builder<'a> {
    tree: Tree<'a>,
    stack: Vec<Open>,
    whitespace: Whitespace,
    max_depth: usize,
    done: bool,
}

/// Why the events do not make a tree.
pub(super) enum Shape {
    /// An end with no start, or content after the document element.
    Unbalanced,
    /// More nodes than an index can name.
    TooMany,
    /// Elements nested deeper than the limit.
    TooDeep,
}

impl<'a> Builder<'a> {
    /// A builder sized for a part of `len` bytes, refusing nesting deeper than `max_depth`.
    pub(super) fn new(len: usize, whitespace: Whitespace, max_depth: usize) -> Self {
        Self {
            tree: Tree {
                slots: Vec::with_capacity(len / 48),
                attrs: Vec::with_capacity(len / 64),
            },
            stack: Vec::new(),
            whitespace,
            max_depth,
            done: false,
        }
    }

    fn push(&mut self, item: Item<'a>) -> Result<u32, Shape> {
        let at = u32::try_from(self.tree.slots.len()).map_err(|_| Shape::TooMany)?;
        if at == NONE {
            return Err(Shape::TooMany);
        }
        self.tree.slots.push(Slot {
            item,
            first: NONE,
            next: NONE,
        });
        if let Some(parent) = self.stack.last_mut() {
            if parent.last == NONE {
                self.tree.slots[parent.at as usize].first = at;
            } else {
                self.tree.slots[parent.last as usize].next = at;
            }
            parent.last = at;
        }
        Ok(at)
    }

    /// An attribute of the element about to be opened.
    pub(super) fn attr(&mut self, key: &'a str, value: Cow<'a, str>) {
        self.tree.attrs.push((key, value));
    }

    /// Open an element holding the attributes given since the last open.
    pub(super) fn open(&mut self, name: &'a str, first_attr: usize) -> Result<(), Shape> {
        if self.done {
            return Err(Shape::Unbalanced);
        }
        if self.stack.len() >= self.max_depth {
            return Err(Shape::TooDeep);
        }
        let end = u32::try_from(self.tree.attrs.len()).map_err(|_| Shape::TooMany)?;
        let start = u32::try_from(first_attr).map_err(|_| Shape::TooMany)?;
        if let Some(parent) = self.stack.last_mut() {
            parent.elems = true;
        }
        let at = self.push(Item::Elem {
            name,
            attrs: (start, end),
        })?;
        self.stack.push(Open {
            at,
            last: NONE,
            elems: false,
            blank: false,
        });
        Ok(())
    }

    /// Whether an attribute named `key` has been given since `mark`; only answered for the
    /// first [`LINEAR_ATTRS`], the rest are left to [`Builder::duplicate_since`].
    pub(super) fn has_attr_since(&self, mark: usize, key: &str) -> bool {
        let given = &self.tree.attrs[mark..];
        given.len() < LINEAR_ATTRS && given.iter().any(|(k, _)| *k == key)
    }

    /// A key given twice since `mark`, for an element with more than [`LINEAR_ATTRS`].
    pub(super) fn duplicate_since(&self, mark: usize) -> Option<&'a str> {
        let given = &self.tree.attrs[mark..];
        if given.len() <= LINEAR_ATTRS {
            return None;
        }
        let mut keys: Vec<&'a str> = given.iter().map(|(k, _)| *k).collect();
        keys.sort_unstable();
        keys.windows(2).find(|w| w[0] == w[1]).map(|w| w[0])
    }

    /// How many attributes have been given so far: pass it to [`Builder::open`].
    pub(super) fn attr_mark(&self) -> usize {
        self.tree.attrs.len()
    }

    /// Character data; merged with the text before it when nothing came between.
    pub(super) fn text(&mut self, text: Cow<'a, str>) -> Result<(), Shape> {
        let Some(top) = self.stack.last_mut() else {
            return Ok(());
        };
        top.blank |= text.trim().is_empty();
        if top.last != NONE {
            if let Item::Text(prev) = &mut self.tree.slots[top.last as usize].item {
                prev.to_mut().push_str(&text);
                return Ok(());
            }
        }
        self.push(Item::Text(text)).map(drop)
    }

    /// Close the innermost open element.
    pub(super) fn close(&mut self) -> Result<(), Shape> {
        let open = self.stack.pop().ok_or(Shape::Unbalanced)?;
        if open.elems && open.blank && self.whitespace == Whitespace::DropBetweenElements {
            self.settle(open.at);
        }
        self.done = self.stack.is_empty();
        Ok(())
    }

    /// Unlink whitespace-only text from an element that contains elements.
    fn settle(&mut self, at: u32) {
        let slots = &mut self.tree.slots;
        let blank = |s: &Slot<'_>| matches!(&s.item, Item::Text(t) if t.trim().is_empty());
        let mut link = slots[at as usize].first;
        while link != NONE && blank(&slots[link as usize]) {
            link = slots[link as usize].next;
        }
        slots[at as usize].first = link;
        while link != NONE {
            let mut next = slots[link as usize].next;
            while next != NONE && blank(&slots[next as usize]) {
                next = slots[next as usize].next;
            }
            slots[link as usize].next = next;
            link = next;
        }
    }

    /// The tree, once the document element has closed.
    pub(super) fn finish(self) -> Option<Tree<'a>> {
        self.done.then_some(self.tree)
    }
}
