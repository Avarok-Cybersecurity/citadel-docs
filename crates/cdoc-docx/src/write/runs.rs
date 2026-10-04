//! Inline content as runs: text, breaks, tabs, footnote marks, fields and images.

use super::Writer;
use crate::error::DocxError;
use crate::props::write_rpr;
use crate::xml::Node;
use cdoc_model::{
    BreakKind, ContentId, Field, FieldKind, InlineContent, InlineImage, RunProperties,
};

const PICTURE_URI: &str = "http://schemas.openxmlformats.org/drawingml/2006/picture";
/// EMU per hundredth of a point.
const EMU_PER_PT100: i64 = 127;

fn run(rpr: Option<Node>, content: Node) -> Node {
    let node = Node::new("w:r");
    match rpr {
        Some(rpr) => node.child(rpr).child(content),
        None => node.child(content),
    }
}

fn field_char(kind: &str) -> Node {
    run(None, Node::new("w:fldChar").attr("w:fldCharType", kind))
}

fn instruction(field: &FieldKind) -> String {
    match field {
        FieldKind::PageNumber => " PAGE ".to_owned(),
        FieldKind::PageCount => " NUMPAGES ".to_owned(),
        FieldKind::Date { format } => {
            format!(" DATE \\@ \"{}\" ", format.replace('"', "\\\""))
        }
    }
}

/// The file extension for an image MIME type.
fn extension(mime: &str) -> String {
    let subtype = mime.strip_prefix("image/").unwrap_or("");
    let stem: String = subtype
        .chars()
        .take_while(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect();
    if stem.is_empty() {
        "bin".to_owned()
    } else {
        stem
    }
}

impl Writer<'_> {
    pub(super) fn content(
        &mut self,
        content: &InlineContent,
        deleted: bool,
    ) -> Result<Vec<Node>, DocxError> {
        let text_name = if deleted { "w:delText" } else { "w:t" };
        Ok(match content {
            InlineContent::Text(t) => {
                let rpr = (!t.properties.is_none()).then(|| write_rpr(&t.properties));
                let text = Node::new(text_name)
                    .attr("xml:space", "preserve")
                    .text(&t.text);
                vec![run(rpr, text)]
            }
            InlineContent::Break { kind } => {
                let br = Node::new("w:br");
                let br = match kind {
                    BreakKind::Line => br,
                    BreakKind::Page => br.attr("w:type", "page"),
                    BreakKind::Column => br.attr("w:type", "column"),
                };
                vec![run(None, br)]
            }
            InlineContent::Tab => vec![run(None, Node::new("w:tab"))],
            InlineContent::FootnoteRef { id } => {
                let style = if self.footnote_style {
                    RunProperties {
                        style: Some(cdoc_model::StyleId::new("FootnoteReference")),
                        ..RunProperties::none()
                    }
                } else {
                    RunProperties {
                        vertical_align: Some(cdoc_model::VerticalAlign::Superscript),
                        ..RunProperties::none()
                    }
                };
                let mark = Node::new("w:footnoteReference").num("w:id", id.0);
                vec![run(Some(write_rpr(&style)), mark)]
            }
            InlineContent::Field(f) => Self::field_runs(f, deleted),
            InlineContent::Image(img) => vec![self.image_run(img)?],
            InlineContent::FormField(f) => vec![Self::form_field_sdt(f, deleted)],
            InlineContent::SignatureField(f) => vec![Self::signature_sdt(f, deleted)],
        })
    }

    fn field_runs(field: &Field, deleted: bool) -> Vec<Node> {
        let (instr_name, text_name) = if deleted {
            ("w:delInstrText", "w:delText")
        } else {
            ("w:instrText", "w:t")
        };
        let instr = Node::new(instr_name)
            .attr("xml:space", "preserve")
            .text(&instruction(&field.field));
        let result = Node::new(text_name)
            .attr("xml:space", "preserve")
            .text(&field.cached);
        vec![
            field_char("begin"),
            run(None, instr),
            field_char("separate"),
            run(None, result),
            field_char("end"),
        ]
    }

    fn media_rel(&mut self, id: &ContentId) -> Result<String, DocxError> {
        let existing = self
            .media_parts
            .iter()
            .find(|p| &p.id == id)
            .map(|p| p.name.clone());
        let name = if let Some(name) = existing {
            name
        } else {
            let image = self.media.image(id)?;
            let name = format!(
                "word/media/image{}.{}",
                self.media_parts.len() + 1,
                extension(image.mime)
            );
            self.out.put(&name, image.bytes.to_vec());
            self.media_parts.push(super::MediaPart {
                id: id.clone(),
                name: name.clone(),
                mime: image.mime.to_owned(),
            });
            name
        };
        let relative = name.strip_prefix("word/").unwrap_or(&name);
        Ok(self.rels.add("image", relative, false))
    }

    fn image_run(&mut self, img: &InlineImage) -> Result<Node, DocxError> {
        let rid = self.media_rel(&img.content_id)?;
        self.drawings += 1;
        let id = self.drawings;
        let cx = i64::from(img.width.0) * EMU_PER_PT100;
        let cy = i64::from(img.height.0) * EMU_PER_PT100;
        let mut doc_pr = Node::new("wp:docPr")
            .num("id", id)
            .attr("name", &format!("Picture {id}"));
        if let Some(alt) = &img.alt {
            doc_pr = doc_pr.attr("descr", alt);
        }
        let extent = |name: &str| Node::new(name).num("cx", cx).num("cy", cy);
        let picture = Node::new("pic:pic")
            .child(
                Node::new("pic:nvPicPr")
                    .child(
                        Node::new("pic:cNvPr")
                            .num("id", id)
                            .attr("name", &format!("image{id}")),
                    )
                    .child(Node::new("pic:cNvPicPr")),
            )
            .child(
                Node::new("pic:blipFill")
                    .child(Node::new("a:blip").attr("r:embed", &rid))
                    .child(Node::new("a:stretch").child(Node::new("a:fillRect"))),
            )
            .child(
                Node::new("pic:spPr")
                    .child(
                        Node::new("a:xfrm")
                            .child(Node::new("a:off").num("x", 0).num("y", 0))
                            .child(extent("a:ext")),
                    )
                    .child(
                        Node::new("a:prstGeom")
                            .attr("prst", "rect")
                            .child(Node::new("a:avLst")),
                    ),
            );
        let inline = Node::new("wp:inline")
            .attr("distT", "0")
            .attr("distB", "0")
            .attr("distL", "0")
            .attr("distR", "0")
            .child(extent("wp:extent"))
            .child(doc_pr)
            .child(
                Node::new("a:graphic").child(
                    Node::new("a:graphicData")
                        .attr("uri", PICTURE_URI)
                        .child(picture),
                ),
            );
        Ok(run(None, Node::new("w:drawing").child(inline)))
    }
}
