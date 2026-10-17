//! The document: the body split into sections where a block starts a master page, each
//! section's page layout and headers and footers from its master pages (a master whose next
//! style is another is the first page of a title-page section), the settings, and the notes and
//! comments in their model order.

use super::body::{Blocks, Start};
use super::table::bare;
use super::Reader;
use crate::ns::Part;
use crate::page::read_layout;
use cdoc_model::{
    Block, Comment, Document, DocumentMeta, DocumentSettings, Footnote, HeaderFooter,
    HeaderFooterSet, Numbering, Section, SectionProperties, SectionStart, StyleSheet,
};
use cdoc_pkg::xml::Node;
use cdoc_pkg::PackageError;

const BANDS: [&str; 4] = [
    "style:header",
    "style:header-left",
    "style:footer",
    "style:footer-left",
];

fn order_by<T, K: PartialEq>(items: &mut [T], key: impl Fn(&T) -> K, order: &[K]) {
    items.sort_by_key(|item| {
        order
            .iter()
            .position(|k| *k == key(item))
            .unwrap_or(usize::MAX)
    });
}

impl<'a> Reader<'a> {
    fn band(&mut self, master: &Node, name: &str) -> Result<Option<HeaderFooter>, PackageError> {
        let Some(node) = master.find(name) else {
            return Ok(None);
        };
        if node.get("style:display") == Some("false") && !name.ends_with("-left") {
            self.log.lose("a header or footer switched off");
            return Ok(None);
        }
        let mut blocks = self.container(node.elems())?.blocks;
        if blocks.is_empty() {
            blocks.push(bare());
        }
        Ok(Some(HeaderFooter { blocks }))
    }

    fn section(&mut self, i: usize, start: &Start) -> Result<SectionProperties, PackageError> {
        let Some(master) = self
            .index
            .master(&start.master)
            .or_else(|| self.index.masters.first().copied())
        else {
            self.log
                .lose("no master page; the page of a blank document assumed");
            return Ok(Document::blank().sections.remove(0).properties);
        };
        if master.get("style:name") != Some(start.master.as_str()) {
            self.log
                .lose("a block starting a master page that is not defined");
        }
        let next = master
            .get("style:next-style-name")
            .and_then(|n| self.index.master(n));
        let (main, first) = match next {
            Some(next) if next.get("style:name") != master.get("style:name") => {
                (next, Some(master))
            }
            _ => (master, None),
        };
        let title_page = first.is_some();
        let first = first.or_else(|| {
            self.sidecar
                .first_masters
                .get(&i)
                .and_then(|n| self.index.master(n))
        });
        for child in main.elems().filter(|c| !BANDS.contains(&c.name.as_str())) {
            self.log.lose(&format!("style:master-page/{}", child.name));
        }
        let mut headers = HeaderFooterSet::none();
        let mut footers = HeaderFooterSet::none();
        headers.default = self.band(main, BANDS[0])?;
        headers.even = self.band(main, BANDS[1])?;
        footers.default = self.band(main, BANDS[2])?;
        footers.even = self.band(main, BANDS[3])?;
        if let Some(first) = first {
            headers.first = self.band(first, BANDS[0])?;
            footers.first = self.band(first, BANDS[2])?;
        }
        let empty = Node::new("style:page-layout");
        let layout = main
            .get("style:page-layout-name")
            .and_then(|n| self.index.layouts.get(n).copied())
            .unwrap_or(&empty);
        let (header, footer) = (!headers.present().is_empty(), !footers.present().is_empty());
        let page = read_layout(layout, header, footer, start.page_number, &mut self.log);
        let props = SectionProperties {
            start: SectionStart::NextPage,
            page,
            headers,
            footers,
            title_page,
        };
        Ok(self.fixed("section", &i.to_string(), props))
    }

