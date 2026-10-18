//! The editor through the facade: one [`EditSession`] per open document, driven by binary
//! request records and answering with binary result records (`docs/edit-commands.md`). Byte
//! level, so it is tested natively; `edit_bindings.rs` only adds the wasm-bindgen signatures.
//!
//! An editor borrows its faces and layout options for as long as it lives, and a JavaScript
//! object cannot hold a borrow, so [`EditFonts::seal`] moves them into a `'static` home that
//! is never freed. Seal once per page with every face the documents need and open every
//! editor on that one [`SealedFonts`]: what it holds is the font files, kept for the page's
//! life anyway.

use cdoc_crdt::ClientId;
use cdoc_edit::wire::{decode_request, encode_peer_view, encode_result};
use cdoc_edit::{EditError, Editor};
use cdoc_layout::{FontLibrary, LayoutOptions, Pt100};

/// Faces being gathered for editing; [`EditFonts::seal`] makes them usable.
#[derive(Debug)]
pub struct EditFonts(FontLibrary);

/// Faces and layout options sealed for the page's lifetime.
#[derive(Debug, Clone, Copy)]
pub struct SealedFonts(pub(crate) &'static (FontLibrary, LayoutOptions));

impl EditFonts {
    /// No faces yet; `fallback_family` serves any family not added or aliased.
    #[must_use]
    pub fn new(fallback_family: &str) -> Self {
        Self(FontLibrary::new(fallback_family))
    }

    /// Adds a face (a TrueType or OpenType file).
    pub fn add_font(&mut self, family: &str, bold: bool, italic: bool, bytes: &[u8]) {
        self.0.add(family, bold, italic, bytes.to_vec());
    }

    /// Serves `target` whenever a document asks for `name`.
    pub fn alias_font(&mut self, name: &str, target: &str) {
        self.0.alias(name, target);
    }

    /// Seals the faces with Word-like layout options; they live as long as the page.
    #[must_use]
    pub fn seal(self) -> SealedFonts {
        SealedFonts(Box::leak(Box::new((self.0, LayoutOptions::word_like()))))
    }
}

/// One open document.
pub struct EditSession(pub(crate) Editor<'static>);

pub(crate) fn text(e: &EditError) -> String {
    e.to_string()
}

impl EditSession {
    /// Opens a replica from a CRDT snapshot as client `client`.
    pub fn open(snapshot: &[u8], client: u64, fonts: SealedFonts) -> Result<Self, String> {
        let (library, options) = fonts.0;
        Editor::open(snapshot, ClientId(client), library, options)
            .map(Self)
            .map_err(|e| text(&e))
    }

    /// A blank document as client `client`; every blank is the same document, so peers that
    /// open blank converge with it.
    pub fn blank(client: u64, fonts: SealedFonts) -> Result<Self, String> {
        let (library, options) = fonts.0;
        Editor::blank(ClientId(client), library, options)
            .map(Self)
            .map_err(|e| text(&e))
    }

    /// Runs a request record and returns the result record.
    pub fn request(&mut self, request: &[u8]) -> Result<Vec<u8>, String> {
        let req = decode_request(request).map_err(|e| e.to_string())?;
        let applied = self.0.handle(req).map_err(|e| text(&e))?;
        let outcome = self
            .0
            .outcome(applied.update, applied.dirty_pages, applied.created)
            .map_err(|e| text(&e))?;
        Ok(encode_result(&outcome))
    }

    /// Applies a peer's update and returns the result record (its update is empty).
    pub fn apply_remote(&mut self, update: &[u8]) -> Result<Vec<u8>, String> {
        let r = self.0.apply_remote(update).map_err(|e| text(&e))?;
        let outcome = self
            .0
            .outcome(Vec::new(), r.dirty_pages, Vec::new())
            .map_err(|e| text(&e))?;
        Ok(encode_result(&outcome))
    }

    /// The display list of `pages`.
    pub fn paint(&self, pages: &[u32]) -> Result<Vec<u8>, String> {
        let pages: Vec<usize> = pages
            .iter()
            .map(|&p| usize::try_from(p).map_err(|e| e.to_string()))
            .collect::<Result<_, _>>()?;
        self.0.paint(&pages).map_err(|e| text(&e))
    }

    /// The position record nearest `(x, y)` on `page` (hundredths of a point).
    pub fn hit_test(&self, page: u32, x: Pt100, y: Pt100) -> Result<Vec<u8>, String> {
        let page = usize::try_from(page).map_err(|e| e.to_string())?;
        let pos = self.0.hit_test(page, x, y).map_err(|e| text(&e))?;
        Ok(cdoc_edit::wire::encode_position(pos))
    }

    /// Pages in the document.
    #[must_use]
    pub fn page_count(&self) -> usize {
        self.0.page_count()
    }

    /// Page `page`'s width and height, hundredths of a point.
    #[must_use]
    pub fn page_size(&self, page: u32) -> Option<[Pt100; 2]> {
        let (w, h) = self.0.page_size(usize::try_from(page).ok()?)?;
        Some([w, h])
    }

    /// The full CRDT snapshot.
    #[must_use]
    pub fn snapshot(&self) -> Vec<u8> {
        self.0.snapshot()
    }

    /// The replica's state vector.
    #[must_use]
    pub fn state_vector(&self) -> Vec<u8> {
        self.0.state_vector()
    }

    /// What a peer at state vector `sv` lacks.
    pub fn encode_diff(&self, sv: &[u8]) -> Result<Vec<u8>, String> {
        self.0.encode_diff(sv).map_err(|e| text(&e))
    }

    /// Collects the tombstones every peer in `known` (a list of their state vectors) has
    /// seen; returns the counts record. Only the session knows when that is safe.
    pub fn gc(&mut self, known: &[u8]) -> Result<Vec<u8>, String> {
        let known = cdoc_edit::wire::decode_known(known).map_err(|e| e.to_string())?;
        let collected = self.0.gc(&known).map_err(|e| text(&e))?;
        Ok(cdoc_edit::wire::encode_collected(&collected))
    }

    /// Holds an image file of at most `max_bytes` bytes; returns the added-image record (its
    /// content id and pixel size). PNG and JPEG only.
    pub fn add_image(&mut self, bytes: &[u8], max_bytes: usize) -> Result<Vec<u8>, String> {
        let added = self
            .0
            .add_image(bytes.to_vec(), max_bytes)
            .map_err(|e| text(&e))?;
        Ok(cdoc_edit::wire::encode_added_image(&added))
    }

    /// The image record (MIME type and bytes) of a held image; an unknown id fails by name.
    pub fn image(&self, content_id: &str) -> Result<Vec<u8>, String> {
        let id = cdoc_model::ContentId::new(content_id);
        let image =
            cdoc_model::ImagePort::image(self.0.images(), &id).map_err(|e| e.to_string())?;
        Ok(cdoc_edit::wire::encode_image(&image))
    }

    /// The list record of every held image: id, MIME type, size in bytes.
    #[must_use]
    pub fn images(&self) -> Vec<u8> {
        cdoc_edit::wire::encode_image_list(self.0.images())
    }

    /// The document's styles record (resolved, for the gallery).
    pub fn styles(&self) -> Result<Vec<u8>, String> {
        let styles = self.0.styles().map_err(|e| text(&e))?;
        Ok(cdoc_edit::wire::encode_styles(&styles))
    }

    /// This replica's awareness record.
    #[must_use]
    pub fn awareness(&self) -> Vec<u8> {
        self.0.awareness()
    }

    /// A peer's awareness record as a peer-view record of rectangles.
    pub fn peer_view(&self, record: &[u8]) -> Result<Vec<u8>, String> {
        self.0
            .peer_view(record)
            .map(|v| encode_peer_view(&v))
            .map_err(|e| text(&e))
    }
}
