//! The walk over block nodes: paragraphs, headings, quotes, code blocks and rules, and where
//! each lands given the quote or list it is in.

use crate::node::{whole, Children, Node};
use crate::PART;
use cdoc_model::builtin::ids::{self, heading};
use cdoc_model::builtin::{rule_paragraph, UniformLists};
use cdoc_model::{
    Block, Document, Inline, NumId, NumberingRef, Paragraph, ParagraphProperties, Report,
};

/// The list a block is in.
#[derive(Clone, Copy)]
pub(crate) struct ListAt {
    pub(crate) num_id: NumId,
    /// The model's level, at most `MAX_LEVEL`.
    pub(crate) level: u8,
    /// How deeply the input nests it, from 0.
    pub(crate) depth: u32,
}

/// Where a block is.
#[derive(Clone, Copy)]
pub(crate) struct Ctx {
    pub(crate) quote: bool,
    pub(crate) list: Option<ListAt>,
}

impl Ctx {
    const BODY: Self = Self {
        quote: false,
        list: None,
    };
}

pub(crate) struct Reader {
    pub(crate) report: Report,
    pub(crate) out: Vec<Block>,
    pub(crate) lists: UniformLists,
    /// Text found where a block belongs, waiting to become a paragraph.
    pub(crate) loose: Vec<Inline>,
    /// The number of the list item whose first paragraph has not been read yet.
    pub(crate) number: Option<NumberingRef>,
}

/// The document `root` holds, and what it lost.
pub(crate) fn read(root: Node<'_>) -> (Document, Report) {
    let mut r = Reader {
        report: Report::empty(),
        out: Vec::new(),
        lists: UniformLists::new(),
        loose: Vec::new(),
        number: None,
    };
    if root.kind == "doc" {
        r.check_attrs(root, &[]);
        r.blocks(root, Ctx::BODY);
    } else {
        r.lose("a document whose root is not a doc node (read as its content)");
        r.block(root, Ctx::BODY);
        r.flush_loose(Ctx::BODY);
    }
    let mut doc = Document::blank();
    if r.out.is_empty() {
        r.lose("final section had no content; an empty paragraph added");
    } else {
        doc.sections[0].blocks = r.out;
    }
    doc.numbering = r.lists.finish();
    doc.normalize();
    (doc, r.report)
}

pub(crate) fn styled(style: cdoc_model::StyleId, inlines: Vec<Inline>) -> Paragraph {
    Paragraph {
        properties: ParagraphProperties {
            style: Some(style),
            ..ParagraphProperties::none()
        },
        inlines,
    }
}

impl Reader {
    pub(crate) fn lose(&mut self, what: &str) {
        self.report.note(PART, what);
    }

    /// Report every attribute of `node` outside `known`.
    pub(crate) fn check_attrs(&mut self, node: Node<'_>, known: &[&str]) {
        for attr in node.attr_names() {
            if !known.contains(&attr) {
                let (kind, attr) = (crate::name(node.kind), crate::name(attr));
                self.lose(&format!(
                    "an attribute the chat editor did not have: {kind}.{attr}"
                ));
            }
        }
    }

    /// The children of `node`, reporting a `content` that is not an array.
    pub(crate) fn children<'a>(&mut self, node: Node<'a>) -> &'a [serde_json::Value] {
        match node.content() {
            Children::Array(items) => items,
            Children::Malformed => {
                self.lose("a malformed node (skipped)");
                &[]
            }
        }
    }

    /// Every child of `node` as a block in `ctx`.
    pub(crate) fn blocks(&mut self, node: Node<'_>, ctx: Ctx) {
        for value in self.children(node) {
            match Node::of(value) {
                Some(child) => self.block(child, ctx),
                None => self.lose("a malformed node (skipped)"),
            }
        }
        self.flush_loose(ctx);
    }

    pub(crate) fn block(&mut self, node: Node<'_>, ctx: Ctx) {
        if matches!(node.kind, "text" | "hardBreak") {
            let mut loose = std::mem::take(&mut self.loose);
            self.inline(node, false, &mut loose);
            self.loose = loose;
            return;
        }
        self.flush_loose(ctx);
        match node.kind {
            "paragraph" => {
                self.check_attrs(node, &[]);
                let inlines = self.inlines(node, false);
                self.place(styled(ids::normal(), inlines), ctx, true);
            }
            "heading" => {
                self.check_attrs(node, &["level"]);
                let level = self.heading_level(node);
                let inlines = self.inlines(node, false);
                let style = heading(level).unwrap_or_else(ids::normal);
                self.place(styled(style, inlines), ctx, false);
            }
            "codeBlock" => {
                self.check_attrs(node, &["language"]);
                if node
                    .attr("language")
                    .is_some_and(|v| v.as_str() != Some(""))
                {
                    self.lose("a code block's language (dropped)");
                }
                self.lose("code block (read as monospace paragraph)");
                let inlines = self.inlines(node, true);
                self.place(styled(ids::normal(), inlines), ctx, false);
            }
            "horizontalRule" => {
                self.check_attrs(node, &[]);
                self.place(rule_paragraph(), ctx, false);
                self.blocks(node, ctx);
            }
            "blockquote" => self.quote(node, ctx),
            "bulletList" | "orderedList" => self.list(node, ctx),
            "listItem" => {
                self.lose("a list item outside a list (read as its paragraphs)");
                self.blocks(node, ctx);
            }
            other => {
                let kind = crate::name(other);
                self.lose(&format!("a node of unknown type {kind}; its text kept"));
                self.blocks(node, ctx);
            }
        }
    }

    fn heading_level(&mut self, node: Node<'_>) -> u8 {
        let Some(value) = node.attr("level") else {
            return 1;
        };
        match whole(value) {
            Some(n @ 1..=6) => u8::try_from(n).unwrap_or(1),
            Some(n) => {
                self.lose("a heading level outside 1 to 6 read as the nearest");
                if n < 1 {
                    1
                } else {
                    6
                }
            }
            None => {
                self.lose("a heading level outside 1 to 6 read as the nearest");
                1
            }
        }
    }

    fn quote(&mut self, node: Node<'_>, ctx: Ctx) {
        self.check_attrs(node, &[]);
        if ctx.list.is_some() {
            self.lose("a block quote inside a list item (the quote not kept)");
        } else if ctx.quote {
            self.lose("nested block quote (flattened)");
        } else {
            self.lose("block quote (read as the Quote style)");
        }
        self.blocks(node, Ctx { quote: true, ..ctx });
    }
}
