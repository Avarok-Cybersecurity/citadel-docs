//! Where every style lives: font faces, the common styles of `styles.xml`, the automatic styles
//! of each part, list styles, date styles, page layouts and master pages. A name in a part is
//! looked up in that part's automatic styles first, then in the common styles.

use crate::ns::Part;
use crate::props::face_family;
use cdoc_pkg::xml::Node;
use std::collections::BTreeMap;

type Families<'a> = BTreeMap<(String, String), &'a Node>;

/// The styles of a document, by kind and name.
#[derive(Debug)]
pub struct Index<'a> {
    /// `style:font-name` to family.
    pub faces: BTreeMap<String, String>,
    /// `office:styles` `style:style`, by (family, name), and in document order.
    pub common: Families<'a>,
    /// Common styles in document order.
    pub common_order: Vec<&'a Node>,
    /// Automatic styles of `content.xml` and `styles.xml`.
    pub autos: [Families<'a>; 2],
    /// List styles: common ones and each part's automatic ones, in that order of definition.
    pub lists: Vec<(Option<Part>, &'a Node)>,
    /// Date styles: common, and each part's.
    pub dates: [BTreeMap<String, &'a Node>; 3],
    /// Page layouts.
    pub layouts: BTreeMap<String, &'a Node>,
    /// Master pages, in document order.
    pub masters: Vec<&'a Node>,
    /// The paragraph family's default style.
    pub default_paragraph: Option<&'a Node>,
    /// Style entries no reader maps, for the report.
    pub unread: Vec<String>,
}

fn named(node: &Node) -> Option<(String, String)> {
    Some((
        node.get("style:family")?.to_owned(),
        node.get("style:name")?.to_owned(),
    ))
}

const fn slot(part: Part) -> usize {
    match part {
        Part::Content => 0,
        Part::Styles => 1,
    }
}

impl<'a> Index<'a> {
    /// Index both parts.
    #[must_use]
    pub fn build(content: &'a Node, styles: Option<&'a Node>) -> Self {
        let mut index = Self {
            faces: BTreeMap::new(),
            common: BTreeMap::new(),
            common_order: Vec::new(),
            autos: [BTreeMap::new(), BTreeMap::new()],
            lists: Vec::new(),
            dates: [BTreeMap::new(), BTreeMap::new(), BTreeMap::new()],
            layouts: BTreeMap::new(),
            masters: Vec::new(),
            default_paragraph: None,
            unread: Vec::new(),
        };
        let parts: Vec<(Part, &Node)> = std::iter::once((Part::Content, content))
            .chain(styles.map(|s| (Part::Styles, s)))
            .collect();
        for (_, root) in &parts {
            for face in root
                .find("office:font-face-decls")
                .into_iter()
                .flat_map(Node::elems)
            {
                if let Some(name) = face.get("style:name") {
                    let family = face_family(name, face.get("svg:font-family"));
                    index.faces.insert(name.to_owned(), family);
                }
            }
        }
        if let Some(styles) = styles {
            for node in styles
                .find("office:styles")
                .into_iter()
                .flat_map(Node::elems)
            {
                index.add_common(node);
            }
            for node in styles
                .find("office:master-styles")
                .into_iter()
                .flat_map(Node::elems)
            {
                if node.name == "style:master-page" {
                    index.masters.push(node);
                }
            }
        }
        for (part, root) in parts {
            for node in root
                .find("office:automatic-styles")
                .into_iter()
                .flat_map(Node::elems)
            {
                index.add_auto(part, node);
            }
        }
        index
    }

    fn add_common(&mut self, node: &'a Node) {
        match node.name.as_str() {
            "style:style" => {
                if let Some(key) = named(node) {
                    self.common.insert(key, node);
                    self.common_order.push(node);
                }
            }
            "style:default-style" => match node.get("style:family") {
                Some("paragraph") => self.default_paragraph = Some(node),
                Some(other) => self
                    .unread
                    .push(format!("style:default-style of family {other}")),
                None => {}
            },
            "text:list-style" => self.lists.push((None, node)),
            "number:date-style" => {
                if let Some(name) = node.get("style:name") {
                    self.dates[2].insert(name.to_owned(), node);
                }
            }
            _ => self.unread.push(format!("office:styles/{}", node.name)),
        }
    }

    fn add_auto(&mut self, part: Part, node: &'a Node) {
        match node.name.as_str() {
            "style:style" => {
                if let Some(key) = named(node) {
                    self.autos[slot(part)].insert(key, node);
                }
            }
            "text:list-style" => self.lists.push((Some(part), node)),
            "number:date-style" => {
                if let Some(name) = node.get("style:name") {
                    self.dates[slot(part)].insert(name.to_owned(), node);
                }
            }
            "style:page-layout" => {
                if let Some(name) = node.get("style:name") {
                    self.layouts.insert(name.to_owned(), node);
                }
            }
            _ => self
                .unread
                .push(format!("office:automatic-styles/{}", node.name)),
        }
    }

    /// A style of `family` named `name` as seen from `part`, and whether it is automatic.
    #[must_use]
    pub fn style(&self, part: Part, family: &str, name: &str) -> Option<(&'a Node, bool)> {
        let key = (family.to_owned(), name.to_owned());
        if let Some(node) = self.autos[slot(part)].get(&key) {
            return Some((node, true));
        }
        self.common.get(&key).map(|n| (*n, false))
    }

    /// A date style as seen from `part`.
    #[must_use]
    pub fn date(&self, part: Part, name: &str) -> Option<&'a Node> {
        self.dates[slot(part)]
            .get(name)
            .or_else(|| self.dates[2].get(name))
            .copied()
    }

    /// A master page by name.
    #[must_use]
    pub fn master(&self, name: &str) -> Option<&'a Node> {
        self.masters
            .iter()
            .find(|m| m.get("style:name") == Some(name))
            .copied()
    }
}
