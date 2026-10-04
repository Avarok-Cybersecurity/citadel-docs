//! `.docx` to model.

mod block;
mod drawing;
mod field;
mod notes;
mod paragraph;
mod run;
mod sdt;
mod section;
mod table;

pub use section::inherit_headers_and_footers;

use crate::error::DocxError;
use crate::ids::{self, Ids};
use crate::package::{
    rel_type, rels_name, require, resolve, ContentTypes, Package, ReadLimits, Rels,
};
use crate::ports::MediaSink;
use crate::report::Log;
use crate::{numbering, settings, styles, xml};
use cdoc_model::{ContentId, Document, DocumentMeta, Numbering, Report, StyleSheet};
use std::collections::{BTreeMap, BTreeSet};

/// State shared by every part reader.
struct Reader<'a> {
    pkg: &'a Package,
    sink: &'a mut dyn MediaSink,
    types: ContentTypes,
    log: Log,
    ids: Ids,
    rels: Rels,
    consumed: BTreeSet<String>,
    images: BTreeMap<String, ContentId>,
}

/// Read a `.docx`. Embedded images go to `sink`, which names them. The report lists what the
/// model could not hold. The returned document is normalised (adjacent equal runs merged) and
/// has inherited headers and footers filled in, but is not validated.
pub fn read_docx(
    bytes: &[u8],
    limits: &ReadLimits,
    sink: &mut dyn MediaSink,
) -> Result<(Document, Report), DocxError> {
    let pkg = Package::read(bytes, limits)?;
    let types = ContentTypes::parse(require(&pkg, "[Content_Types].xml")?)?;
    let root_rels = Rels::parse("_rels/.rels", require(&pkg, "_rels/.rels")?)?;
    let ids = match pkg.get(ids::PART) {
        Some(bytes) => Ids::parse(bytes)?,
        None => Ids::empty(),
    };
    let main = root_rels
        .of_type(rel_type::DOCUMENT)
        .map(|r| resolve("", &r.target))
        .ok_or_else(|| DocxError::MissingPart("the main document relationship".to_owned()))?;
    let mut r = Reader {
        pkg: &pkg,
        sink,
        types,
        log: Log::new(&main),
        ids,
        rels: Rels::empty(),
        consumed: BTreeSet::new(),
        images: BTreeMap::new(),
    };
    r.consumed
        .extend(["[Content_Types].xml", "_rels/.rels", ids::PART].map(str::to_owned));
    let mut doc = r.document(&main)?;
    if let Some(core) = root_rels.of_type(rel_type::CORE) {
        let name = resolve("", &core.target);
        r.log.part.clone_from(&name);
        let meta = settings::read_core(&r.load(&name)?, &mut r.log);
        doc.meta = DocumentMeta {
            language: meta.language.or(doc.meta.language.take()),
            ..meta
        };
    }
    r.report_unread();
    r.log
        .report
        .absorb(crate::repair::repair_references(&mut doc));
    doc.normalize();
    Ok((doc, r.log.report))
}

impl Reader<'_> {
    /// Parse a part, marking it read.
    fn load(&mut self, name: &str) -> Result<xml::Node, DocxError> {
        self.consumed.insert(name.to_owned());
        xml::parse(name, require(self.pkg, name)?)
    }

    /// Run `f` with `name` as the current part and its relationships loaded.
    fn in_part<T>(
        &mut self,
        name: &str,
        f: impl FnOnce(&mut Self) -> Result<T, DocxError>,
    ) -> Result<T, DocxError> {
        let rels_part = rels_name(name);
        let rels = match self.pkg.get(&rels_part) {
            Some(bytes) => {
                self.consumed.insert(rels_part.clone());
                Rels::parse(&rels_part, bytes)?
            }
            None => Rels::empty(),
        };
        let saved_rels = std::mem::replace(&mut self.rels, rels);
        let saved_part = std::mem::replace(&mut self.log.part, name.to_owned());
        let result = f(self);
        self.rels = saved_rels;
        self.log.part = saved_part;
        result
    }

    /// The part a relationship of the current part of this type names.
    fn target_of(&self, kind: &str) -> Option<String> {
        let rel = self.rels.of_type(kind)?;
        Some(resolve(&self.log.part, &rel.target))
    }

    fn document(&mut self, main: &str) -> Result<Document, DocxError> {
        let root = self.load(main)?;
        self.in_part(main, |r| {
            let body = root.find("w:body").ok_or_else(|| DocxError::Invalid {
                part: main.to_owned(),
                detail: "no w:body".to_owned(),
            })?;
            let (styles, language) = r.styles()?;
            let numbering = r
                .read_aux(rel_type::NUMBERING, numbering::read_numbering)?
                .unwrap_or_else(Numbering::empty);
            let settings = match r.read_aux(rel_type::SETTINGS, settings::read_settings)? {
                Some(settings) => settings,
                None => settings::read_settings(&xml::Node::new("w:settings"), &mut r.log),
            };
            let comments = r.comments()?;
            let footnotes = r.footnotes()?;
            let sections = r.sections(body)?;
            Ok(Document {
                meta: DocumentMeta {
                    title: None,
                    language,
                },
                settings,
                styles,
                numbering,
                sections,
                footnotes,
                comments,
            })
        })
    }

    /// Read the part the current part's relationship of `kind` names, if there is one.
    fn read_aux<T>(
        &mut self,
        kind: &str,
        read: impl FnOnce(&xml::Node, &mut Log) -> T,
    ) -> Result<Option<T>, DocxError> {
        let Some(name) = self.target_of(kind) else {
            return Ok(None);
        };
        let root = self.load(&name)?;
        self.in_part(&name, |r| Ok(Some(read(&root, &mut r.log))))
    }

    fn styles(&mut self) -> Result<(StyleSheet, Option<String>), DocxError> {
        let Some((mut sheet, language)) = self.read_aux(rel_type::STYLES, styles::read_styles)?
        else {
            self.log
                .lose("styles.xml absent; built-in styles substituted");
            return Ok((StyleSheet::word_builtin(), None));
        };
        let name = self.target_of(rel_type::STYLES).unwrap_or_default();
        let builtin = StyleSheet::word_builtin().defaults.run;
        if sheet.defaults.run.font.is_none() {
            self.log.report.note(
                &name,
                "default font unresolved (theme font); built-in substituted",
            );
            sheet.defaults.run.font = builtin.font;
        }
        if sheet.defaults.run.size.is_none() {
            self.log
                .report
                .note(&name, "default size absent; built-in substituted");
            sheet.defaults.run.size = builtin.size;
        }
        Ok((sheet, language))
    }

    fn report_unread(&mut self) {
        let unread: Vec<String> = self
            .pkg
            .names()
            .filter(|n| !is_rels(n) && !self.consumed.contains(*n))
            .map(str::to_owned)
            .collect();
        for name in unread {
            self.log.report.note(&name, "part not read");
        }
    }
}

fn is_rels(name: &str) -> bool {
    std::path::Path::new(name)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("rels"))
}
