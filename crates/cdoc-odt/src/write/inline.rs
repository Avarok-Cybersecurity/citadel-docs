//! Inline content: text runs as spans with automatic text styles, images as frames, breaks,
//! tabs, fields, form fields as input fields carrying the exact value, footnote marks.

use super::line::Line;
use super::registry::Auto;
use super::Writer;
use crate::props::{read_run, write_run};
use crate::residual::Residual;
use crate::units;
use cdoc_model::{
    Field, FieldKind, FormField, FormFieldKind, InlineContent, InlineImage, RunProperties,
    SignatureField, TextRun,
};
use cdoc_pkg::tag::Tagged;
use cdoc_pkg::xml::Node;
use cdoc_pkg::{Log, PackageError};
use std::collections::BTreeMap;

fn display(field: &FormField) -> String {
    match &field.field {
        FormFieldKind::Text { value, .. } => value.clone(),
        FormFieldKind::Checkbox { checked } => {
            if *checked { "\u{2612}" } else { "\u{2610}" }.to_owned()
        }
        FormFieldKind::Dropdown { options, selected } => selected
            .and_then(|i| options.get(usize::try_from(i).ok()?))
            .cloned()
            .unwrap_or_default(),
        FormFieldKind::Date { value } => value.clone().unwrap_or_default(),
    }
}

fn input(tag: &Tagged, shown: &str) -> Node {
    let node = Node::new("text:text-input").attr("text:description", &tag.encode());
    if shown.is_empty() {
        node
    } else {
        node.text(shown)
    }
}

impl Writer<'_> {
    /// The style a run's span names: its character style alone, or an automatic style based on
    /// it carrying the direct formatting. `None` when the run needs no span.
    pub(super) fn span_style(&mut self, props: &RunProperties) -> Option<String> {
        let direct = RunProperties {
            style: None,
            ..props.clone()
        };
        let parent = props.style.as_ref().map(|s| self.names.of(s));
        let node = write_run(&direct);
        let mut scratch = Log::new("(readback)");
        let back = read_run(&node, &BTreeMap::new(), &mut scratch);
        let mut auto = Auto::new("text", vec![node]);
        auto.residual = Residual::between(&direct, &back, &[]);
        if auto.is_empty() {
            return parent;
        }
        auto.parent = parent;
        Some(self.autos().name(auto))
    }

    fn text(&mut self, run: &TextRun, line: &mut Line) {
        let mut children = Vec::new();
        let altered = line.space.encode(&run.text, &mut children);
        self.log.report.note_n(
            &self.log.part.clone(),
            "a tab, line feed or carriage return inside text written as a tab or line break",
            altered,
        );
        match self.span_style(&run.properties) {
            Some(style) => {
                let mut span = Node::new("text:span").attr("text:style-name", &style);
                span.children = children;
                line.element(span);
            }
            None => children.into_iter().for_each(|c| line.push(c)),
        }
    }

    fn frame(&mut self, img: &InlineImage, line: &mut Line) -> Result<(), PackageError> {
        let path = self.picture(&img.content_id)?;
        self.frames += 1;
        let mut frame = Node::new("draw:frame")
            .attr("draw:name", &format!("Image{}", self.frames))
            .attr("text:anchor-type", "as-char")
            .attr("svg:width", &units::write_length(img.width.0))
            .attr("svg:height", &units::write_length(img.height.0))
            .attr("draw:z-index", "0")
            .child(
                Node::new("draw:image")
                    .attr("xlink:href", &path)
                    .attr("xlink:type", "simple")
                    .attr("xlink:show", "embed")
                    .attr("xlink:actuate", "onLoad"),
            );
        if let Some(alt) = &img.alt {
            frame = frame.child(Node::new("svg:desc").text(alt));
        }
        line.element(frame);
        line.space.visible();
        Ok(())
    }

    fn field(&mut self, field: &Field, line: &mut Line) {
        let node = match &field.field {
            FieldKind::PageNumber => {
                Node::new("text:page-number").attr("text:select-page", "current")
            }
            FieldKind::PageCount => Node::new("text:page-count"),
            FieldKind::Date { format } => {
                let style = self.date_style(format);
                Node::new("text:date").attr("style:data-style-name", &style)
            }
        };
        let node = if field.cached.is_empty() {
            node
        } else {
            node.text(&field.cached)
        };
        line.element(node);
        line.space.visible();
    }

    fn form(field: &FormField, line: &mut Line) {
        line.element(input(&Tagged::Form(field.clone()), &display(field)));
        line.space.visible();
    }

    fn signature(field: &SignatureField, line: &mut Line) {
        let shown = field
            .label
            .clone()
            .unwrap_or_else(|| field.signer_role.clone());
        line.element(input(&Tagged::Signature(field.clone()), &shown));
        line.space.visible();
    }

    /// Write one inline's content (its marks are the caller's business). Page and column
    /// breaks are the paragraph's business and never reach here.
    pub(super) fn content(
        &mut self,
        content: &InlineContent,
        line: &mut Line,
    ) -> Result<(), PackageError> {
        match content {
            InlineContent::Text(run) => self.text(run, line),
            InlineContent::Image(img) => self.frame(img, line)?,
            InlineContent::Break { .. } => {
                line.element(Node::new("text:line-break"));
                line.space.white();
            }
            InlineContent::Tab => {
                line.element(Node::new("text:tab"));
                line.space.white();
            }
            InlineContent::FootnoteRef { id } => self.footnote(*id, line)?,
            InlineContent::Field(f) => self.field(f, line),
            InlineContent::FormField(f) => Self::form(f, line),
            InlineContent::SignatureField(f) => Self::signature(f, line),
        }
        Ok(())
    }
}
