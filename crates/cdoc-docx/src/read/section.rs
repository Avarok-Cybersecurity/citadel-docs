//! The body: paragraphs and tables cut into sections at each `w:sectPr`, section properties,
//! and the headers and footers they reference.

use super::block::bare_paragraph;
use super::Reader;
use crate::error::DocxError;
use crate::package::resolve;
use crate::props::{
    header_kind_from, length, num, number_format_from, orientation_from, section_start_from,
};
use crate::seal::unseal;
use crate::xml::{El, WordEl};
use cdoc_model::{
    Block, Columns, HeaderFooter, HeaderFooterSet, Length, Margins, NumberFormat, Orientation,
    PageNumbering, PageSetup, PageSize, Section, SectionProperties, SectionStart,
};

/// Fill each section's missing headers and footers from the section before it, as Word shows
/// them: a section with no reference of its own continues the previous one's.
pub fn inherit_headers_and_footers(sections: &mut [Section]) {
    for i in 1..sections.len() {
        let (before, rest) = sections.split_at_mut(i);
        let previous = &before[i - 1].properties;
        let props = &mut rest[0].properties;
        for (set, from) in [
            (&mut props.headers, &previous.headers),
            (&mut props.footers, &previous.footers),
        ] {
            for (slot, inherited) in [
                (&mut set.default, &from.default),
                (&mut set.first, &from.first),
                (&mut set.even, &from.even),
            ] {
                if slot.is_none() {
                    slot.clone_from(inherited);
                }
            }
        }
    }
}

impl Reader<'_> {
    pub(super) fn sections(&mut self, body: El<'_>) -> Result<Vec<Section>, DocxError> {
        let mut nodes = Vec::new();
        self.block_nodes(body, &mut nodes);
        let mut sections = Vec::new();
        let mut blocks: Vec<Block> = Vec::new();
        let mut last_props: Option<SectionProperties> = None;
        for node in nodes {
            match node.name() {
                "w:p" => {
                    let (paragraph, sect) = self.paragraph(node)?;
                    blocks.push(Block::Paragraph(paragraph));
                    if let Some(sect) = sect {
                        let properties = self.section_properties(Some(sect))?;
                        unseal(&mut blocks);
                        sections.push(Section {
                            properties,
                            blocks: std::mem::take(&mut blocks),
                        });
                    }
                }
                "w:tbl" => blocks.push(Block::Table(self.table(node)?)),
                _ => last_props = Some(self.section_properties(Some(node))?),
            }
        }
        unseal(&mut blocks);
        if blocks.is_empty() {
            self.log
                .lose("final section had no content; an empty paragraph added");
            blocks.push(bare_paragraph());
        }
        let properties = if let Some(p) = last_props {
            p
        } else {
            self.log
                .lose("no final w:sectPr; default page setup assumed");
            self.section_properties(None)?
        };
        sections.push(Section { properties, blocks });
        inherit_headers_and_footers(&mut sections);
        Ok(sections)
    }

    fn section_properties(&mut self, sect: Option<El<'_>>) -> Result<SectionProperties, DocxError> {
        let mut props = SectionProperties {
            start: SectionStart::NextPage,
            page: PageSetup::a4_normal(),
            headers: HeaderFooterSet::none(),
            footers: HeaderFooterSet::none(),
            title_page: false,
        };
        let mut seen = [false; 3];
        for child in sect.into_iter().flat_map(El::elems) {
            match child.name() {
                "w:headerReference" | "w:footerReference" => self.reference(child, &mut props)?,
                "w:type" => {
                    props.start = child
                        .value()
                        .and_then(section_start_from)
                        .unwrap_or(SectionStart::NextPage);
                }
                "w:pgSz" => {
                    props.page.size = self.page_size(child);
                    seen[0] = true;
                }
                "w:pgMar" => {
                    props.page.margins = self.margins(child);
                    seen[1] = true;
                }
                "w:pgNumType" => props.page.page_numbering = self.page_numbering(child),
                "w:cols" => props.page.columns = self.columns(child),
                "w:titlePg" => props.title_page = child.is_on(),
                other => self.log.unknown(&format!("w:sectPr/{other}")),
            }
        }
        if !seen[0] || !seen[1] {
            self.log
                .lose("a section without page size or margins; A4 with 1 inch margins assumed");
        }
        Ok(props)
    }

    fn reference(&mut self, node: El<'_>, props: &mut SectionProperties) -> Result<(), DocxError> {
        let header = node.name() == "w:headerReference";
        let kind = node.get("w:type").and_then(header_kind_from);
        let rel = node.get("r:id").and_then(|id| self.rels.by_id(id));
        let (Some(kind), Some(rel)) = (kind, rel) else {
            self.log
                .lose("a header or footer reference that cannot be resolved");
            return Ok(());
        };
        let part = resolve(&self.log.part, &rel.target);
        let tree = self.load(&part)?;
        let blocks = self.in_part(&part, |r| r.container_blocks(tree.root()))?;
        let set = if header {
            &mut props.headers
        } else {
            &mut props.footers
        };
        *set.slot(kind) = Some(HeaderFooter { blocks });
        Ok(())
    }

    fn page_size(&mut self, node: El<'_>) -> PageSize {
        let width = length(node, "w:w", &mut self.log);
        let height = length(node, "w:h", &mut self.log);
        let (width, height) = (
            width.unwrap_or(PageSize::A4_PORTRAIT.width),
            height.unwrap_or(PageSize::A4_PORTRAIT.height),
        );
        let orientation =
            node.get("w:orient")
                .and_then(orientation_from)
                .unwrap_or(if width > height {
                    Orientation::Landscape
                } else {
                    Orientation::Portrait
                });
        PageSize {
            width,
            height,
            orientation,
        }
    }

    fn margins(&mut self, node: El<'_>) -> Margins {
        let log = &mut self.log;
        let mut get = |key: &str, fallback: Length| length(node, key, log).unwrap_or(fallback);
        let n = Margins::NORMAL;
        Margins {
            top: get("w:top", n.top),
            bottom: get("w:bottom", n.bottom),
            left: get("w:left", n.left),
            right: get("w:right", n.right),
            header: get("w:header", n.header),
            footer: get("w:footer", n.footer),
            gutter: get("w:gutter", n.gutter),
        }
    }

    fn page_numbering(&mut self, node: El<'_>) -> PageNumbering {
        PageNumbering {
            format: node
                .get("w:fmt")
                .and_then(number_format_from)
                .unwrap_or(NumberFormat::Decimal),
            start: num(node, "w:start", &mut self.log),
        }
    }

    fn columns(&mut self, node: El<'_>) -> Columns {
        if node.find("w:col").is_some() {
            self.log
                .lose("columns of unequal width (w:col); equal columns assumed");
        }
        Columns {
            count: num(node, "w:num", &mut self.log).unwrap_or(Columns::ONE.count),
            gap: length(node, "w:space", &mut self.log).unwrap_or(Columns::ONE.gap),
            separator: matches!(node.get("w:sep"), Some("1" | "true" | "on")),
        }
    }
}
