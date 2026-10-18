//! The shared XML trees, read the OOXML way (whitespace between elements is layout), plus the
//! `w:val` conventions every `WordprocessingML` part uses. Parts are read into the borrowed
//! [`Tree`] and written from the owned [`Node`].

pub use cdoc_pkg::xml::{serialize, Child, El, Node, Tree};

use crate::error::DocxError;
use cdoc_pkg::xml::Whitespace;
use cdoc_pkg::ReadLimits;

/// Parse a part; text lives only in `w:t`, so whitespace between elements is dropped.
pub fn parse<'a>(part: &str, bytes: &'a [u8], limits: &ReadLimits) -> Result<Tree<'a>, DocxError> {
    cdoc_pkg::xml::parse_tree(part, bytes, Whitespace::DropBetweenElements, limits)
}

/// Building `<name w:val="value"/>`.
pub trait WordNode {
    /// `<name w:val="value"/>`.
    #[must_use]
    fn val(name: &str, value: &str) -> Self;
}

impl WordNode for Node {
    fn val(name: &str, value: &str) -> Self {
        Self::new(name).attr("w:val", value)
    }
}

/// Reading the `w:val` conventions.
pub trait WordEl<'t> {
    /// The `w:val` attribute.
    fn value(self) -> Option<&'t str>;
    /// Whether an on/off element (`<w:b/>`, `<w:b w:val="0"/>`) is on.
    fn is_on(&self) -> bool;
}

impl<'t> WordEl<'t> for El<'t> {
    fn value(self) -> Option<&'t str> {
        self.get("w:val")
    }

    fn is_on(&self) -> bool {
        !matches!(self.value(), Some("0" | "false" | "off"))
    }
}

/// An on/off element: `<name/>` for true, `<name w:val="0"/>` for false.
#[must_use]
pub fn on_off(name: &str, on: bool) -> Node {
    if on {
        Node::new(name)
    } else {
        Node::val(name, "0")
    }
}
