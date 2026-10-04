//! Dangling references. Files in the wild name styles that are not defined, lists that do not
//! exist and comments with no body; the model requires every id to resolve. Each such
//! reference is dropped, and the drop is reported.

use crate::report::Log;
use cdoc_model::{
    Document, InlineContent, NumberingRef, Report, RunProperties, StyleId, StyleKind, StyleSheet,
};
use std::collections::BTreeMap;

fn drop_wrong_style(
    style: &mut Option<StyleId>,
    kinds: &BTreeMap<StyleId, StyleKind>,
    wanted: StyleKind,
    what: &str,
    log: &mut Log,
) {
    if let Some(id) = style {
        if kinds.get(id) != Some(&wanted) {
            log.lose(what);
            *style = None;
        }
    }
}

fn break_cycles(sheet: &mut StyleSheet, log: &mut Log) {
    for i in 0..sheet.styles.len() {
        let id = sheet.styles[i].id.clone();
        if sheet.chain(&id).is_err() {
            log.lose("a style in a basedOn cycle; link dropped");
            sheet.styles[i].based_on = None;
        }
    }
}

/// Drop every reference in `doc` that does not resolve, returning what was dropped.
pub fn repair_references(doc: &mut Document) -> Report {
    let mut log = Log::new("(model)");
    let kinds: BTreeMap<StyleId, StyleKind> = doc
        .styles
        .styles
        .iter()
        .map(|s| (s.id.clone(), s.kind))
        .collect();
    for style in &mut doc.styles.styles {
        if style
            .based_on
            .as_ref()
            .is_some_and(|b| !kinds.contains_key(b))
        {
            log.lose("a style based on an undefined style; link dropped");
            style.based_on = None;
        }
    }
    break_cycles(&mut doc.styles, &mut log);
    let run_fix = |props: &mut RunProperties, log: &mut Log| {
        drop_wrong_style(
            &mut props.style,
            &kinds,
            StyleKind::Character,
            "a character style that is not defined; dropped",
            log,
        );
    };
    run_fix(&mut doc.styles.defaults.run, &mut log);
    for style in &mut doc.styles.styles {
        run_fix(&mut style.run, &mut log);
    }
    let abstracts: Vec<_> = doc.numbering.definitions.iter().map(|d| d.id).collect();
    let before = doc.numbering.instances.len();
    doc.numbering
        .instances
        .retain(|i| abstracts.contains(&i.abstract_id));
    for _ in doc.numbering.instances.len()..before {
        log.lose("a list instance of an undefined list definition; dropped");
    }
    let levels: BTreeMap<u32, usize> = doc
        .numbering
        .instances
        .iter()
        .filter_map(|i| {
            let def = doc
                .numbering
                .definitions
                .iter()
                .find(|d| d.id == i.abstract_id)?;
            Some((i.id.0, def.levels.len()))
        })
        .collect();
    let footnotes: Vec<_> = doc.footnotes.iter().map(|f| f.id).collect();
    let comments: Vec<_> = doc.comments.iter().map(|c| c.id.clone()).collect();
    doc.for_each_paragraph_mut(&mut |p| {
        drop_wrong_style(
            &mut p.properties.style,
            &kinds,
            StyleKind::Paragraph,
            "a paragraph style that is not defined; dropped",
            &mut log,
        );
        if let Some(NumberingRef { num_id, level }) = p.properties.numbering {
            if levels
                .get(&num_id.0)
                .is_none_or(|n| usize::from(level) >= *n)
            {
                log.lose("a paragraph in a list that is not defined; list dropped");
                p.properties.numbering = None;
            }
        }
        let before = p.inlines.len();
        p.inlines.retain(|i| match &i.content {
            InlineContent::FootnoteRef { id } => footnotes.contains(id),
            _ => true,
        });
        for _ in p.inlines.len()..before {
            log.lose("a footnote reference with no footnote; dropped");
        }
        for inline in &mut p.inlines {
            if let InlineContent::Text(run) = &mut inline.content {
                run_fix(&mut run.properties, &mut log);
            }
            let n = inline.marks.comments.len();
            inline.marks.comments.retain(|c| comments.contains(c));
            for _ in inline.marks.comments.len()..n {
                log.lose("a comment anchor with no comment; dropped");
            }
        }
    });
    doc.for_each_table_mut(&mut |t| {
        drop_wrong_style(
            &mut t.properties.style,
            &kinds,
            StyleKind::Table,
            "a table style that is not defined; dropped",
            &mut log,
        );
    });
    log.report
}
