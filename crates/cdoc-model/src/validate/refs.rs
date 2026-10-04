//! Reference integrity: every id the tree names must exist, once, and be of the right kind.

use super::{check_unique, ValidationError};
use crate::inline::InlineContent;
use crate::numbering::MAX_LEVEL;
use crate::paragraph::ParagraphProperties;
use crate::style::StyleKind;
use crate::Document;

pub(super) fn check(doc: &Document, errors: &mut Vec<ValidationError>) {
    styles(doc, errors);
    numbering(doc, errors);
    footnotes(doc, errors);
    comments(doc, errors);
    fields(doc, errors);
}

fn styles(doc: &Document, errors: &mut Vec<ValidationError>) {
    check_unique(
        doc.styles.styles.iter().map(|s| s.id.clone()),
        ValidationError::DuplicateStyle,
        errors,
    );
    for style in &doc.styles.styles {
        if let Err(e) = doc.styles.chain(&style.id) {
            errors.push(e.into());
        }
    }
    let mut paragraph_props: Vec<&ParagraphProperties> = vec![&doc.styles.defaults.paragraph];
    paragraph_props.extend(doc.paragraphs().map(|p| &p.properties));
    for props in paragraph_props {
        if let Some(id) = &props.style {
            match doc.styles.get(id) {
                None => errors.push(ValidationError::Style(crate::style::StyleError::Unknown(
                    id.clone(),
                ))),
                Some(s) if s.kind != StyleKind::Paragraph => {
                    errors.push(ValidationError::StyleKind(id.clone(), "paragraph"));
                }
                Some(_) => {}
            }
        }
    }
    for props in doc.run_properties() {
        if let Some(id) = &props.style {
            match doc.styles.get(id) {
                None => errors.push(ValidationError::Style(crate::style::StyleError::Unknown(
                    id.clone(),
                ))),
                Some(s) if s.kind != StyleKind::Character => {
                    errors.push(ValidationError::StyleKind(id.clone(), "character"));
                }
                Some(_) => {}
            }
        }
    }
    for table in doc.blocks().flat_map(|(_, b)| b.tables()) {
        if let Some(id) = &table.properties.style {
            match doc.styles.get(id) {
                None => errors.push(ValidationError::Style(crate::style::StyleError::Unknown(
                    id.clone(),
                ))),
                Some(s) if s.kind != StyleKind::Table => {
                    errors.push(ValidationError::StyleKind(id.clone(), "table"));
                }
                Some(_) => {}
            }
        }
    }
}

fn numbering(doc: &Document, errors: &mut Vec<ValidationError>) {
    let numbering = &doc.numbering;
    check_unique(
        numbering.definitions.iter().map(|d| d.id.0),
        ValidationError::DuplicateNumbering,
        errors,
    );
    check_unique(
        numbering.instances.iter().map(|i| i.id.0),
        ValidationError::DuplicateNumbering,
        errors,
    );
    for definition in &numbering.definitions {
        if definition.levels.is_empty() || definition.levels.len() > usize::from(MAX_LEVEL) + 1 {
            errors.push(ValidationError::LevelCount(definition.id));
        }
    }
    for instance in &numbering.instances {
        let Some(definition) = numbering
            .definitions
            .iter()
            .find(|d| d.id == instance.abstract_id)
        else {
            errors.push(ValidationError::UnknownAbstractNumbering(
                instance.abstract_id,
            ));
            continue;
        };
        for over in &instance.overrides {
            if usize::from(over.level) >= definition.levels.len() {
                errors.push(ValidationError::Level {
                    num_id: instance.id,
                    level: over.level,
                });
            }
        }
    }
    for paragraph in doc.paragraphs() {
        if let Some(r) = &paragraph.properties.numbering {
            match numbering.definition_for(r.num_id) {
                None if numbering.instances.iter().any(|i| i.id == r.num_id) => {}
                None => errors.push(ValidationError::UnknownNumbering(r.num_id)),
                Some(definition) if usize::from(r.level) >= definition.levels.len() => {
                    errors.push(ValidationError::Level {
                        num_id: r.num_id,
                        level: r.level,
                    });
                }
                Some(_) => {}
            }
        }
    }
}

fn footnotes(doc: &Document, errors: &mut Vec<ValidationError>) {
    check_unique(
        doc.footnotes.iter().map(|f| f.id),
        ValidationError::DuplicateFootnote,
        errors,
    );
    for inline in doc.inlines() {
        if let InlineContent::FootnoteRef { id } = &inline.content {
            if !doc.footnotes.iter().any(|f| f.id == *id) {
                errors.push(ValidationError::UnknownFootnote(*id));
            }
        }
    }
}

fn comments(doc: &Document, errors: &mut Vec<ValidationError>) {
    check_unique(
        doc.comments.iter().map(|c| c.id.clone()),
        ValidationError::DuplicateComment,
        errors,
    );
    let exists = |id| doc.comments.iter().any(|c| &c.id == id);
    for comment in &doc.comments {
        if let Some(parent) = &comment.parent {
            if parent == &comment.id {
                errors.push(ValidationError::SelfReply(comment.id.clone()));
            } else if !exists(parent) {
                errors.push(ValidationError::UnknownComment(parent.clone()));
            }
        }
    }
    for inline in doc.inlines() {
        for id in &inline.marks.comments {
            if !exists(id) {
                errors.push(ValidationError::UnknownComment(id.clone()));
            }
        }
    }
}

fn fields(doc: &Document, errors: &mut Vec<ValidationError>) {
    let ids = doc.inlines().filter_map(|i| match &i.content {
        InlineContent::FormField(f) => Some(f.id.clone()),
        InlineContent::SignatureField(f) => Some(f.id.clone()),
        _ => None,
    });
    check_unique(ids, ValidationError::DuplicateField, errors);
}
