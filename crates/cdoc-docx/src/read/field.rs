//! Fields: the instruction decides the kind; the result text is kept as the cached value.

use super::paragraph::{Context, ParaState};
use super::Reader;
use crate::xml::El;
use cdoc_model::{Field, FieldKind, Inline, InlineContent, Link, Marks, RunProperties, TextRun};

/// A complex field between its `begin` and `end` marks.
pub(super) struct FieldState {
    pub instr: String,
    pub result: String,
    pub in_result: bool,
    pub depth: u32,
    pub marks: Marks,
    pub rpr: RunProperties,
}

impl FieldState {
    pub(super) const fn begin(marks: Marks, rpr: RunProperties) -> Self {
        Self {
            instr: String::new(),
            result: String::new(),
            in_result: false,
            depth: 1,
            marks,
            rpr,
        }
    }
}

/// The date picture after `\@` in a `DATE` instruction.
fn date_format(instr: &str) -> Option<String> {
    let rest = instr.split_once("\\@")?.1.trim_start();
    let Some(quoted) = rest.strip_prefix('"') else {
        return rest.split_whitespace().next().map(str::to_owned);
    };
    let mut out = String::new();
    let mut chars = quoted.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => out.extend(chars.next()),
            '"' => break,
            c => out.push(c),
        }
    }
    Some(out)
}

/// The words of a field instruction, with quoted strings kept whole.
fn words(instr: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut chars = instr.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' if quoted => current.extend(chars.next()),
            '"' => {
                quoted = !quoted;
                if !quoted {
                    out.push(std::mem::take(&mut current));
                }
            }
            c if c.is_whitespace() && !quoted => {
                if !current.is_empty() {
                    out.push(std::mem::take(&mut current));
                }
            }
            c => current.push(c),
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

/// The link a `HYPERLINK` instruction makes: `HYPERLINK "url" \o "tip"` or `HYPERLINK \l "anchor"`.
fn hyperlink(instr: &str) -> Option<Link> {
    let (mut target, mut anchor, mut title) = (None, None, None);
    let mut it = words(instr).into_iter().skip(1);
    while let Some(word) = it.next() {
        match word.as_str() {
            "\\l" => anchor = it.next(),
            "\\o" => title = it.next(),
            w if w.starts_with('\\') => {}
            _ => target = target.or(Some(word)),
        }
    }
    let href = match (target, anchor) {
        (Some(t), Some(a)) => format!("{t}#{a}"),
        (Some(t), None) => t,
        (None, Some(a)) => format!("#{a}"),
        (None, None) => return None,
    };
    Some(Link { href, title })
}

impl Reader<'_> {
    pub(super) fn finish_field(&mut self, field: FieldState, st: &mut ParaState) {
        let keyword = field
            .instr
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .to_ascii_uppercase();
        let kind = match keyword.as_str() {
            "PAGE" => Some(FieldKind::PageNumber),
            "NUMPAGES" => Some(FieldKind::PageCount),
            "DATE" => {
                let format = date_format(&field.instr);
                if format.is_none() {
                    self.log.lose("a DATE field without a date picture");
                }
                Some(FieldKind::Date {
                    format: format.unwrap_or_default(),
                })
            }
            _ => None,
        };
        let mut field = field;
        if keyword == "HYPERLINK" {
            if let Some(link) = hyperlink(&field.instr) {
                field.marks.link = Some(link);
            }
        }
        let content = if keyword == "HYPERLINK" && !field.result.is_empty() {
            InlineContent::Text(TextRun {
                text: field.result,
                properties: field.rpr,
            })
        } else if let Some(kind) = kind {
            InlineContent::Field(Field {
                field: kind,
                cached: field.result,
            })
        } else {
            self.log
                .lose(&format!("field {keyword}; its result is kept as text"));
            if field.result.is_empty() {
                return;
            }
            InlineContent::Text(TextRun {
                text: field.result,
                properties: field.rpr,
            })
        };
        st.inlines.push(Inline {
            content,
            marks: field.marks,
        });
    }

    /// `<w:fldSimple w:instr="..">runs</w:fldSimple>`.
    pub(super) fn simple_field(&mut self, node: El<'_>, ctx: &Context, st: &mut ParaState) {
        let mut result = String::new();
        collect_text(node, &mut result);
        let rpr = node
            .find("w:r")
            .and_then(|r| r.find("w:rPr"))
            .map_or_else(RunProperties::none, |n| {
                crate::props::read_rpr(n, &mut self.log)
            });
        let mut field = FieldState::begin(st.marks(ctx), rpr);
        node.get("w:instr")
            .unwrap_or_default()
            .clone_into(&mut field.instr);
        field.result = result;
        self.finish_field(field, st);
    }
}

fn collect_text(node: El<'_>, out: &mut String) {
    for child in node.elems() {
        match child.name() {
            "w:t" | "w:delText" => out.push_str(&child.content()),
            _ => collect_text(child, out),
        }
    }
}
