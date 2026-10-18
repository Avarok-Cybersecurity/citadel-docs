//! `.odt` / `.ott` to model.

mod body;
mod changes;
mod ctx;
mod frame;
mod index;
mod inline;
mod lists;
mod marks;
mod sections;
mod sheet;
mod table;

use crate::ns::{kind_of, Part};
use crate::sidecar::{self, Sidecar};
use cdoc_model::{
    AbstractNumId, Comment, ContentId, DocumentMeta, Footnote, FootnoteId, NumId,
    NumberingInstance, RunProperties, StyleId,
};
use cdoc_pkg::xml::{self, Node, Whitespace};
use cdoc_pkg::{require, Imported, Kind, Log, MediaSink, Package, PackageError, ReadLimits};
use changes::Region;
use index::Index;
use std::collections::{BTreeMap, BTreeSet};

/// State shared by every part reader.
struct Reader<'a> {
    pkg: &'a Package,
    sink: &'a mut dyn MediaSink,
    log: Log,
    sidecar: Sidecar,
    index: Index<'a>,
    part: Part,
    consumed: BTreeSet<String>,
    media_types: BTreeMap<String, String>,
    images: BTreeMap<String, ContentId>,
    /// Annotation names that have an `office:annotation-end`: ranges, not points.
    ends: BTreeSet<String>,
    regions: BTreeMap<String, Region<'a>>,
    /// Format-change regions, reported once and skipped where marked.
    formats: BTreeSet<String>,
    comments: Vec<Comment>,
    footnotes: Vec<Footnote>,
    instances: Vec<NumberingInstance>,
    list_nums: BTreeMap<String, NumId>,
    last_list: BTreeMap<String, NumId>,
    style_ids: BTreeMap<(String, String), StyleId>,
    abstracts: Vec<(Option<Part>, String, AbstractNumId)>,
    level_counts: BTreeMap<AbstractNumId, usize>,
    para_cache: BTreeMap<(Part, String), sheet::ParaStyle>,
    span_cache: BTreeMap<(Part, String), RunProperties>,
    note_names: BTreeMap<String, FootnoteId>,
    unnamed: usize,
}

fn parse(pkg: &Package, name: &str) -> Result<Option<Node>, PackageError> {
    pkg.get(name)
        .map(|b| xml::parse(name, b, Whitespace::Keep))
        .transpose()
}

fn manifest(pkg: &Package) -> Result<(BTreeMap<String, String>, Option<Kind>), PackageError> {
    let Some(root) = parse(pkg, "META-INF/manifest.xml")? else {
        return Ok((BTreeMap::new(), None));
    };
    let mut types = BTreeMap::new();
    let mut kind = None;
    for entry in root.elems().filter(|e| e.name == "manifest:file-entry") {
        if entry.find("manifest:encryption-data").is_some() {
            return Err(PackageError::Invalid {
                part: "META-INF/manifest.xml".to_owned(),
                detail: "the package is encrypted".to_owned(),
            });
        }
        let (Some(path), Some(media)) = (
            entry.get("manifest:full-path"),
            entry.get("manifest:media-type"),
        ) else {
            continue;
        };
        if path == "/" {
            kind = kind_of(media);
        }
        types.insert(path.to_owned(), media.to_owned());
    }
    Ok((types, kind))
}

fn kind(pkg: &Package, from_manifest: Option<Kind>) -> Result<Kind, PackageError> {
    let declared = pkg
        .get("mimetype")
        .map(|b| String::from_utf8_lossy(b).into_owned());
    let kind = match &declared {
        Some(mt) => kind_of(mt),
        None => from_manifest,
    };
    kind.ok_or_else(|| PackageError::Invalid {
        part: "mimetype".to_owned(),
        detail: format!(
            "not an OpenDocument text document or template: {}",
            declared.as_deref().unwrap_or("no media type")
        ),
    })
}

