//! Attribute names derived from the model: a fully populated instance of each properties type
//! is serialised and its keys are the node's attributes. A field added to the model therefore
//! appears in the schema as soon as it is set here; the round-trip tests fail if it is not,
//! because every attribute the encoder emits must be in the schema.

use crate::names::attr;
use cdoc_model::{
    Alignment, Border, BorderStyle, BorderWidth, Borders, CellProperties, Color, Columns, Field,
    FieldKind, FirstLine, FormField, FormFieldKind, HeaderFooterSet, Indent, InlineImage, Length,
    LineSpacing, Link, Margins, NumId, NumberFormat, NumberingRef, Orientation, PageAnchor,
    PageNumbering, PageSetup, PageSize, ParagraphProperties, SectionProperties, SectionStart,
    Shading, SignatureField, Spacing, StyleId, TableProperties, TableRow, TableWidth, Underline,
    UnderlineStyle, VerticalCellAlign,
};
use serde::Serialize;

fn keys<T: Serialize>(value: &T) -> Vec<String> {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::Object(map)) => map.keys().cloned().collect(),
        other => unreachable!("properties serialise to an object: {other:?}"),
    }
}

fn black() -> Color {
    Color::rgb(0, 0, 0)
}

fn border() -> Border {
    Border {
        style: BorderStyle::Single,
        width: BorderWidth(4),
        color: Some(black()),
        space: Length(0),
    }
}

fn borders() -> Borders {
    Borders {
        between: Some(border()),
        ..Borders::all(border())
    }
}

fn shading() -> Shading {
    Shading { fill: black() }
}

pub(super) fn paragraph_attrs() -> Vec<String> {
    keys(&ParagraphProperties {
        style: Some(StyleId::new("Normal")),
        alignment: Some(Alignment::Left),
        spacing: Some(Spacing {
            before: Some(Length(0)),
            after: Some(Length(0)),
            line: Some(LineSpacing::Multiple { lines: 240 }),
        }),
        indent: Some(Indent {
            left: Some(Length(0)),
            right: Some(Length(0)),
            first_line: Some(FirstLine::Indent { by: Length(0) }),
        }),
        borders: Some(borders()),
        shading: Some(shading()),
        keep_with_next: Some(true),
        keep_lines: Some(true),
        page_break_before: Some(true),
        numbering: Some(NumberingRef {
            num_id: NumId(1),
            level: 0,
        }),
        outline_level: Some(0),
    })
}

pub(super) fn section_attrs() -> Vec<String> {
    let mut attrs = keys(&SectionProperties {
        start: SectionStart::NextPage,
        page: PageSetup {
            size: PageSize {
                width: Length(1),
                height: Length(1),
                orientation: Orientation::Portrait,
            },
            margins: Margins::NORMAL,
            columns: Columns::ONE,
            page_numbering: PageNumbering {
                format: NumberFormat::Decimal,
                start: Some(1),
            },
        },
        headers: HeaderFooterSet::none(),
        footers: HeaderFooterSet::none(),
        title_page: true,
    });
    attrs.retain(|k| k != "headers" && k != "footers");
    attrs
}

pub(super) fn table_attrs() -> Vec<String> {
    let mut attrs = keys(&TableProperties {
        style: Some(StyleId::new("TableGrid")),
        width: TableWidth::Auto,
        alignment: Some(Alignment::Left),
        borders: Some(borders()),
        cell_padding: Some(Length(0)),
    });
    attrs.push(attr::COLUMN_WIDTHS.to_owned());
    attrs
}

pub(super) fn row_attrs() -> Vec<String> {
    let mut attrs = keys(&TableRow {
        height: Some(cdoc_model::RowHeight::Exact { height: Length(1) }),
        cant_split: true,
        cells: Vec::new(),
    });
    attrs.retain(|k| k != "cells");
    attrs
}

pub(super) fn cell_attrs() -> Vec<String> {
    let mut attrs = keys(&CellProperties {
        width: Some(TableWidth::Auto),
        borders: Some(borders()),
        shading: Some(shading()),
        vertical_align: Some(VerticalCellAlign::Top),
        ..CellProperties::single()
    });
    attrs.retain(|k| k != "colSpan" && k != "rowSpan");
    attrs.push(attr::COLSPAN.to_owned());
    attrs.push(attr::ROWSPAN.to_owned());
    attrs
}

pub(super) fn image_attrs() -> Vec<String> {
    keys(&InlineImage {
        content_id: cdoc_model::ContentId::new("x"),
        width: Length(1),
        height: Length(1),
        alt: Some(String::new()),
    })
}

pub(super) fn field_attrs() -> Vec<String> {
    keys(&Field {
        field: FieldKind::PageNumber,
        cached: String::new(),
    })
}

pub(super) fn form_field_attrs() -> Vec<String> {
    keys(&FormField {
        id: cdoc_model::FieldId::new("x"),
        name: Some(String::new()),
        field: FormFieldKind::Checkbox { checked: false },
    })
}

pub(super) fn signature_field_attrs() -> Vec<String> {
    keys(&SignatureField {
        id: cdoc_model::FieldId::new("x"),
        signer_role: String::new(),
        label: Some(String::new()),
        anchor: Some(PageAnchor {
            page: 0,
            x: Length(0),
            y: Length(0),
            width: Length(1),
            height: Length(1),
        }),
    })
}

pub(super) fn underline_attrs() -> Vec<String> {
    keys(&Underline {
        style: UnderlineStyle::Single,
        color: Some(black()),
    })
}

pub(super) fn link_attrs() -> Vec<String> {
    keys(&Link {
        href: String::new(),
        title: Some(String::new()),
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn section_attrs_exclude_children() {
        let attrs = super::section_attrs();
        assert!(attrs.contains(&"page".to_owned()));
        assert!(!attrs.contains(&"headers".to_owned()));
    }

    #[test]
    fn cell_attrs_use_prosemirror_tables_spelling() {
        let attrs = super::cell_attrs();
        assert!(attrs.contains(&"colspan".to_owned()));
        assert!(!attrs.contains(&"colSpan".to_owned()));
    }
}
