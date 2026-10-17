//! White space in paragraph text. ODF collapses runs of white space to one space and drops it
//! at the start of a paragraph, so the writer spells every space that is not a single space
//! after a visible character as `<text:s/>` (with `text:c` for several), and the reader
//! applies the collapsing rule (ODF 1.2 part 1, 6.1.2) with the same state across spans.

use cdoc_pkg::xml::{Child, Node};

/// The collapsing state shared by everything in one paragraph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Space {
    /// Whether the last character produced was a space (or nothing has been produced yet), so
    /// a literal white-space character now is dropped.
    after_space: bool,
}

impl Space {
    /// The state at the start of a paragraph.
    #[must_use]
    pub const fn start() -> Self {
        Self { after_space: true }
    }

    /// After an element that produced white space (`text:s`, `text:tab`, `text:line-break`).
    pub fn white(&mut self) {
        self.after_space = true;
    }

    /// After an element that produced something visible (an image, a field, a note mark).
    pub fn visible(&mut self) {
        self.after_space = false;
    }

    /// Collapse literal character data.
    pub fn collapse(&mut self, text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        for c in text.chars() {
            if matches!(c, ' ' | '\t' | '\n' | '\r') {
                if !self.after_space {
                    out.push(' ');
                    self.after_space = true;
                }
            } else {
                out.push(c);
                self.after_space = false;
            }
        }
        out
    }

    /// Encode text for the writer, appending to `out`. Tabs and line feeds inside text cannot
    /// survive collapsing; they are written as `text:tab` / `text:line-break` and counted in
    /// the second value; carriage returns are dropped and counted.
    pub fn encode(&mut self, text: &str, out: &mut Vec<Child>) -> u32 {
        let mut literal = String::new();
        let mut spaces = 0u32;
        let mut altered = 0u32;
        let flush_spaces = |spaces: &mut u32, literal: &mut String, out: &mut Vec<Child>| {
            if *spaces == 0 {
                return;
            }
            push_literal(literal, out);
            let node = Node::new("text:s");
            let node = if *spaces > 1 {
                node.num("text:c", *spaces)
            } else {
                node
            };
            out.push(Child::Node(node));
            *spaces = 0;
        };
        for c in text.chars() {
            match c {
                ' ' if !self.after_space && spaces == 0 => {
                    literal.push(' ');
                    self.after_space = true;
                }
                ' ' => spaces += 1,
                '\t' | '\n' | '\r' => {
                    flush_spaces(&mut spaces, &mut literal, out);
                    altered += 1;
                    if c != '\r' {
                        push_literal(&mut literal, out);
                        let name = if c == '\t' {
                            "text:tab"
                        } else {
                            "text:line-break"
                        };
                        out.push(Child::Node(Node::new(name)));
                        self.after_space = true;
                    }
                }
                c => {
                    flush_spaces(&mut spaces, &mut literal, out);
                    literal.push(c);
                    self.after_space = false;
                }
            }
        }
        flush_spaces(&mut spaces, &mut literal, out);
        push_literal(&mut literal, out);
        altered
    }
}

fn push_literal(literal: &mut String, out: &mut Vec<Child>) {
    if !literal.is_empty() {
        out.push(Child::Text(std::mem::take(literal)));
    }
}

/// The text of a `text:s` element: `text:c` spaces, one when absent.
#[must_use]
pub fn spaces(node: &Node) -> String {
    let count = node
        .get("text:c")
        .and_then(|c| c.parse::<usize>().ok())
        .unwrap_or(1)
        .min(10_000);
    " ".repeat(count)
}

#[cfg(test)]
#[path = "text_tests.rs"]
mod tests;
