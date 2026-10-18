//! XML text to a [`Node`] tree.

use super::{Child, Node};
use crate::error::PackageError;
use quick_xml::escape::resolve_predefined_entity;
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

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

/// Parse a part.
pub fn parse(part: &str, bytes: &[u8], whitespace: Whitespace) -> Result<Node, PackageError> {
    let bad = |detail: String| PackageError::Xml {
        part: part.to_owned(),
        detail,
    };
    let mut reader = Reader::from_reader(bytes);
    reader.config_mut().expand_empty_elements = true;
    let mut stack: Vec<Node> = Vec::new();
    let mut root = None;
    loop {
        match reader.read_event().map_err(|e| bad(e.to_string()))? {
            Event::Start(e) => stack.push(open(&e, &bad)?),
            Event::End(_) => {
                let done = stack
                    .pop()
                    .ok_or_else(|| bad("unbalanced end tag".into()))?;
                let done = match whitespace {
                    Whitespace::DropBetweenElements => settle(done),
                    Whitespace::Keep => done,
                };
                match stack.last_mut() {
                    Some(parent) => parent.children.push(Child::Node(done)),
                    None => root = Some(done),
                }
            }
            Event::Text(t) => {
                let text = t.decode().map_err(|e| bad(e.to_string()))?;
                push_text(&mut stack, &text);
            }
            Event::CData(t) => {
                let text = t.decode().map_err(|e| bad(e.to_string()))?;
                push_text(&mut stack, &text);
            }
            Event::GeneralRef(r) => {
                let resolved =
                    if let Some(c) = r.resolve_char_ref().map_err(|e| bad(e.to_string()))? {
                        c.to_string()
                    } else {
                        let name = r.decode().map_err(|e| bad(e.to_string()))?;
                        resolve_predefined_entity(&name)
                            .ok_or_else(|| bad(format!("unknown entity &{name};")))?
                            .to_owned()
                    };
                push_text(&mut stack, &resolved);
            }
            Event::Eof => break,
            _ => {}
        }
    }
    root.ok_or_else(|| bad("no root element".into()))
}

fn open(e: &BytesStart<'_>, bad: &dyn Fn(String) -> PackageError) -> Result<Node, PackageError> {
    let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
    let mut node = Node::new(&name);
    for attr in e.attributes() {
        let attr = attr.map_err(|err| bad(err.to_string()))?;
        let key = String::from_utf8_lossy(attr.key.as_ref()).into_owned();
        let value = attr
            .unescape_value()
            .map_err(|err| bad(err.to_string()))?
            .into_owned();
        node.attrs.push((key, value));
    }
    Ok(node)
}

fn push_text(stack: &mut [Node], text: &str) {
    let Some(top) = stack.last_mut() else { return };
    if let Some(Child::Text(last)) = top.children.last_mut() {
        last.push_str(text);
    } else {
        top.children.push(Child::Text(text.to_owned()));
    }
}

/// Drop layout whitespace from elements that contain elements.
fn settle(mut node: Node) -> Node {
    if node.children.iter().any(|c| matches!(c, Child::Node(_))) {
        node.children
            .retain(|c| !matches!(c, Child::Text(t) if t.trim().is_empty()));
    }
    node
}
