//! Model to `.odt` / `.ott`.

mod body;
mod inline;
mod line;
mod master;
mod notes;
mod orphans;
mod package;
mod para;
mod parts;
mod region;
mod registry;
mod table;

use crate::ns::Part;
use crate::sidecar::Sidecar;
use crate::styles::StyleNames;
use cdoc_model::{CommentId, ContentId, Document, FootnoteId, ImagePort, NumId, Report};
use cdoc_pkg::xml::Node;
use cdoc_pkg::{Log, PackageError, WriteOptions};
use registry::Registry;
use std::collections::{BTreeMap, BTreeSet};

/// An image part waiting to be written.
struct Picture {
    id: ContentId,
    path: String,
    mime: String,
}

/// Everything the part writers share.
struct Writer<'a> {
    doc: &'a Document,
    media: &'a dyn ImagePort,
    log: Log,
    names: StyleNames,
    part: Part,
    autos: [Registry; 2],
    dates: [Vec<(String, String)>; 2],
    sidecar: Sidecar,
    pictures: Vec<Picture>,
    /// Change regions of the story group being written (the body, or one header).
    regions: Vec<region::Region>,
    region_count: usize,
    footnotes_written: BTreeSet<FootnoteId>,
    citations: usize,
    comments_written: BTreeSet<CommentId>,
    segments: usize,
    /// Point annotations and orphan footnotes for the first body paragraph.
    pending: Vec<Node>,
    lists: usize,
    first_list: BTreeMap<NumId, String>,
    tables: usize,
    frames: usize,
}

/// Convert a document to `.odt` (or, when `options` asks for a template, `.ott`) bytes. Image
/// bytes come from `media`. The report lists what other ODF readers will not see: values the
/// format has no slot for (kept in the `cdoc/sidecar.xml` part, so this crate reads them back)
/// and constructs written as their nearest ODF equivalent.
pub fn write_odt(
    doc: &Document,
    media: &dyn ImagePort,
    options: WriteOptions,
) -> Result<(Vec<u8>, Report), PackageError> {
    let names = StyleNames::for_sheet(&doc.styles);
    let taken: BTreeSet<(String, String)> = doc
        .styles
        .styles
        .iter()
        .map(|s| (crate::styles::family(s.kind).to_owned(), names.of(&s.id)))
        .collect();
    let mut w = Writer {
        doc,
        media,
        log: Log::new("content.xml"),
        names,
        part: Part::Content,
        autos: [Registry::new("", taken.clone()), Registry::new("M", taken)],
        dates: [Vec::new(), Vec::new()],
        sidecar: Sidecar::empty(),
        pictures: Vec::new(),
        regions: Vec::new(),
        region_count: 0,
        footnotes_written: BTreeSet::new(),
        citations: 0,
        comments_written: BTreeSet::new(),
        segments: 0,
        pending: Vec::new(),
        lists: 0,
        first_list: BTreeMap::new(),
        tables: 0,
        frames: 0,
    };
    let bytes = w.package(options)?;
    Ok((bytes, w.log.report))
}

impl Writer<'_> {
    const fn index(&self) -> usize {
        match self.part {
            Part::Content => 0,
            Part::Styles => 1,
        }
    }

    /// The automatic styles of the part being written.
    fn autos(&mut self) -> &mut Registry {
        let i = self.index();
        &mut self.autos[i]
    }

    /// Run `f` writing into `part`, with the loss context set to it.
    fn in_part<T>(&mut self, part: Part, f: impl FnOnce(&mut Self) -> T) -> T {
        let saved = std::mem::replace(&mut self.part, part);
        let saved_name = std::mem::replace(&mut self.log.part, part.file().to_owned());
        let out = f(self);
        self.part = saved;
        self.log.part = saved_name;
        out
    }

    /// The data style for a date picture in the current part.
    fn date_style(&mut self, picture: &str) -> String {
        let i = self.index();
        if let Some((name, _)) = self.dates[i].iter().find(|(_, p)| p == picture) {
            return name.clone();
        }
        let prefix = if i == 0 { "N" } else { "MN" };
        let name = format!("{prefix}{}", self.dates[i].len() + 1);
        self.dates[i].push((name.clone(), picture.to_owned()));
        name
    }

    /// The package path of an image, fetching its bytes once.
    fn picture(&mut self, id: &ContentId) -> Result<String, PackageError> {
        if let Some(p) = self.pictures.iter().find(|p| &p.id == id) {
            return Ok(p.path.clone());
        }
        let image = self.media.image(id)?;
        let ext = match image.mime {
            "image/png" => "png",
            "image/jpeg" => "jpg",
            "image/gif" => "gif",
            "image/svg+xml" => "svg",
            "image/bmp" => "bmp",
            "image/tiff" => "tif",
            "image/webp" => "webp",
            _ => "bin",
        };
        let path = format!("Pictures/{}.{ext}", self.pictures.len() + 1);
        self.pictures.push(Picture {
            id: id.clone(),
            path: path.clone(),
            mime: image.mime.to_owned(),
        });
        Ok(path)
    }
}