/// Read a `.odt` or `.ott`. Embedded images go to `sink`, which names them. The report lists
/// what the model could not hold; the kind says whether the package was a template. The
/// returned document is normalised but not validated.
pub fn read_odt(
    bytes: &[u8],
    limits: &ReadLimits,
    sink: &mut dyn MediaSink,
) -> Result<Imported, PackageError> {
    let pkg = Package::read(bytes, limits)?;
    let (media_types, manifest_kind) = manifest(&pkg)?;
    let kind = kind(&pkg, manifest_kind)?;
    let content = xml::parse(
        "content.xml",
        require(&pkg, "content.xml")?,
        Whitespace::Keep,
    )?;
    let styles = parse(&pkg, "styles.xml")?;
    let sidecar = match pkg.get(sidecar::PART) {
        Some(bytes) => Sidecar::parse(bytes)?,
        None => Sidecar::empty(),
    };
    let mut r = Reader {
        pkg: &pkg,
        sink,
        log: Log::new("content.xml"),
        sidecar,
        index: Index::build(&content, styles.as_ref()),
        part: Part::Content,
        consumed: [
            "mimetype",
            "META-INF/manifest.xml",
            "content.xml",
            "styles.xml",
            sidecar::PART,
        ]
        .map(str::to_owned)
        .into(),
        media_types,
        images: BTreeMap::new(),
        ends: BTreeSet::new(),
        regions: BTreeMap::new(),
        formats: BTreeSet::new(),
        comments: Vec::new(),
        footnotes: Vec::new(),
        instances: Vec::new(),
        list_nums: BTreeMap::new(),
        last_list: BTreeMap::new(),
        style_ids: BTreeMap::new(),
        abstracts: Vec::new(),
        level_counts: BTreeMap::new(),
        para_cache: BTreeMap::new(),
        span_cache: BTreeMap::new(),
        note_names: BTreeMap::new(),
        unnamed: 0,
    };
    r.prescan(&content);
    if let Some(s) = &styles {
        r.prescan(s);
    }
    let mut doc = r.document(&content)?;
    doc.meta = r.meta()?;
    r.report_unread();
    r.log.report.absorb(cdoc_pkg::repair_references(&mut doc));
    doc.normalize();
    Ok(Imported {
        document: doc,
        report: r.log.report,
        kind,
    })
}

impl<'a> Reader<'a> {
    /// Find annotation ends and change regions anywhere in a part.
    fn prescan(&mut self, root: &'a Node) {
        let mut stack = vec![root];
        while let Some(node) = stack.pop() {
            match node.name.as_str() {
                "office:annotation-end" => {
                    if let Some(name) = node.get("office:name") {
                        self.ends.insert(name.to_owned());
                    }
                }
                "text:changed-region" => self.region(node),
                _ => {}
            }
            stack.extend(node.elems());
        }
    }

    /// Run `f` reading `part`'s content (its automatic styles, its loss context).
    fn in_part<T>(&mut self, part: Part, f: impl FnOnce(&mut Self) -> T) -> T {
        let saved = std::mem::replace(&mut self.part, part);
        let saved_name = std::mem::replace(&mut self.log.part, part.file().to_owned());
        let out = f(self);
        self.part = saved;
        self.log.part = saved_name;
        out
    }

    fn meta(&mut self) -> Result<DocumentMeta, PackageError> {
        let mut meta = DocumentMeta {
            title: None,
            language: None,
        };
        if self.pkg.get("settings.xml").is_some() {
            self.consumed.insert("settings.xml".to_owned());
            self.log
                .report
                .note("settings.xml", "application settings not read");
        }
        if let Some(root) = parse(self.pkg, "meta.xml")? {
            self.consumed.insert("meta.xml".to_owned());
            for item in root.find("office:meta").into_iter().flat_map(Node::elems) {
                match item.name.as_str() {
                    "dc:title" => meta.title = Some(item.content()),
                    "dc:language" => meta.language = Some(item.content()),
                    "meta:generator" => {}
                    other => self
                        .log
                        .report
                        .note("meta.xml", &format!("office:meta/{other}")),
                }
            }
        }
        Ok(meta)
    }

    fn report_unread(&mut self) {
        let unread: Vec<String> = self
            .pkg
            .names()
            .filter(|n| !self.consumed.contains(*n))
            .map(str::to_owned)
            .collect();
        for name in unread {
            self.log.report.note(&name, "part not read");
        }
    }
}
