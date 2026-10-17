//! Automatic styles: one per distinct combination of family, parent, extra attributes,
//! property elements and residual, named `P1`, `T1`, `Ce1`… in each part (`M`-prefixed in
//! `styles.xml`, as `LibreOffice` does). Unique styles (paragraph continuations, padding cells)
//! are never shared.

use crate::residual::Residual;
use cdoc_pkg::xml::Node;
use std::collections::BTreeSet;

/// One automatic style.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Auto {
    /// `paragraph`, `text`, `table`, `table-column`, `table-row`, `table-cell`.
    pub family: &'static str,
    /// The named style it is based on.
    pub parent: Option<String>,
    /// Attributes of `style:style` itself (`style:master-page-name`).
    pub attrs: Vec<(String, String)>,
    /// Property elements.
    pub props: Vec<Node>,
    /// What the properties cannot hold.
    pub residual: Residual,
}

impl Auto {
    /// A style of `family` with these property elements and nothing else.
    #[must_use]
    pub fn new(family: &'static str, props: Vec<Node>) -> Self {
        Self {
            family,
            parent: None,
            attrs: Vec::new(),
            props,
            residual: Residual::none(),
        }
    }

    /// Whether the style would say nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.parent.is_none()
            && self.attrs.is_empty()
            && self.residual.is_empty()
            && self
                .props
                .iter()
                .all(|p| p.attrs.is_empty() && p.children.is_empty())
    }
}

/// The automatic styles of one part.
#[derive(Debug, Clone)]
pub struct Registry {
    prefix: &'static str,
    /// Name, style, and whether the style may be shared.
    styles: Vec<(String, Auto, bool)>,
    taken: BTreeSet<(String, String)>,
}

fn letter(family: &str) -> &'static str {
    match family {
        "paragraph" => "P",
        "text" => "T",
        "table" => "Ta",
        "table-column" => "Co",
        "table-row" => "Ro",
        "table-cell" => "Ce",
        _ => "S",
    }
}

impl Registry {
    /// An empty registry naming its styles `{prefix}{letter}{n}`, avoiding `named` (family,
    /// name) pairs.
    #[must_use]
    pub fn new(prefix: &'static str, named: BTreeSet<(String, String)>) -> Self {
        Self {
            prefix,
            styles: Vec::new(),
            taken: named,
        }
    }

    /// The name of a style equal to `style`, registering it if new.
    pub fn name(&mut self, style: Auto) -> String {
        if let Some((name, _, _)) = self
            .styles
            .iter()
            .find(|(_, s, shared)| *shared && *s == style)
        {
            return name.clone();
        }
        self.add(style, true)
    }

    /// Register a style that is never shared.
    pub fn unique(&mut self, style: Auto) -> String {
        self.add(style, false)
    }

    fn add(&mut self, style: Auto, shared: bool) -> String {
        let mut n = self
            .styles
            .iter()
            .filter(|(_, s, _)| s.family == style.family)
            .count()
            + 1;
        let name = loop {
            let candidate = format!("{}{}{n}", self.prefix, letter(style.family));
            if self
                .taken
                .insert((style.family.to_owned(), candidate.clone()))
            {
                break candidate;
            }
            n += 1;
        };
        self.styles.push((name.clone(), style, shared));
        name
    }

    /// Each style with its name, in registration order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &Auto)> {
        self.styles.iter().map(|(n, s, _)| (n.as_str(), s))
    }

    /// The `style:style` elements.
    #[must_use]
    pub fn nodes(&self) -> Vec<Node> {
        self.iter()
            .map(|(name, s)| {
                let mut node = Node::new("style:style")
                    .attr("style:name", name)
                    .attr("style:family", s.family);
                if let Some(parent) = &s.parent {
                    node = node.attr("style:parent-style-name", parent);
                }
                for (k, v) in &s.attrs {
                    node = node.attr(k, v);
                }
                node.children(
                    s.props
                        .iter()
                        .filter(|p| !p.attrs.is_empty() || !p.children.is_empty())
                        .cloned(),
                )
            })
            .collect()
    }
}
