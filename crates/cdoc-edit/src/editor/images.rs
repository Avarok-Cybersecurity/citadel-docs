//! Images the user adds: checked by their header, named by their content, and held for
//! whoever draws or exports them (an [`ImagePort`](cdoc_model::ImagePort)). The id is
//! `sha256-` and the hex digest of the bytes, so every peer and the content store derive the
//! same id from the same file and can check the bytes they fetch against it.

use super::Editor;
use crate::error::EditError;
use cdoc_crdt::ClientId;
use cdoc_layout::{FontPort, LayoutOptions};
use cdoc_model::image_probe::{probe, ImageKind};
use cdoc_model::{ContentId, Document, ImageLibrary, InlineContent};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// An image the editor now holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddedImage {
    /// The id to insert it by (`Command::InsertImage`) and to store its bytes under.
    pub content_id: ContentId,
    /// Its type, from the file's signature.
    pub kind: ImageKind,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

fn content_id(bytes: &[u8]) -> ContentId {
    let mut id = String::from("sha256-");
    for b in Sha256::digest(bytes) {
        write!(id, "{b:02x}").expect("writing to a string");
    }
    ContentId::new(id)
}

/// The checked file: its header, or the refusal `add_image` gives.
fn checked(
    bytes: &[u8],
    max_bytes: usize,
) -> Result<cdoc_model::image_probe::ImageHeader, EditError> {
    if bytes.len() > max_bytes {
        return Err(EditError::ImageTooLarge {
            size: bytes.len(),
            limit: max_bytes,
        });
    }
    Ok(probe(bytes)?)
}

impl<'f> Editor<'f> {
    /// A new document holding an imported `document` and the image files it came with, under
    /// the ids the importer gave them. Each file is checked as [`Editor::add_image`] checks
    /// it and renamed to its content id, in the document and in the editor's images; an
    /// image the document names that is not among `files` keeps its id (the content store
    /// has it).
    pub fn from_imported(
        mut document: Document,
        files: Vec<(ContentId, Vec<u8>)>,
        max_image_bytes: usize,
        client: ClientId,
        fonts: &'f dyn FontPort,
        options: &'f LayoutOptions,
    ) -> Result<Self, EditError> {
        let mut renamed = BTreeMap::new();
        let mut held = Vec::with_capacity(files.len());
        for (provisional, bytes) in files {
            let header = checked(&bytes, max_image_bytes)?;
            let id = content_id(&bytes);
            renamed.insert(provisional, id.clone());
            held.push((id, header.kind, bytes));
        }
        document.for_each_paragraph_mut(&mut |p| {
            for inline in &mut p.inlines {
                if let InlineContent::Image(image) = &mut inline.content {
                    if let Some(id) = renamed.get(&image.content_id) {
                        image.content_id = id.clone();
                    }
                }
            }
        });
        let mut editor = Self::from_document(&document, client, fonts, options)?;
        for (id, kind, bytes) in held {
            editor.images.add(id, kind.mime(), bytes);
        }
        Ok(editor)
    }

    /// Holds an image file of at most `max_bytes` bytes and returns its id and pixel size.
    /// PNG and JPEG are accepted, recognised by their signature; anything else is refused by
    /// name. The display size is the caller's (`Command::InsertImage`): pixels carry no
    /// physical size the editor could trust.
    pub fn add_image(&mut self, bytes: Vec<u8>, max_bytes: usize) -> Result<AddedImage, EditError> {
        let header = checked(&bytes, max_bytes)?;
        let id = content_id(&bytes);
        self.images.add(id.clone(), header.kind.mime(), bytes);
        Ok(AddedImage {
            content_id: id,
            kind: header.kind,
            width: header.width,
            height: header.height,
        })
    }

    /// The faces the editor lays out with.
    #[must_use]
    pub fn fonts(&self) -> &'f dyn FontPort {
        self.fonts
    }

    /// The images added in this session, by content id.
    #[must_use]
    pub const fn images(&self) -> &ImageLibrary {
        &self.images
    }
}
