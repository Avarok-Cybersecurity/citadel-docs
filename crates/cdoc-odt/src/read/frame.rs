//! Frames: an image goes to the media sink (once per package path) and becomes an inline
//! image with its size and description; anything else a frame holds is reported.

use super::ctx::Ctx;
use super::Reader;
use crate::units;
use cdoc_model::image_probe::{carried_mime, held_opaque, OPAQUE_FILE};
use cdoc_model::{ContentId, ImageBytes, Inline, InlineContent, InlineImage, Length};
use cdoc_pkg::xml::Node;
use cdoc_pkg::PackageError;

fn mime_of(path: &str) -> &'static str {
    let ext = path
        .rsplit('.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "bmp" => "image/bmp",
        "tif" | "tiff" => "image/tiff",
        "webp" => "image/webp",
        _ => "application/octet-stream",
    }
}

impl Reader<'_> {
    fn image_id(&mut self, path: &str) -> Result<Option<ContentId>, PackageError> {
        if let Some(id) = self.images.get(path) {
            return Ok(Some(id.clone()));
        }
        let Some(bytes) = self.pkg.get(path) else {
            return Ok(None);
        };
        let mime = self
            .media_types
            .get(path)
            .map_or_else(|| mime_of(path), String::as_str);
        let mime = carried_mime(mime).unwrap_or_else(|| {
            self.log.lose(&held_opaque(mime));
            OPAQUE_FILE
        });
        let id = self.sink.store(ImageBytes { mime, bytes })?;
        self.consumed.insert(path.to_owned());
        self.images.insert(path.to_owned(), id.clone());
        Ok(Some(id))
    }

    pub(super) fn frame(
        &mut self,
        n: &Node,
        ctx: &mut Ctx<'_>,
        out: &mut Vec<Inline>,
    ) -> Result<(), PackageError> {
        let anchor = n.get("text:anchor-type").unwrap_or("paragraph");
        if anchor != "as-char" {
            self.log.lose(&format!(
                "an image anchored to the {anchor} read as an inline image"
            ));
        }
        for child in n
            .elems()
            .filter(|c| !matches!(c.name.as_str(), "draw:image" | "svg:desc" | "svg:title"))
        {
            self.log.lose(&format!("draw:frame/{}", child.name));
        }
        let Some(href) = n.find("draw:image").and_then(|i| i.get("xlink:href")) else {
            return Ok(());
        };
        let Some(content_id) = self.image_id(href.trim_start_matches("./"))? else {
            self.log.lose("an image whose file is not in the package");
            return Ok(());
        };
        let size = |key: &str| {
            n.get(key)
                .and_then(units::length)
                .filter(|v| *v > 0)
                .map(Length)
        };
        let (Some(width), Some(height)) = (size("svg:width"), size("svg:height")) else {
            self.log.lose("an image without a size");
            return Ok(());
        };
        let alt = n
            .find("svg:desc")
            .or_else(|| n.find("svg:title"))
            .map(Node::content);
        ctx.push(
            InlineContent::Image(InlineImage {
                content_id,
                width,
                height,
                alt,
            }),
            out,
        );
        ctx.space.visible();
        Ok(())
    }
}
