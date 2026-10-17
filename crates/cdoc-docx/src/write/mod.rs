//! Model to `.docx`.

mod block;
mod inline;
mod notes;
mod parts;
mod runs;
mod sdt;
mod section;
mod table;

use crate::error::DocxError;
use crate::ids::{self, Ids};
use crate::package::{rels_name, Package, Rels};
use crate::report::Log;
use crate::xml::{serialize, Node};
use cdoc_model::{ContentId, Document, ImagePort, Report, StyleId};
use cdoc_pkg::WriteOptions;

/// An image part waiting to be written.
struct MediaPart {
    id: ContentId,
    name: String,
    mime: String,
}

/// Everything the part writers share.
struct Writer<'a> {
    media: &'a dyn ImagePort,
    log: Log,
    ids: Ids,
    rels: Rels,
    out: Package,
    overrides: Vec<(String, &'static str)>,
    media_parts: Vec<MediaPart>,
    footnote_style: bool,
    headers: usize,
    footers: usize,
    drawings: u32,
}

/// Convert a document to `.docx` (or, when `options` asks for a template, `.dotx`) bytes. Image
/// bytes come from `media`. The report lists what the format could not carry. Macro-enabled
/// kinds are never written.
pub fn write_docx(
    doc: &Document,
    media: &dyn ImagePort,
    options: WriteOptions,
) -> Result<(Vec<u8>, Report), DocxError> {
    let mut w = Writer {
        media,
        log: Log::new("word/document.xml"),
        ids: Ids::empty(),
        rels: Rels::empty(),
        out: Package::empty(),
        overrides: Vec::new(),
        media_parts: Vec::new(),
        footnote_style: doc.styles.get(&StyleId::new("FootnoteReference")).is_some(),
        headers: 0,
        footers: 0,
        drawings: 0,
    };
    for comment in &doc.comments {
        w.ids.comments.number(comment.id.as_str());
    }
    w.package(doc, options)?;
    let bytes = w.out.write(crate::package::LEAD)?;
    Ok((bytes, w.log.report))
}

impl Writer<'_> {
    /// Serialise `node` as `name`, noting characters XML cannot carry.
    fn put(&mut self, name: &str, node: &Node, content_type: Option<&'static str>) {
        let (bytes, replaced) = serialize(node);
        self.log.report.note_n(
            name,
            "characters not allowed in XML replaced with U+FFFD",
            u32::try_from(replaced).unwrap_or(u32::MAX),
        );
        self.out.put(name, bytes);
        if let Some(ct) = content_type {
            self.overrides.push((format!("/{name}"), ct));
        }
    }

    /// Build a part with its own relationships and loss context, then store it.
    fn part(
        &mut self,
        name: &str,
        content_type: &'static str,
        build: impl FnOnce(&mut Self) -> Result<Node, DocxError>,
    ) -> Result<(), DocxError> {
        let saved_rels = std::mem::replace(&mut self.rels, Rels::empty());
        let saved_part = std::mem::replace(&mut self.log.part, name.to_owned());
        let node = build(self);
        let rels = std::mem::replace(&mut self.rels, saved_rels);
        self.log.part = saved_part;
        let node = node?;
        self.put(name, &node, Some(content_type));
        if !rels.is_empty() {
            self.put(&rels_name(name), &rels.to_node(), None);
        }
        Ok(())
    }

    fn package(&mut self, doc: &Document, options: WriteOptions) -> Result<(), DocxError> {
        let mut root_rels = Rels::empty();
        root_rels.add("officeDocument", "word/document.xml", false);
        self.part(
            "word/document.xml",
            crate::kind::main_content_type(options.kind),
            |w| {
                let body = w.body(doc)?;
                w.aux_parts(doc)?;
                Ok(crate::ns::root("w:document").child(body))
            },
        )?;
        if let Some(core) = crate::settings::write_core(&doc.meta) {
            self.put(
                "docProps/core.xml",
                &core,
                Some(crate::ns::content_type::CORE),
            );
            root_rels.add(crate::package::rel_type::CORE, "docProps/core.xml", false);
        }
        if let Some(sidecar) = self.ids.to_node() {
            self.put(ids::PART, &sidecar, Some(ids::CONTENT_TYPE));
            root_rels.add(crate::package::rel_type::IDS, ids::PART, false);
        }
        self.put("_rels/.rels", &root_rels.to_node(), None);
        let types = self.content_types();
        self.put("[Content_Types].xml", &types, None);
        Ok(())
    }

    fn content_types(&self) -> Node {
        let ns = "http://schemas.openxmlformats.org/package/2006/content-types";
        let default = |ext: &str, ct: &str| {
            Node::new("Default")
                .attr("Extension", ext)
                .attr("ContentType", ct)
        };
        let mut types = Node::new("Types")
            .attr("xmlns", ns)
            .child(default(
                "rels",
                "application/vnd.openxmlformats-package.relationships+xml",
            ))
            .child(default("xml", "application/xml"));
        let mut seen: Vec<&str> = Vec::new();
        for part in &self.media_parts {
            let ext = part.name.rsplit('.').next().unwrap_or("bin");
            if !seen.contains(&ext) {
                seen.push(ext);
                types = types.child(default(ext, &part.mime));
            }
        }
        for (name, ct) in &self.overrides {
            types = types.child(
                Node::new("Override")
                    .attr("PartName", name)
                    .attr("ContentType", ct),
            );
        }
        types
    }
}
