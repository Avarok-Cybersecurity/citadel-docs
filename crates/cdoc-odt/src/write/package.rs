//! The package: `mimetype` (stored, first), the manifest, `content.xml`, `styles.xml`,
//! `meta.xml`, the pictures and the sidecar, with the model's keyed lists (comments,
//! footnotes, list instances) recorded once per id.

use super::Writer;
use crate::ns::{self, media_type, MANIFEST};
use crate::sidecar::{self, CommentExtra};
use cdoc_model::NumberingInstance;
use cdoc_pkg::xml::Node;
use cdoc_pkg::{Compression, Lead, Package, PackageError, WriteOptions};
use std::collections::BTreeSet;

impl Writer<'_> {
    /// Record comments, footnotes and list instances in model order, the first of each id.
    fn keyed(&mut self) {
        let doc = self.doc;
        let mut ids = BTreeSet::new();
        for c in &doc.comments {
            if ids.insert(&c.id) {
                let extra = CommentExtra {
                    id: c.id.clone(),
                    resolved: c.resolved,
                    parent: c.parent.clone(),
                };
                self.sidecar.comments.push(extra);
            } else {
                self.log
                    .lose("a comment with the id of an earlier one dropped");
            }
        }
        for f in &doc.footnotes {
            if self.sidecar.footnotes.contains(&f.id) {
                self.log
                    .lose("a footnote with the id of an earlier one dropped");
            } else {
                self.sidecar.footnotes.push(f.id);
            }
        }
        if !doc.numbering.instances.is_empty() {
            let mut instances: Vec<NumberingInstance> = Vec::new();
            for i in &doc.numbering.instances {
                if instances.iter().any(|kept| kept.id == i.id) {
                    self.log
                        .lose("a list instance with the id of an earlier one dropped");
                } else {
                    instances.push(i.clone());
                }
            }
            self.sidecar.instances = Some(instances);
        }
    }

    /// Write every part and zip them.
    pub(super) fn package(&mut self, options: WriteOptions) -> Result<Vec<u8>, PackageError> {
        let (content, styles) = self.documents()?;
        self.keyed();
        self.report_residuals();
        let mut out = Package::empty();
        let kind = media_type(options.kind);
        out.put("mimetype", kind.as_bytes().to_vec());
        self.put(&mut out, "content.xml", &content);
        self.put(&mut out, "styles.xml", &styles);
        let meta = self.meta();
        self.put(&mut out, "meta.xml", &meta);
        let side = self.sidecar.to_node();
        self.put(&mut out, sidecar::PART, &side);
        let mut entries = vec![("/".to_owned(), kind.to_owned())];
        for name in ["content.xml", "styles.xml", "meta.xml"] {
            entries.push((name.to_owned(), "text/xml".to_owned()));
        }
        entries.push((sidecar::PART.to_owned(), sidecar::MEDIA_TYPE.to_owned()));
        for picture in std::mem::take(&mut self.pictures) {
            let bytes = self.media.image(&picture.id)?.bytes.to_vec();
            out.put(&picture.path, bytes);
            entries.push((picture.path, picture.mime));
        }
        let manifest = Node::new("manifest:manifest")
            .attr("xmlns:manifest", MANIFEST)
            .attr("manifest:version", ns::VERSION)
            .children(entries.iter().map(|(path, mt)| {
                let entry = Node::new("manifest:file-entry").attr("manifest:full-path", path);
                let entry = if path == "/" {
                    entry.attr("manifest:version", ns::VERSION)
                } else {
                    entry
                };
                entry.attr("manifest:media-type", mt)
            }));
        self.put(&mut out, "META-INF/manifest.xml", &manifest);
        out.write(Lead {
            name: "mimetype",
            compression: Compression::Stored,
        })
    }
}
