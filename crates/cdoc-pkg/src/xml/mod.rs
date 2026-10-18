//! A minimal XML tree. The package parts are small enough to hold whole, and a tree lets the
//! readers look children up by name instead of tracking parser state. Names keep their prefix
//! (`w:p`, `text:p`): both formats fix their prefixes by convention, and namespace declarations
//! are written once, by the part builders.

mod lex;
mod parse;
mod scan;
mod tree;
mod write;

pub use parse::{parse, parse_tree, Whitespace};
pub use tree::{El, Tree};
pub use write::serialize;

/// A node's child.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Child {
    /// An element.
    Node(Node),
    /// Character data.
    Text(String),
}

/// An element with its attributes and children, in document order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    /// The qualified name, e.g. `w:p`.
    pub name: String,
    /// Attributes in document order.
    pub attrs: Vec<(String, String)>,
    /// Children in document order.
    pub children: Vec<Child>,
}

impl Node {
    /// An empty element.
    #[must_use]
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            attrs: Vec::new(),
            children: Vec::new(),
        }
    }

    /// Add an attribute.
    #[must_use]
    pub fn attr(mut self, key: &str, value: &str) -> Self {
        self.attrs.push((key.to_owned(), value.to_owned()));
        self
    }

    /// Add an attribute holding a number.
    #[must_use]
    pub fn num(self, key: &str, value: impl std::fmt::Display) -> Self {
        self.attr(key, &value.to_string())
    }

    /// Add a child element.
    #[must_use]
    pub fn child(mut self, child: Self) -> Self {
        self.children.push(Child::Node(child));
        self
    }

    /// Add several child elements.
    #[must_use]
    pub fn children(mut self, children: impl IntoIterator<Item = Self>) -> Self {
        self.children.extend(children.into_iter().map(Child::Node));
        self
    }

    /// Add character data.
    #[must_use]
    pub fn text(mut self, text: &str) -> Self {
        self.children.push(Child::Text(text.to_owned()));
        self
    }

    /// An attribute's value.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Child elements in order.
    pub fn elems(&self) -> impl Iterator<Item = &Self> {
        self.children.iter().filter_map(|c| match c {
            Child::Node(n) => Some(n),
            Child::Text(_) => None,
        })
    }

    /// The first child element with this name.
    #[must_use]
    pub fn find(&self, name: &str) -> Option<&Self> {
        self.elems().find(|n| n.name == name)
    }

    /// All character data directly inside, concatenated.
    #[must_use]
    pub fn content(&self) -> String {
        self.children
            .iter()
            .filter_map(|c| match c {
                Child::Text(t) => Some(t.as_str()),
                Child::Node(_) => None,
            })
            .collect()
    }
}
