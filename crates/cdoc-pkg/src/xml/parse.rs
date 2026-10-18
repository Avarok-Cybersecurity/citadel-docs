//! XML text to a [`Tree`] (borrowed, for readers) or a [`Node`] tree (owned).

use super::builder::Builder;
use super::lex::lex;
use super::tree::Tree;
use super::Node;
use crate::error::PackageError;
use crate::limits::ReadLimits;

/// What happens to character data that is only whitespace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Whitespace {
    /// Dropped from elements that contain elements; kept inside an element that has no element
    /// children (OOXML: text lives only in `w:t`, so whitespace between elements is layout).
    DropBetweenElements,
    /// Kept everywhere (ODF: text sits between elements in a paragraph, so a space between two
    /// spans is content).
    Keep,
}

/// Parse a part into an owned tree.
pub fn parse(
    part: &str,
    bytes: &[u8],
    whitespace: Whitespace,
    limits: &ReadLimits,
) -> Result<Node, PackageError> {
    Ok(parse_tree(part, bytes, whitespace, limits)?
        .root()
        .to_node())
}

/// Parse a part into a tree that borrows `bytes`, refusing nesting deeper than
/// [`ReadLimits::max_depth`].
pub fn parse_tree<'a>(
    part: &str,
    bytes: &'a [u8],
    whitespace: Whitespace,
    limits: &ReadLimits,
) -> Result<Tree<'a>, PackageError> {
    let bad = |detail: String| PackageError::Xml {
        part: part.to_owned(),
        detail,
    };
    let text = std::str::from_utf8(bytes).map_err(|e| bad(e.to_string()))?;
    let text = text.strip_prefix('\u{FEFF}').unwrap_or(text);
    let mut b = Builder::new(text.len(), whitespace, limits.max_depth);
    lex(text, &mut b).map_err(bad)?;
    b.finish().ok_or_else(|| bad("no root element".into()))
}
