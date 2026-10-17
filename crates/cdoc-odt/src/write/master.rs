//! Sections as master pages. Each section has a page layout (`pmN`) and a master page
//! (`SectionN`) holding its default and even headers and footers; a title page adds a first
//! master page (`SectionNFirst`) whose next style is the section's, which the section's first
//! block starts. The even variants are `style:header-left`, shown only when the document uses
//! different even pages. A first-page header without a title page has no slot: its master
//! page is written unreferenced and named in the sidecar.

use super::para::SectionStart;
use super::Writer;
use crate::ns::Part;
use crate::page::{read_layout, write_layout};
use crate::residual::Residual;
use cdoc_model::{HeaderFooter, Section, SectionProperties, SectionStart as Start};
use cdoc_pkg::xml::{Child, Node};
use cdoc_pkg::{Log, PackageError};

fn main_name(i: usize) -> String {
    format!("Section{}", i + 1)
}

fn first_name(i: usize) -> String {
    format!("Section{}First", i + 1)
}

/// What section `i`'s first block carries.
#[must_use]
pub fn section_start(i: usize, section: &Section) -> SectionStart {
    let p = &section.properties;
    SectionStart {
        master: if p.title_page {
            first_name(i)
        } else {
            main_name(i)
        },
        page_number: p.page.page_numbering.start,
    }
}

impl Writer<'_> {
    fn band(
        &mut self,
        name: &str,
        hf: Option<&HeaderFooter>,
        display: Option<bool>,
    ) -> Result<Option<Node>, PackageError> {
        let Some(hf) = hf else { return Ok(None) };
        let saved = std::mem::take(&mut self.regions);
        let blocks = self.container(&hf.blocks, None);
        let regions = std::mem::replace(&mut self.regions, saved);
        let mut node = Node::new(name);
        if let Some(display) = display {
            node = node.attr("style:display", if display { "true" } else { "false" });
        }
        if !regions.is_empty() {
            node = node.child(
                Node::new("text:tracked-changes")
                    .children(regions.iter().map(super::region::Region::node)),
            );
        }
        Ok(Some(node.children(blocks?)))
    }

    fn master(
        &mut self,
        name: &str,
        layout: &str,
        next: Option<&str>,
        variants: [Option<&HeaderFooter>; 4],
    ) -> Result<Node, PackageError> {
        let even = self.doc.settings.even_and_odd_headers;
        let mut node = Node::new("style:master-page")
            .attr("style:name", name)
            .attr("style:page-layout-name", layout);
        if let Some(next) = next {
            node = node.attr("style:next-style-name", next);
        }
        let [header, header_left, footer, footer_left] = variants;
        let bands = [
            self.band("style:header", header, None)?,
            self.band("style:header-left", header_left, Some(even))?,
            self.band("style:footer", footer, None)?,
            self.band("style:footer-left", footer_left, Some(even))?,
        ];
        node.children
            .extend(bands.into_iter().flatten().map(Child::Node));
        Ok(node)
    }

    fn section_residual(
        &mut self,
        i: usize,
        props: &SectionProperties,
        layout: &Node,
        header: bool,
        footer: bool,
    ) {
        let mut scratch = Log::new("(readback)");
        let back = SectionProperties {
            start: Start::NextPage,
            page: read_layout(
                layout,
                header,
                footer,
                props.page.page_numbering.start,
                &mut scratch,
            ),
            headers: props.headers.clone(),
            footers: props.footers.clone(),
            title_page: props.title_page,
        };
        let residual = Residual::between(props, &back, &["headers", "footers"]);
        if !residual.is_empty() {
            self.sidecar
                .residuals
                .insert(("section".to_owned(), i.to_string()), residual);
        }
    }

    /// The page layouts and master pages, in `styles.xml`.
    pub(super) fn masters(&mut self) -> Result<(Vec<Node>, Vec<Node>), PackageError> {
        self.in_part(Part::Styles, |w| {
            let mut layouts = Vec::new();
            let mut masters = Vec::new();
            for (i, section) in w.doc.sections.iter().enumerate() {
                let p = &section.properties;
                let header = !p.headers.present().is_empty();
                let footer = !p.footers.present().is_empty();
                let layout_name = format!("pm{}", i + 1);
                let layout = write_layout(&layout_name, &p.page, header, footer);
                w.section_residual(i, p, &layout, header, footer);
                layouts.push(layout);
                let (h, f) = (&p.headers, &p.footers);
                let main = [h.default.as_ref(), h.even.as_ref(), f.default.as_ref(), f.even.as_ref()];
                masters.push(w.master(&main_name(i), &layout_name, None, main)?);
                if p.title_page || h.first.is_some() || f.first.is_some() {
                    let first = [h.first.as_ref(), None, f.first.as_ref(), None];
                    masters.push(w.master(&first_name(i), &layout_name, Some(&main_name(i)), first)?);
                    if !p.title_page {
                        w.log.lose("a first-page header or footer without a title page kept in an unused master page");
                        w.sidecar.first_masters.insert(i, first_name(i));
                    }
                }
            }
            Ok((layouts, masters))
        })
    }
}
