//! Form and signature fields as content controls. The control is native, so Word shows a
//! drop-down or a check box; the tag carries the exact model value.

use super::Writer;
use crate::xml::{Node, WordNode};
use cdoc_model::{FormField, FormFieldKind, SignatureField};
use cdoc_pkg::tag::Tagged;

fn display(text: &str, deleted: bool) -> Node {
    let name = if deleted { "w:delText" } else { "w:t" };
    let content = Node::new("w:sdtContent");
    if text.is_empty() {
        return content;
    }
    let t = Node::new(name).attr("xml:space", "preserve").text(text);
    content.child(Node::new("w:r").child(t))
}

fn control(field: &FormFieldKind) -> Node {
    match field {
        FormFieldKind::Text { .. } => Node::new("w:text"),
        FormFieldKind::Checkbox { checked } => Node::new("w14:checkbox")
            .child(Node::new("w14:checked").attr("w14:val", if *checked { "1" } else { "0" }))
            .child(
                Node::new("w14:checkedState")
                    .attr("w14:val", "2612")
                    .attr("w14:font", "MS Gothic"),
            )
            .child(
                Node::new("w14:uncheckedState")
                    .attr("w14:val", "2610")
                    .attr("w14:font", "MS Gothic"),
            ),
        FormFieldKind::Dropdown { options, selected } => {
            let mut list = Node::new("w:dropDownList");
            if let Some(s) = selected {
                list = list.num("w:lastValue", s);
            }
            list.children(options.iter().enumerate().map(|(i, o)| {
                Node::new("w:listItem")
                    .attr("w:displayText", o)
                    .num("w:value", i)
            }))
        }
        FormFieldKind::Date { value } => {
            let mut date = Node::new("w:date");
            if let Some(v) = value {
                date = date.attr("w:fullDate", &format!("{v}T00:00:00Z"));
            }
            date.child(Node::val("w:dateFormat", "yyyy-MM-dd"))
        }
    }
}

fn shown(field: &FormFieldKind) -> String {
    match field {
        FormFieldKind::Text { value, .. } => value.clone(),
        FormFieldKind::Checkbox { checked } => {
            if *checked { "\u{2612}" } else { "\u{2610}" }.to_owned()
        }
        FormFieldKind::Dropdown { options, selected } => selected
            .and_then(|s| options.get(usize::try_from(s).ok()?))
            .cloned()
            .unwrap_or_default(),
        FormFieldKind::Date { value } => value.clone().unwrap_or_default(),
    }
}

impl Writer<'_> {
    pub(super) fn form_field_sdt(field: &FormField, deleted: bool) -> Node {
        let mut pr = Node::new("w:sdtPr");
        if let Some(name) = &field.name {
            pr = pr.child(Node::val("w:alias", name));
        }
        pr = pr
            .child(Node::val("w:tag", &Tagged::Form(field.clone()).encode()))
            .child(control(&field.field));
        Node::new("w:sdt")
            .child(pr)
            .child(display(&shown(&field.field), deleted))
    }

    pub(super) fn signature_sdt(field: &SignatureField, deleted: bool) -> Node {
        let mut pr = Node::new("w:sdtPr");
        if let Some(label) = &field.label {
            pr = pr.child(Node::val("w:alias", label));
        }
        pr = pr.child(Node::val(
            "w:tag",
            &Tagged::Signature(field.clone()).encode(),
        ));
        Node::new("w:sdt")
            .child(pr)
            .child(display(field.label.as_deref().unwrap_or(""), deleted))
    }
}
