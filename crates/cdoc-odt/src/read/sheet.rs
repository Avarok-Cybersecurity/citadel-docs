//! The style sheet and the numbering definitions, and the automatic styles a paragraph or span
//! names: each read with the shared property readers, then corrected by its sidecar residual.

use super::Reader;
use crate::lists::read_list_style;
use crate::props::{read_paragraph, read_run, PARAGRAPH_PROPERTIES, TEXT_PROPERTIES};
use crate::styles::{read_defaults, read_style};
use cdoc_model::{
    AbstractNumId, AbstractNumbering, DocumentDefaults, Length, ParagraphProperties, RunProperties,
    StyleId, StyleSheet,
};
use cdoc_pkg::xml::Node;
use serde::{de::DeserializeOwned, Serialize};

/// What a paragraph's style name resolves to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParaStyle {
    /// The paragraph properties, with the named style set.
    pub props: ParagraphProperties,
    /// Text properties the style gives every run (other producers put them there).
    pub run: RunProperties,
    /// The master page the paragraph starts.
    pub master: Option<String>,
    /// The page number it restarts at.
    pub page_number: Option<u32>,
    /// Whether the style puts the paragraph in a list (`style:list-style-name`).
    pub list: bool,
}

impl ParaStyle {
    fn named(style: Option<StyleId>) -> Self {
        Self {
            props: ParagraphProperties {
                style,
                ..ParagraphProperties::none()
            },
            run: RunProperties::none(),
            master: None,
            page_number: None,
            list: false,
        }
    }
}

const AUTO_ATTRS: [&str; 6] = [
    "style:list-style-name",
    "style:name",
    "style:family",
    "style:parent-style-name",
    "style:master-page-name",
    "style:display-name",
];

