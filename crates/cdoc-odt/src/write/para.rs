//! Paragraphs: an automatic paragraph style for the direct formatting (based on the named
//! style), `text:h` for an outline level, the inline stream with its ranges, and the split ODF
//! needs for a page or column break inside a paragraph: the rest goes into a continuation
//! paragraph that breaks before, and the sidecar tells the reader to join the halves again.

use super::line::{Line, Story};
use super::region::is_deletion;
use super::registry::Auto;
use super::Writer;
use crate::ns::Part;
use crate::props::{read_paragraph, write_paragraph};
use crate::residual::Residual;
use cdoc_model::{
    BreakKind, CommentId, Inline, InlineContent, Paragraph, ParagraphProperties, RevisionKind,
};
use cdoc_pkg::xml::Node;
use cdoc_pkg::{Log, PackageError};

/// What the first block of a section carries: the master page it starts and the page number
/// it restarts at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionStart {
    /// The master page's name.
    pub master: String,
    /// The restart value.
    pub page_number: Option<u32>,
}

impl Writer<'_> {
    fn paragraph_style(
        &mut self,
        props: &ParagraphProperties,
        start: Option<&SectionStart>,
    ) -> Option<String> {
        let carried = ParagraphProperties {
            style: None,
            numbering: None,
            outline_level: None,
            ..props.clone()
        };
        let mut node = write_paragraph(&carried);
        if let Some(n) = start.and_then(|s| s.page_number) {
            node = node.num("style:page-number", n);
        }
        let mut scratch = Log::new("(readback)");
        let back = read_paragraph(&node, &["style:page-number"], &mut scratch);
        let parent = props.style.as_ref().map(|s| self.names.of(s));
        let mut auto = Auto::new("paragraph", vec![node]);
        auto.residual = Residual::between(&carried, &back, &[]);
        if let Some(s) = start {
            auto.attrs
                .push(("style:master-page-name".to_owned(), s.master.clone()));
        }
        if auto.is_empty() {
            return parent;
        }
        auto.parent = parent;
        Some(self.autos().name(auto))
    }

    fn element(props: &ParagraphProperties, style: Option<&str>) -> Node {
        let mut node = match props.outline_level {
            Some(level) => Node::new("text:h").num("text:outline-level", u32::from(level) + 1),
            None => Node::new("text:p"),
        };
        if let Some(style) = style {
            node = node.attr("text:style-name", style);
        }
        node
    }

    fn continuation_style(
        &mut self,
        props: &ParagraphProperties,
        inline: &Inline,
        kind: BreakKind,
    ) -> String {
        let before = if kind == BreakKind::Column {
            "column"
        } else {
            "page"
        };
        let mut auto = Auto::new(
            "paragraph",
            vec![Node::new(crate::props::PARAGRAPH_PROPERTIES).attr("fo:break-before", before)],
        );
        auto.parent = props.style.as_ref().map(|s| self.names.of(s));
        let name = self.autos().unique(auto);
        self.sidecar
            .continuations
            .insert(name.clone(), (kind, inline.marks.clone()));
        if is_deletion(inline.marks.revision.as_ref()) {
            self.log
                .lose("a deleted page or column break written as a break");
        }
        name
    }

    fn inline(
        &mut self,
        inline: &Inline,
        story: &mut Story,
        line: &mut Line,
    ) -> Result<(), PackageError> {
        let mut wanted: Vec<CommentId> = Vec::new();
        for id in &inline.marks.comments {
            if !wanted.contains(id) {
                wanted.push(id.clone());
            }
        }
        let insertion = inline
            .marks
            .revision
            .as_ref()
            .filter(|r| r.kind == RevisionKind::Insertion);
        Self::close_comments(&wanted, story, line);
        Self::close_insertion(insertion, story, line);
        line.link_to(inline.marks.link.as_ref());
        self.open_comments(&wanted, story, line);
        self.open_insertion(insertion, story, line);
        match inline
            .marks
            .revision
            .as_ref()
            .filter(|r| r.kind == RevisionKind::Deletion)
        {
            Some(revision) => self.deleted(inline, revision, line),
            None => self.content(&inline.content, line),
        }
    }

    /// Close every range still open in the story at the end of `line`.
    pub(super) fn close_story(story: &mut Story, line: &mut Line) {
        Self::close_comments(&[], story, line);
        Self::close_insertion(None, story, line);
    }

    /// A paragraph, plus a continuation paragraph after each page or column break in it.
    /// `closes` ends the story's open ranges at the paragraph's end (the next block is not a
    /// paragraph of the same container).
    pub(super) fn paragraph(
        &mut self,
        p: &Paragraph,
        story: &mut Story,
        start: Option<&SectionStart>,
        closes: bool,
    ) -> Result<Vec<Node>, PackageError> {
        let style = self.paragraph_style(&p.properties, start);
        let mut out = Vec::new();
        let mut node = Self::element(&p.properties, style.as_deref());
        let mut line = Line::new();
        if self.part == Part::Content && !self.pending.is_empty() {
            for pending in std::mem::take(&mut self.pending) {
                line.element(pending);
            }
        }
        for inline in &p.inlines {
            if let InlineContent::Break {
                kind: kind @ (BreakKind::Page | BreakKind::Column),
            } = inline.content
            {
                node.children = std::mem::replace(&mut line, Line::new()).finish();
                out.push(node);
                let name = self.continuation_style(&p.properties, inline, kind);
                node = Node::new("text:p").attr("text:style-name", &name);
                continue;
            }
            self.inline(inline, story, &mut line)?;
        }
        if closes {
            Self::close_story(story, &mut line);
        }
        node.children = line.finish();
        out.push(node);
        Ok(out)
    }
}
