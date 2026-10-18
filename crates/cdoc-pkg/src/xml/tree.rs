//! A parsed part held as one arena over the part's own bytes: names, attribute values and text
//! borrow the input wherever no entity had to be resolved, and every element, attribute and
//! text piece lives in one of two vectors. Reading a part costs no allocation per element, and
//! dropping it costs none per element either.

use super::{Child, Node, Whitespace};
use std::borrow::Cow;

const NONE: u32 = u32::MAX;

#[derive(Debug)]
enum Item<'a> {
    Elem { name: &'a str, attrs: (u32, u32) },
    Text(Cow<'a, str>),
}

#[derive(Debug)]
struct Slot<'a> {
    item: Item<'a>,
    first: u32,
    next: u32,
}

/// A parsed part. [`Tree::root`] is the document element.
#[derive(Debug)]
pub struct Tree<'a> {
    slots: Vec<Slot<'a>>,
    attrs: Vec<(&'a str, Cow<'a, str>)>,
}

/// An element of a [`Tree`]. Copy, so readers pass it by value.
#[derive(Debug, Clone, Copy)]
pub struct El<'t> {
    tree: &'t Tree<'t>,
    at: u32,
}

impl<'a> Tree<'a> {
    /// The document element.
    #[must_use]
    pub fn root(&self) -> El<'_> {
        El { tree: self, at: 0 }
    }

    fn slot(&self, at: u32) -> &Slot<'a> {
        &self.slots[at as usize]
    }
}

impl<'t> El<'t> {
    /// The qualified name, e.g. `w:p`.
    #[must_use]
    pub fn name(self) -> &'t str {
        match &self.tree.slot(self.at).item {
            Item::Elem { name, .. } => name,
            Item::Text(_) => "",
        }
    }

    /// Attributes in document order.
    #[must_use]
    pub fn attrs(self) -> &'t [(&'t str, Cow<'t, str>)] {
        match self.tree.slot(self.at).item {
            Item::Elem {
                attrs: (start, end),
                ..
            } => &self.tree.attrs[start as usize..end as usize],
            Item::Text(_) => &[],
        }
    }

    /// An attribute's value.
    #[must_use]
    pub fn get(self, key: &str) -> Option<&'t str> {
        self.attrs()
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| &**v)
    }

    fn kids(self) -> impl Iterator<Item = (u32, &'t Item<'t>)> {
        let tree = self.tree;
        let mut at = tree.slot(self.at).first;
        std::iter::from_fn(move || {
            (at != NONE).then(|| {
                let slot = tree.slot(at);
                let here = at;
                at = slot.next;
                (here, &slot.item)
            })
        })
    }

    /// Child elements in order.
    pub fn elems(self) -> impl Iterator<Item = El<'t>> {
        let tree = self.tree;
        self.kids().filter_map(move |(at, item)| match item {
            Item::Elem { .. } => Some(El { tree, at }),
            Item::Text(_) => None,
        })
    }

    /// The first child element with this name.
    #[must_use]
    pub fn find(self, name: &str) -> Option<El<'t>> {
        self.elems().find(|e| e.name() == name)
    }

    /// All character data directly inside, concatenated; borrowed when there is one piece.
    #[must_use]
    pub fn content(self) -> Cow<'t, str> {
        let mut texts = self.kids().filter_map(|(_, item)| match item {
            Item::Text(t) => Some(&**t),
            Item::Elem { .. } => None,
        });
        let Some(first) = texts.next() else {
            return Cow::Borrowed("");
        };
        match texts.next() {
            None => Cow::Borrowed(first),
            Some(second) => {
                let mut all = String::from(first);
                all.push_str(second);
                texts.for_each(|t| all.push_str(t));
                Cow::Owned(all)
            }
        }
    }

    /// The element and everything inside it as an owned [`Node`].
    #[must_use]
    pub fn to_node(self) -> Node {
        Node {
            name: self.name().to_owned(),
            attrs: self
                .attrs()
                .iter()
                .map(|(k, v)| ((*k).to_owned(), v.clone().into_owned()))
                .collect(),
            children: self
                .kids()
                .map(|(at, item)| match item {
                    Item::Elem { .. } => Child::Node(El { tree: self.tree, at }.to_node()),
                    Item::Text(t) => Child::Text(t.clone().into_owned()),
                })
                .collect(),
        }
    }
}

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
    done: bool,
}

/// Why the events do not make a tree.
pub(super) enum Shape {
    /// An end with no start, or content after the document element.
    Unbalanced,
    /// More nodes than an index can name.
    TooMany,
}

impl<'a> Builder<'a> {
    /// A builder sized for a part of `len` bytes.
    pub(super) fn new(len: usize, whitespace: Whitespace) -> Self {
        Self {
            tree: Tree {
                slots: Vec::with_capacity(len / 48),
                attrs: Vec::with_capacity(len / 64),
            },
            stack: Vec::new(),
            whitespace,
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

    /// Whether an attribute named `key` has been given since `mark`.
    pub(super) fn has_attr_since(&self, mark: usize, key: &str) -> bool {
        self.tree.attrs[mark..].iter().any(|(k, _)| *k == key)
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
