//! Sections: the body, section properties, and the header and footer parts they point at.

use super::Writer;
use crate::error::DocxError;
use crate::ns::{content_type as ct, declare};
use crate::package::rel_type;
use crate::props::{
    header_kind_name, number_format_name, orientation_name, section_start_name, twips,
};
use crate::xml::{Node, WordNode};
use cdoc_model::{Document, HeaderFooter, HeaderFooterSet, SectionProperties};

impl Writer<'_> {
    pub(super) fn body(&mut self, doc: &Document) -> Result<Node, DocxError> {
        let mut body = Node::new("w:body");
        let last = doc.sections.len().saturating_sub(1);
        for (i, section) in doc.sections.iter().enumerate() {
            let sect = self.sect_pr(&section.properties)?;
            if i == last {
                body = body
                    .children(self.blocks(&section.blocks, None)?)
                    .child(sect);
            } else {
                body = body.children(self.blocks(&section.blocks, Some(sect))?);
            }
        }
        Ok(body)
    }

    fn sect_pr(&mut self, props: &SectionProperties) -> Result<Node, DocxError> {
        let mut node = Node::new("w:sectPr");
        for (set, element, header) in [
            (&props.headers, "w:headerReference", true),
            (&props.footers, "w:footerReference", false),
        ] {
            node = self.references(node, set, element, header)?;
        }
        let page = &props.page;
        let m = &page.margins;
        let log = &mut self.log;
        node = node
            .child(Node::val("w:type", section_start_name(props.start)))
            .child(
                Node::new("w:pgSz")
                    .num("w:w", twips(page.size.width, log))
                    .num("w:h", twips(page.size.height, log))
                    .attr("w:orient", orientation_name(page.size.orientation)),
            )
            .child(
                Node::new("w:pgMar")
                    .num("w:top", twips(m.top, log))
                    .num("w:right", twips(m.right, log))
                    .num("w:bottom", twips(m.bottom, log))
                    .num("w:left", twips(m.left, log))
                    .num("w:header", twips(m.header, log))
                    .num("w:footer", twips(m.footer, log))
                    .num("w:gutter", twips(m.gutter, log)),
            );
        let mut numbering =
            Node::new("w:pgNumType").attr("w:fmt", number_format_name(page.page_numbering.format));
        if let Some(start) = page.page_numbering.start {
            numbering = numbering.num("w:start", start);
        }
        let mut cols = Node::new("w:cols")
            .num("w:num", page.columns.count)
            .num("w:space", twips(page.columns.gap, log));
        if page.columns.separator {
            cols = cols.attr("w:sep", "1");
        }
        node = node.child(numbering).child(cols);
        if props.title_page {
            node = node.child(Node::new("w:titlePg"));
        }
        Ok(node)
    }

    fn references(
        &mut self,
        mut node: Node,
        set: &HeaderFooterSet,
        element: &str,
        header: bool,
    ) -> Result<Node, DocxError> {
        for (kind, content) in set.present() {
            let rid = self.header_footer(content, header)?;
            node = node.child(
                Node::new(element)
                    .attr("w:type", header_kind_name(kind))
                    .attr("r:id", &rid),
            );
        }
        Ok(node)
    }

    fn header_footer(&mut self, content: &HeaderFooter, header: bool) -> Result<String, DocxError> {
        let (count, stem, content_type, root, kind) = if header {
            self.headers += 1;
            (
                self.headers,
                "header",
                ct::HEADER,
                "w:hdr",
                rel_type::HEADER,
            )
        } else {
            self.footers += 1;
            (
                self.footers,
                "footer",
                ct::FOOTER,
                "w:ftr",
                rel_type::FOOTER,
            )
        };
        let file = format!("{stem}{count}.xml");
        self.part(&format!("word/{file}"), content_type, |w| {
            Ok(declare(
                Node::new(root).children(w.blocks(&content.blocks, None)?),
            ))
        })?;
        Ok(self.rels.add(kind, &file, false))
    }
}
