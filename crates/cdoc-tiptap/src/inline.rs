//! The walk over inline nodes: text with its marks, line breaks, and whatever else turns up
//! inside a paragraph, read as its text.

use crate::blocks::Reader;
use crate::node::{Children, Node};
use cdoc_model::builtin::CODE_FONT;
use cdoc_model::{BreakKind, Inline, InlineContent, RunProperties};

/// The block types `StarterKit` has, which never belong inside a paragraph.
const BLOCKS: [&str; 9] = [
    "doc",
    "paragraph",
    "heading",
    "blockquote",
    "codeBlock",
    "bulletList",
    "orderedList",
    "listItem",
    "horizontalRule",
];

fn line_break() -> Inline {
    Inline::plain(InlineContent::Break {
        kind: BreakKind::Line,
    })
}

impl Reader {
    /// The inline content of `node`; in a code block (`code`) every run is monospace and a
    /// newline is a line break.
    pub(crate) fn inlines(&mut self, node: Node<'_>, code: bool) -> Vec<Inline> {
        let mut out = Vec::new();
        self.inline_children(node, code, &mut out);
        out
    }

    fn inline_children(&mut self, node: Node<'_>, code: bool, out: &mut Vec<Inline>) {
        for value in self.children(node) {
            match Node::of(value) {
                Some(child) => self.inline(child, code, out),
                None => self.lose("a malformed node (skipped)"),
            }
        }
    }

    pub(crate) fn inline(&mut self, node: Node<'_>, code: bool, out: &mut Vec<Inline>) {
        match node.kind {
            "text" => self.text(node, code, out),
            "hardBreak" => {
                self.check_attrs(node, &[]);
                if !matches!(node.marks(), Children::Array([])) {
                    self.lose("run formatting on a non-text inline");
                }
                out.push(line_break());
                self.inline_children(node, code, out);
            }
            kind if BLOCKS.contains(&kind) => {
                self.lose("a block inside a paragraph (read as its text)");
                self.inline_children(node, code, out);
            }
            other => {
                let kind = crate::name(other);
                self.lose(&format!("a node of unknown type {kind}; its text kept"));
                self.inline_children(node, code, out);
            }
        }
    }

    fn text(&mut self, node: Node<'_>, code: bool, out: &mut Vec<Inline>) {
        self.check_attrs(node, &[]);
        let Some(text) = node.text() else {
            self.lose("a malformed node (skipped)");
            return;
        };
        let mut properties = self.run_properties(node);
        if code {
            properties.font = Some(CODE_FONT.to_owned());
        }
        if !code {
            if !text.is_empty() {
                out.push(Inline::text(text, properties));
            }
            return;
        }
        for (i, line) in text.split('\n').enumerate() {
            if i > 0 {
                out.push(line_break());
            }
            if !line.is_empty() {
                out.push(Inline::text(line, properties.clone()));
            }
        }
    }

    /// The run formatting `node`'s marks give, reporting each mark `StarterKit` does not have.
    fn run_properties(&mut self, node: Node<'_>) -> RunProperties {
        let mut properties = RunProperties::none();
        let marks = match node.marks() {
            Children::Array(marks) => marks,
            Children::Malformed => {
                self.lose(&format!(
                    "a mark of unknown type {} (dropped)",
                    crate::name("")
                ));
                return properties;
            }
        };
        for value in marks {
            let Some(mark) = Node::of(value) else {
                self.lose(&format!(
                    "a mark of unknown type {} (dropped)",
                    crate::name("")
                ));
                continue;
            };
            if matches!(mark.kind, "bold" | "italic" | "strike" | "code") {
                self.check_attrs(mark, &[]);
            }
            match mark.kind {
                "bold" => properties.bold = Some(true),
                "italic" => properties.italic = Some(true),
                "strike" => properties.strike = Some(true),
                "code" => {
                    self.lose("inline code (read as a monospace run without its shading)");
                    properties.font = Some(CODE_FONT.to_owned());
                }
                other => {
                    let kind = crate::name(other);
                    self.lose(&format!("a mark of unknown type {kind} (dropped)"));
                }
            }
        }
        properties
    }
}
