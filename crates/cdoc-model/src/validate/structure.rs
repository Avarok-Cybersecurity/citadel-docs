//! Structural and geometric constraints: non-empty containers, positive sizes, sane spans.

use super::ValidationError;
use crate::fields::FormFieldKind;
use crate::inline::InlineContent;
use crate::page::PageSetup;
use crate::paragraph::{LineSpacing, ParagraphProperties};
use crate::table::Table;
use crate::Document;

pub(super) fn check(doc: &Document, errors: &mut Vec<ValidationError>) {
    if doc.sections.is_empty() {
        errors.push(ValidationError::NoSections);
    }
    containers(doc, errors);
    geometry(doc, errors);
    typography(doc, errors);
    inlines(doc, errors);
    for table in doc.blocks().flat_map(|(_, b)| b.tables()) {
        check_table(table, errors);
    }
}

fn containers(doc: &Document, errors: &mut Vec<ValidationError>) {
    for (i, section) in doc.sections.iter().enumerate() {
        if section.blocks.is_empty() {
            errors.push(ValidationError::EmptyContainer(format!("section {i}")));
        }
        for (name, set) in [
            ("header", &section.properties.headers),
            ("footer", &section.properties.footers),
        ] {
            for (kind, hf) in set.present() {
                if hf.blocks.is_empty() {
                    errors.push(ValidationError::EmptyContainer(format!(
                        "section {i} {kind:?} {name}"
                    )));
                }
            }
        }
    }
    for footnote in &doc.footnotes {
        if footnote.blocks.is_empty() {
            errors.push(ValidationError::EmptyContainer(format!(
                "footnote {}",
                footnote.id.0
            )));
        }
    }
}

fn geometry(doc: &Document, errors: &mut Vec<ValidationError>) {
    for (section, s) in doc.sections.iter().enumerate() {
        errors.extend(s.properties.page.problems(section));
    }
}

impl PageSetup {
    /// What [`Document::validate`] reports about this page setup as section `section`'s.
    #[must_use]
    pub fn problems(&self, section: usize) -> Vec<ValidationError> {
        let mut errors = Vec::new();
        if !self.size.width.is_positive() || !self.size.height.is_positive() {
            errors.push(ValidationError::PageSize { section });
        }
        if self.margins.all().iter().any(|m| !m.is_non_negative()) {
            errors.push(ValidationError::NegativeMargin { section });
        }
        if self.columns.count == 0 {
            errors.push(ValidationError::Columns { section });
        }
        errors
    }
}

fn typography(doc: &Document, errors: &mut Vec<ValidationError>) {
    let defaults = &doc.styles.defaults.run;
    if defaults.font.is_none() || defaults.size.is_none() {
        errors.push(ValidationError::DefaultsIncomplete);
    }
    if doc
        .run_properties()
        .any(|r| r.size.is_some_and(|s| !s.is_valid()))
    {
        errors.push(ValidationError::FontSize);
    }
    let mut paragraph_props: Vec<&ParagraphProperties> = vec![&doc.styles.defaults.paragraph];
    paragraph_props.extend(doc.styles.styles.iter().map(|s| &s.paragraph));
    paragraph_props.extend(doc.paragraphs().map(|p| &p.properties));
    for props in paragraph_props {
        if let Some(level) = props.outline_level {
            if level > crate::numbering::MAX_LEVEL {
                errors.push(ValidationError::OutlineLevel(level));
            }
        }
        let bad_line = match props.spacing.as_ref().and_then(|s| s.line) {
            Some(LineSpacing::Multiple { lines }) => lines == 0,
            Some(LineSpacing::Exact { height } | LineSpacing::AtLeast { height }) => {
                !height.is_positive()
            }
            None => false,
        };
        if bad_line {
            errors.push(ValidationError::LineSpacing);
        }
    }
}

fn inlines(doc: &Document, errors: &mut Vec<ValidationError>) {
    for inline in doc.inlines() {
        if inline
            .marks
            .link
            .as_ref()
            .is_some_and(|l| l.href.is_empty())
        {
            errors.push(ValidationError::EmptyHref);
        }
        match &inline.content {
            InlineContent::Text(run) if run.text.is_empty() => {
                errors.push(ValidationError::EmptyText);
            }
            InlineContent::Image(img) if !img.width.is_positive() || !img.height.is_positive() => {
                errors.push(ValidationError::ImageSize);
            }
            InlineContent::FormField(field) => {
                if let FormFieldKind::Dropdown { options, selected } = &field.field {
                    let out_of_range = selected
                        .is_some_and(|s| usize::try_from(s).map_or(true, |s| s >= options.len()));
                    if options.is_empty() || out_of_range {
                        errors.push(ValidationError::Dropdown(field.id.clone()));
                    }
                }
            }
            _ => {}
        }
    }
}

fn check_table(table: &Table, errors: &mut Vec<ValidationError>) {
    if table.column_widths.is_empty() {
        errors.push(ValidationError::Table("no columns"));
    }
    if table.column_widths.iter().any(|w| !w.is_positive()) {
        errors.push(ValidationError::Table("a column width is not positive"));
    }
    if table.rows.is_empty() {
        errors.push(ValidationError::Table("no rows"));
    }
    for row in &table.rows {
        if row.cells.is_empty() {
            errors.push(ValidationError::Table("a row has no cells"));
        }
        let span: u64 = row
            .cells
            .iter()
            .map(|c| u64::from(c.properties.col_span))
            .sum();
        if span > table.column_widths.len() as u64 {
            errors.push(ValidationError::Table(
                "a row spans more columns than the grid",
            ));
        }
        for cell in &row.cells {
            if cell.properties.col_span == 0 || cell.properties.row_span == 0 {
                errors.push(ValidationError::Table("a cell has a zero span"));
            }
            if cell.blocks.is_empty() {
                errors.push(ValidationError::Table("a cell has no blocks"));
            }
        }
    }
}
