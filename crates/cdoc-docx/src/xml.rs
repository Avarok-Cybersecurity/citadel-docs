//! The shared XML tree, read the OOXML way (whitespace between elements is layout), plus the
//! `w:val` conventions every `WordprocessingML` part uses.

pub use cdoc_pkg::xml::{serialize, Child, Node};

use crate::error::DocxError;
use cdoc_pkg::xml::Whitespace;

/// Parse a part; text lives only in `w:t`, so whitespace between elements is dropped.
pub fn parse(part: &str, bytes: &[u8]) -> Result<Node, DocxError> {
    cdoc_pkg::xml::parse(part, bytes, Whitespace::DropBetweenElements)
}

/// The `w:val` conventions.
pub trait WordNode {
    /// `<name w:val="value"/>`.
    #[must_use]
    fn val(name: &str, value: &str) -> Self;
    /// The `w:val` attribute.
    fn value(&self) -> Option<&str>;
    /// Whether an on/off element (`<w:b/>`, `<w:b w:val="0"/>`) is on.
    fn is_on(&self) -> bool;
}

impl WordNode for Node {
    fn val(name: &str, value: &str) -> Self {
        Self::new(name).attr("w:val", value)
    }

    fn value(&self) -> Option<&str> {
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
