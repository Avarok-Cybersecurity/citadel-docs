//! A parsed part held as one arena over the part's own bytes: names, attribute values and text
//! borrow the input wherever no entity had to be resolved, and every element, attribute and
//! text piece lives in one of two vectors. Reading a part costs no allocation per element, and
//! dropping it costs none per element either.

use super::{Child, Node};
use std::borrow::Cow;

pub(super) const NONE: u32 = u32::MAX;

#[derive(Debug)]
pub(super) enum Item<'a> {
    Elem { name: &'a str, attrs: (u32, u32) },
    Text(Cow<'a, str>),
}

#[derive(Debug)]
pub(super) struct Slot<'a> {
    pub(super) item: Item<'a>,
    pub(super) first: u32,
    pub(super) next: u32,
}

/// A parsed part. [`Tree::root`] is the document element.
#[derive(Debug)]
pub struct Tree<'a> {
    pub(super) slots: Vec<Slot<'a>>,
    pub(super) attrs: Vec<(&'a str, Cow<'a, str>)>,
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
                    Item::Elem { .. } => Child::Node(
                        El {
                            tree: self.tree,
                            at,
                        }
                        .to_node(),
                    ),
                    Item::Text(t) => Child::Text(t.clone().into_owned()),
                })
                .collect(),
        }
    }
}
