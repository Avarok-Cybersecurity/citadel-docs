//! The XML reference: the quick-xml parser `cdoc-pkg` shipped before it had a lexer of its own,
//! kept verbatim apart from returning its error as text. The lexer must build the same tree from
//! whatever this accepts, except where it is deliberately stricter (see [`stricter`]).

use cdoc_pkg::xml::{Child, Node, Whitespace};
use quick_xml::escape::resolve_predefined_entity;
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

/// Parse as the old parser did.
pub fn parse(bytes: &[u8], whitespace: Whitespace) -> Result<Node, String> {
    let bad = |detail: String| detail;
    let mut reader = Reader::from_reader(bytes);
    reader.config_mut().expand_empty_elements = true;
    let mut stack: Vec<Node> = Vec::new();
    let mut root = None;
    loop {
        match reader.read_event().map_err(|e| bad(e.to_string()))? {
            Event::Start(e) => stack.push(open(&e)?),
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
            Event::Text(t) => push_text(&mut stack, &t.decode().map_err(|e| e.to_string())?),
            Event::CData(t) => push_text(&mut stack, &t.decode().map_err(|e| e.to_string())?),
            Event::GeneralRef(r) => {
                let resolved = if let Some(c) = r.resolve_char_ref().map_err(|e| e.to_string())? {
                    c.to_string()
                } else {
                    let name = r.decode().map_err(|e| e.to_string())?;
                    resolve_predefined_entity(&name)
                        .ok_or_else(|| format!("unknown entity &{name};"))?
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

fn open(e: &BytesStart<'_>) -> Result<Node, String> {
    let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
    let mut node = Node::new(&name);
    for attr in e.attributes() {
        let attr = attr.map_err(|err| err.to_string())?;
        let key = String::from_utf8_lossy(attr.key.as_ref()).into_owned();
        let value = attr
            .unescape_value()
            .map_err(|err| err.to_string())?
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

fn settle(mut node: Node) -> Node {
    if node.children.iter().any(|c| matches!(c, Child::Node(_))) {
        node.children
            .retain(|c| !matches!(c, Child::Text(t) if t.trim().is_empty()));
    }
    node
}
