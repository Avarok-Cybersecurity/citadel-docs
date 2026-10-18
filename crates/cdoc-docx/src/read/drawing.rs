//! Inline pictures. Anything else `DrawingML` can hold (shapes, charts) is reported.

use super::Reader;
use crate::error::DocxError;
use crate::xml::El;
use cdoc_model::image_probe::{carried_mime, held_opaque, OPAQUE_FILE};
use cdoc_model::ImageBytes;
use cdoc_model::{InlineImage, Length};

/// EMU per hundredth of a point.
const EMU_PER_PT100: i64 = 127;

fn dig<'t>(node: El<'t>, path: &[&str]) -> Option<El<'t>> {
    path.iter().try_fold(node, |n, name| n.find(name))
}

fn emu(node: El<'_>, key: &str) -> Option<Length> {
    let emus: i64 = node.get(key)?.parse().ok()?;
    let units = i32::try_from(round_div_i64(emus, EMU_PER_PT100)).ok()?;
    Some(Length(units))
}

const fn round_div_i64(n: i64, d: i64) -> i64 {
    (n + d / 2).div_euclid(d)
}

fn mime_for(extension: &str) -> Option<&'static str> {
    Some(match extension {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        "tif" | "tiff" => "image/tiff",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        _ => return None,
    })
}

impl Reader<'_> {
    pub(super) fn drawing(&mut self, drawing: El<'_>) -> Result<Option<InlineImage>, DocxError> {
        let Some(holder) = drawing
            .find("wp:inline")
            .or_else(|| drawing.find("wp:anchor"))
        else {
            self.log
                .lose("w:drawing that is neither inline nor anchored");
            return Ok(None);
        };
        if holder.name() == "wp:anchor" {
            self.log.lose("floating image placed inline");
        }
        let blip = dig(
            holder,
            &[
                "a:graphic",
                "a:graphicData",
                "pic:pic",
                "pic:blipFill",
                "a:blip",
            ],
        );
        let Some(blip) = blip else {
            self.log
                .lose("drawing that is not a picture (shape, chart or diagram)");
            return Ok(None);
        };
        let Some(extent) = holder.find("wp:extent") else {
            self.log.lose("a picture without an extent");
            return Ok(None);
        };
        let (Some(width), Some(height)) = (emu(extent, "cx"), emu(extent, "cy")) else {
            self.log.lose("a picture with an unreadable size");
            return Ok(None);
        };
        let Some(id) = self.picture_part(blip.get("r:embed"))? else {
            return Ok(None);
        };
        let alt = holder
            .find("wp:docPr")
            .and_then(|n| n.get("descr"))
            .map(str::to_owned);
        Ok(Some(InlineImage {
            content_id: id,
            width,
            height,
            alt,
        }))
    }

    fn picture_part(
        &mut self,
        rid: Option<&str>,
    ) -> Result<Option<cdoc_model::ContentId>, DocxError> {
        let Some(rel) = rid.and_then(|id| self.rels.by_id(id)) else {
            self.log
                .lose("a picture whose relationship cannot be found (linked or missing)");
            return Ok(None);
        };
        if rel.external {
            self.log.lose("a linked (external) picture");
            return Ok(None);
        }
        let part = crate::package::resolve(&self.log.part, &rel.target);
        if let Some(id) = self.images.get(&part) {
            return Ok(Some(id.clone()));
        }
        let Some(bytes) = self.pkg.get(&part) else {
            self.log.lose("a picture whose part is missing");
            return Ok(None);
        };
        let extension = part
            .rsplit('.')
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        let mime = self
            .types
            .of(&part)
            .filter(|m| m.starts_with("image/"))
            .map(str::to_owned)
            .or_else(|| mime_for(&extension).map(str::to_owned));
        let Some(mime) = mime else {
            self.log.lose("a picture of an unknown type");
            return Ok(None);
        };
        let mime = carried_mime(&mime).unwrap_or_else(|| {
            self.log.lose(&held_opaque(&mime));
            OPAQUE_FILE
        });
        let id = self.sink.store(ImageBytes { mime, bytes })?;
        self.consumed.insert(part.clone());
        self.images.insert(part, id.clone());
        Ok(Some(id))
    }
}
