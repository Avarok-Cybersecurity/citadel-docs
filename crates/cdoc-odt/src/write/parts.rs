//! The package: `mimetype` (stored, first), the manifest, `content.xml`, `styles.xml`,
//! `meta.xml`, the pictures and the sidecar.

use super::master::section_start;
use super::Writer;
use crate::datestyle::write_date_style;
use crate::lists::{read_list_style, write_list_style};
use crate::ns;
use crate::ns::Part;
use crate::props::quote_family;
use crate::residual::Residual;
use crate::sidecar;
use crate::styles::{family, read_defaults, read_style, write_defaults, write_style};
use cdoc_model::{AbstractNumId, DocumentSettings};
use cdoc_pkg::xml::{serialize, Node};
use cdoc_pkg::{Log, Package, PackageError};
use std::collections::{BTreeMap, BTreeSet};

fn fonts(nodes: &[&Node], out: &mut BTreeSet<String>) {
    for node in nodes {
        if let Some(f) = node.get("style:font-name") {
            out.insert(f.to_owned());
        }
        let children: Vec<&Node> = node.elems().collect();
        fonts(&children, out);
    }
}

fn face_decls(fonts: &BTreeSet<String>) -> Node {
    Node::new("office:font-face-decls").children(fonts.iter().map(|f| {
        Node::new("style:font-face")
            .attr("style:name", f)
            .attr("svg:font-family", &quote_family(f))
    }))
}

impl Writer<'_> {
    pub(super) fn put(&mut self, out: &mut Package, name: &str, node: &Node) {
        let (bytes, replaced) = serialize(node);
        self.log.report.note_n(
            name,
            "characters not allowed in XML replaced with U+FFFD",
            u32::try_from(replaced).unwrap_or(u32::MAX),
        );
        out.put(name, bytes);
    }

    fn office_styles(&mut self) -> Node {
        let doc = self.doc;
        let mut scratch = Log::new("(readback)");
        let defaults = write_defaults(&doc.styles.defaults, doc.settings.default_tab_stop);
        let (back, _) = read_defaults(&defaults, &BTreeMap::new(), &mut scratch);
        self.keep(
            ("default", "paragraph"),
            Residual::between(&doc.styles.defaults, &back, &[]),
        );
        let mut styles = Node::new("office:styles").child(defaults);
        let mut seen = BTreeSet::new();
        for style in &doc.styles.styles {
            if !seen.insert(&style.id) {
                self.log
                    .lose("a style with the id of an earlier one dropped");
                continue;
            }
            let list = style.paragraph.numbering.and_then(|r| {
                let instance = doc.numbering.instances.iter().find(|i| i.id == r.num_id)?;
                self.list_style(instance.abstract_id)
            });
            let node = write_style(style, &self.names, list);
            if let Some(back) = read_style(&node, &BTreeMap::new(), &mut scratch) {
                let key = format!("{}:{}", family(style.kind), self.names.of(&style.id));
                self.keep(("style", &key), Residual::between(style, &back, &[]));
            }
            styles = styles.child(node);
        }
        let mut seen = BTreeSet::new();
        for (i, def) in doc.numbering.definitions.iter().enumerate() {
            if !seen.insert(def.id) {
                self.log
                    .lose("a list definition with the id of an earlier one dropped");
                continue;
            }
            let name = super::body::list_style_name(i);
            let node = write_list_style(&name, def);
            let id = AbstractNumId(u32::try_from(i + 1).unwrap_or(u32::MAX));
            let back = read_list_style(&node, id, &BTreeMap::new(), &mut scratch);
            self.keep(("list", &name), Residual::between(def, &back, &[]));
            styles = styles.child(node);
        }
        styles
    }

    fn keep(&mut self, (family, name): (&str, &str), residual: Residual) {
        if !residual.is_empty() {
            self.sidecar
                .residuals
                .insert((family.to_owned(), name.to_owned()), residual);
        }
    }

    fn automatic(&mut self, part: Part, extra: Vec<Node>) -> Node {
        let i = match part {
            Part::Content => 0,
            Part::Styles => 1,
        };
        let residuals: Vec<((String, String), Residual)> = self.autos[i]
            .iter()
            .filter(|(_, s)| !s.residual.is_empty())
            .map(|(n, s)| ((s.family.to_owned(), n.to_owned()), s.residual.clone()))
            .collect();
        self.sidecar.residuals.extend(residuals);
        let dates = self.dates[i].iter().map(|(n, p)| write_date_style(n, p));
        Node::new("office:automatic-styles")
            .children(extra)
            .children(self.autos[i].nodes())
            .children(dates)
    }

    fn settings_residual(&mut self, masters: &[Node]) {
        let even = masters.iter().flat_map(Node::elems).any(|band| {
            matches!(
                band.name.as_str(),
                "style:header-left" | "style:footer-left"
            ) && band.get("style:display") != Some("false")
        });
        let s = &self.doc.settings;
        let back = DocumentSettings {
            even_and_odd_headers: even,
            ..s.clone()
        };
        self.keep(("settings", ""), Residual::between(s, &back, &[]));
    }

    pub(super) fn meta(&self) -> Node {
        let mut meta =
            Node::new("office:meta").child(Node::new("meta:generator").text("Citadel Docs"));
        if let Some(t) = &self.doc.meta.title {
            meta = meta.child(Node::new("dc:title").text(t));
        }
        if let Some(l) = &self.doc.meta.language {
            meta = meta.child(Node::new("dc:language").text(l));
        }
        ns::root("office:document-meta").child(meta)
    }

    pub(super) fn report_residuals(&mut self) {
        let places: Vec<String> = self
            .sidecar
            .residuals
            .iter()
            .flat_map(|((family, _), r)| r.places().map(move |p| format!("{family}{p}")))
            .collect();
        for place in places {
            self.log.report.note(
                sidecar::PART,
                &format!("{place} has no ODF slot; kept in the sidecar"),
            );
        }
    }

    /// `content.xml` and `styles.xml`.
    pub(super) fn documents(&mut self) -> Result<(Node, Node), PackageError> {
        let doc = self.doc;
        let mut pending = self.orphan_footnotes()?;
        pending.extend(self.unanchored_comments());
        self.pending = pending;
        let mut body = Vec::new();
        for (i, section) in doc.sections.iter().enumerate() {
            body.extend(self.container(&section.blocks, Some(&section_start(i, section)))?);
        }
        let regions = std::mem::take(&mut self.regions);
        let (layouts, masters) = self.masters()?;
        self.settings_residual(&masters);
        let office_styles = self.office_styles();
        let tracked = Node::new("text:tracked-changes")
            .attr(
                "text:track-changes",
                if doc.settings.track_changes {
                    "true"
                } else {
                    "false"
                },
            )
            .children(regions.iter().map(super::region::Region::node));
        let text = Node::new("office:text").child(tracked).children(body);
        let content_autos = self.automatic(Part::Content, Vec::new());
        let styles_autos = self.automatic(Part::Styles, layouts);
        let master_styles = Node::new("office:master-styles").children(masters);
        let mut used = BTreeSet::new();
        fonts(
            &[
                &text,
                &content_autos,
                &styles_autos,
                &office_styles,
                &master_styles,
            ],
            &mut used,
        );
        let content = ns::root("office:document-content")
            .child(face_decls(&used))
            .child(content_autos)
            .child(Node::new("office:body").child(text));
        let styles = ns::root("office:document-styles")
            .child(face_decls(&used))
            .child(office_styles)
            .child(styles_autos)
            .child(master_styles);
        Ok((content, styles))
    }
}