impl Reader<'_> {
    /// `native` corrected by the sidecar's residual for `(family, name)`.
    pub(super) fn fixed<T: Serialize + DeserializeOwned>(
        &mut self,
        family: &str,
        name: &str,
        native: T,
    ) -> T {
        let Some(residual) = self
            .sidecar
            .residuals
            .get(&(family.to_owned(), name.to_owned()))
        else {
            return native;
        };
        if let Ok(fixed) = residual.apply(&native) {
            fixed
        } else {
            self.log.lose("a damaged sidecar entry; the ODF value used");
            native
        }
    }

    /// The model id of a named style.
    pub(super) fn style_id(&self, family: &str, name: &str) -> StyleId {
        self.style_ids
            .get(&(family.to_owned(), name.to_owned()))
            .cloned()
            .unwrap_or_else(|| StyleId::new(name))
    }

    /// The named styles, the defaults and the default tab stop.
    pub(super) fn sheet(&mut self) -> (StyleSheet, Option<Length>) {
        let mut styles = Vec::new();
        for node in self.index.common_order.clone() {
            let (Some(family), Some(name)) = (node.get("style:family"), node.get("style:name"))
            else {
                continue;
            };
            let Some(style) = read_style(node, &self.index.faces, &mut self.log) else {
                self.log.lose(&format!("a style of family {family}"));
                continue;
            };
            let style = self.fixed("style", &format!("{family}:{name}"), style);
            if node.get("style:list-style-name").is_some() && style.paragraph.numbering.is_none() {
                self.log.lose("a style's list (style:list-style-name)");
            }
            self.style_ids
                .insert((family.to_owned(), name.to_owned()), style.id.clone());
            styles.push(style);
        }
        for what in std::mem::take(&mut self.index.unread) {
            self.log.lose(&what);
        }
        let (defaults, tab) = match self.index.default_paragraph {
            Some(node) => read_defaults(node, &self.index.faces, &mut self.log),
            None => (
                DocumentDefaults {
                    run: RunProperties::none(),
                    paragraph: ParagraphProperties::none(),
                },
                None,
            ),
        };
        let defaults = self.fixed("default", "paragraph", defaults);
        (StyleSheet { defaults, styles }, tab)
    }

    /// One definition per list style, in index order.
    pub(super) fn definitions(&mut self) -> Vec<AbstractNumbering> {
        let mut out = Vec::new();
        for (i, (scope, node)) in self.index.lists.clone().into_iter().enumerate() {
            let id = AbstractNumId(u32::try_from(i + 1).unwrap_or(u32::MAX));
            let name = node.get("style:name").unwrap_or_default().to_owned();
            let def = read_list_style(node, id, &self.index.faces, &mut self.log);
            let def = if scope.is_none() {
                self.fixed("list", &name, def)
            } else {
                def
            };
            self.abstracts.push((scope, name, def.id));
            self.level_counts.insert(def.id, def.levels.len());
            out.push(def);
        }
        out
    }

    /// The definition a list style name denotes in the current part.
    pub(super) fn abstract_for(&self, name: &str) -> Option<AbstractNumId> {
        let find = |scope| {
            self.abstracts
                .iter()
                .find(|(s, n, _)| *s == scope && n == name)
                .map(|(_, _, id)| *id)
        };
        find(Some(self.part)).or_else(|| find(None))
    }

    fn check_auto(&mut self, node: &Node, props: &[&str]) {
        for (key, _) in &node.attrs {
            if !AUTO_ATTRS.contains(&key.as_str()) {
                self.log.lose(&format!("style:style@{key}"));
            }
        }
        for child in node.elems() {
            if !props.contains(&child.name.as_str()) {
                self.log.lose(&format!("style:style/{}", child.name));
            }
        }
    }

    fn auto_paragraph(&mut self, node: &Node, name: &str) -> ParaStyle {
        let empty = Node::new(PARAGRAPH_PROPERTIES);
        let p = node.find(PARAGRAPH_PROPERTIES).unwrap_or(&empty);
        let native = read_paragraph(p, &["style:page-number"], &mut self.log);
        let mut props = self.fixed("paragraph", name, native);
        props.style = node
            .get("style:parent-style-name")
            .map(|n| self.style_id("paragraph", n));
        let run = node
            .find(TEXT_PROPERTIES)
            .map_or_else(RunProperties::none, |t| {
                read_run(t, &self.index.faces, &mut self.log)
            });
        self.check_auto(node, &[PARAGRAPH_PROPERTIES, TEXT_PROPERTIES]);
        ParaStyle {
            props,
            run,
            master: node
                .get("style:master-page-name")
                .filter(|m| !m.is_empty())
                .map(str::to_owned),
            page_number: p.get("style:page-number").and_then(|n| n.parse().ok()),
            list: node.get("style:list-style-name").is_some(),
        }
    }

    /// What a paragraph's `text:style-name` resolves to.
    pub(super) fn para_style(&mut self, name: Option<&str>) -> ParaStyle {
        let Some(name) = name else {
            return ParaStyle::named(None);
        };
        let key = (self.part, name.to_owned());
        if let Some(found) = self.para_cache.get(&key) {
            return found.clone();
        }
        let style = match self.index.style(self.part, "paragraph", name) {
            Some((node, true)) => self.auto_paragraph(node, name),
            Some((_, false)) => ParaStyle::named(Some(self.style_id("paragraph", name))),
            None => {
                self.log
                    .lose("a paragraph naming a style that is not defined");
                ParaStyle::named(None)
            }
        };
        self.para_cache.insert(key, style.clone());
        style
    }

    /// What a span's `text:style-name` resolves to.
    pub(super) fn span_style(&mut self, name: &str) -> RunProperties {
        let key = (self.part, name.to_owned());
        if let Some(found) = self.span_cache.get(&key) {
            return found.clone();
        }
        let run = match self.index.style(self.part, "text", name) {
            Some((node, true)) => {
                let empty = Node::new(TEXT_PROPERTIES);
                let t = node.find(TEXT_PROPERTIES).unwrap_or(&empty);
                let native = read_run(t, &self.index.faces, &mut self.log);
                let mut run = self.fixed("text", name, native);
                run.style = node
                    .get("style:parent-style-name")
                    .map(|n| self.style_id("text", n));
                self.check_auto(node, &[TEXT_PROPERTIES]);
                run
            }
            Some((_, false)) => RunProperties {
                style: Some(self.style_id("text", name)),
                ..RunProperties::none()
            },
            None => {
                self.log.lose("a span naming a style that is not defined");
                RunProperties::none()
            }
        };
        self.span_cache.insert(key, run.clone());
        run
    }
}