    fn sections(&mut self, body: Blocks) -> Result<Vec<Section>, PackageError> {
        let default_master = self
            .index
            .masters
            .first()
            .and_then(|m| m.get("style:name"))
            .unwrap_or_default();
        let mut starts = body.starts;
        if starts.first().is_none_or(|(at, _)| *at != 0) {
            let start = Start {
                master: default_master.to_owned(),
                page_number: None,
            };
            starts.insert(0, (0, start));
        }
        let mut blocks = body.blocks;
        let mut parts = Vec::new();
        for (at, start) in starts.into_iter().rev() {
            let mut own: Vec<Block> = blocks.split_off(at.min(blocks.len()));
            if own.is_empty() {
                own.push(bare());
            }
            parts.push((start, own));
        }
        let mut out = Vec::new();
        for (i, (start, blocks)) in parts.into_iter().rev().enumerate() {
            let properties = self.in_part(Part::Styles, |r| r.section(i, &start))?;
            out.push(Section { properties, blocks });
        }
        Ok(out)
    }

    fn settings(&mut self, text: &Node, tab: Option<cdoc_model::Length>) -> DocumentSettings {
        let even = self
            .index
            .masters
            .iter()
            .flat_map(|m| m.elems())
            .any(|band| {
                matches!(
                    band.name.as_str(),
                    "style:header-left" | "style:footer-left"
                ) && band.get("style:display") != Some("false")
            });
        let track = text
            .find("text:tracked-changes")
            .and_then(|t| t.get("text:track-changes"))
            == Some("true");
        let default_tab_stop = tab.unwrap_or_else(|| {
            self.log
                .lose("no default tab stop; a blank document's assumed");
            Document::blank().settings.default_tab_stop
        });
        let settings = DocumentSettings {
            even_and_odd_headers: even,
            track_changes: track,
            default_tab_stop,
        };
        self.fixed("settings", "", settings)
    }

    fn sheet_complete(&mut self) -> (StyleSheet, Option<cdoc_model::Length>) {
        let (mut sheet, tab) = self.in_part(Part::Styles, Self::sheet);
        let builtin = StyleSheet::word_builtin().defaults.run;
        if sheet.defaults.run.font.is_none() {
            self.log.report.note(
                "styles.xml",
                "no default font; the built-in one substituted",
            );
            sheet.defaults.run.font = builtin.font;
        }
        if sheet.defaults.run.size.is_none() {
            self.log.report.note(
                "styles.xml",
                "no default size; the built-in one substituted",
            );
            sheet.defaults.run.size = builtin.size;
        }
        (sheet, tab)
    }

    fn ordered_notes(&mut self) -> (Vec<Footnote>, Vec<Comment>) {
        let mut footnotes = std::mem::take(&mut self.footnotes);
        for f in &mut footnotes {
            if f.blocks.is_empty() {
                f.blocks.push(bare());
            }
        }
        order_by(&mut footnotes, |f| f.id, &self.sidecar.footnotes);
        let mut comments = std::mem::take(&mut self.comments);
        let extras = &self.sidecar.comments;
        for c in &mut comments {
            if let Some(extra) = extras.iter().find(|e| e.id == c.id) {
                c.resolved = extra.resolved;
                c.parent.clone_from(&extra.parent);
            }
        }
        let order: Vec<_> = extras.iter().map(|e| e.id.clone()).collect();
        order_by(&mut comments, |c| c.id.clone(), &order);
        (footnotes, comments)
    }

    /// The whole document but its metadata.
    pub(super) fn document(&mut self, content: &'a Node) -> Result<Document, PackageError> {
        let (styles, tab) = self.sheet_complete();
        let definitions = self.in_part(Part::Styles, Self::definitions);
        let text = content
            .find("office:body")
            .and_then(|b| b.find("office:text"))
            .ok_or_else(|| PackageError::Invalid {
                part: "content.xml".to_owned(),
                detail: "no office:text body".to_owned(),
            })?;
        let body = self.container(text.elems())?;
        let sections = self.sections(body)?;
        let settings = self.settings(text, tab);
        let instances = self
            .sidecar
            .instances
            .take()
            .unwrap_or_else(|| std::mem::take(&mut self.instances));
        let (footnotes, comments) = self.ordered_notes();
        Ok(Document {
            meta: DocumentMeta {
                title: None,
                language: None,
            },
            settings,
            styles,
            numbering: Numbering {
                definitions,
                instances,
            },
            sections,
            footnotes,
            comments,
        })
    }
}
