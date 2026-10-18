// Adapted from rMazing crates/cc-export/src/backends/image.rs
// Copyright (c) Thomas Braun. Relicensed MIT OR Apache-2.0 by the copyright holder, 2026-10-04.

//! Images as PDF image `XObject`s, one stream per distinct content id however often it is drawn.
//!
//! A JPEG goes in untouched as `/DCTDecode`, so nothing here re-encodes a photograph and the
//! bytes in the file are the bytes the preview drew. Decoded PNG samples go in as Flate, with
//! an `/SMask` when the image has alpha.

use crate::picture::{Picture, Pixels};
use crate::ports::ImagePort;
use crate::PdfError;
use cdoc_layout::{Draw, Layout};
use cdoc_model::image_probe::{probe, ImageKind};
use cdoc_model::ContentId;
use pdf_writer::{Filter, Finish, Name, Pdf, Ref};
use std::collections::BTreeMap;

/// Each image's `XObject` and resource name, by content id.
pub struct Placed {
    /// Resource names, by content id.
    pub names: BTreeMap<String, String>,
    /// `XObject` references, by resource name.
    pub refs: BTreeMap<String, Ref>,
}

fn colour_space(components: u8) -> Name<'static> {
    match components {
        1 => Name(b"DeviceGray"),
        4 => Name(b"DeviceCMYK"),
        _ => Name(b"DeviceRGB"),
    }
}

fn write(pdf: &mut Pdf, alloc: &mut Ref, id: Ref, picture: &Picture) {
    let (w, h) = (
        i32::try_from(picture.width).unwrap_or(i32::MAX),
        i32::try_from(picture.height).unwrap_or(i32::MAX),
    );
    match &picture.pixels {
        Pixels::Jpeg { bytes, components } => {
            let mut image = pdf.image_xobject(id, bytes);
            image.filter(Filter::DctDecode);
            image.width(w).height(h).bits_per_component(8);
            image.pair(Name(b"ColorSpace"), colour_space(*components));
            image.finish();
        }
        Pixels::Raw {
            colour,
            components,
            alpha,
        } => {
            let mask = alpha.as_ref().map(|samples| {
                let mask = alloc.bump();
                let packed = miniz_oxide::deflate::compress_to_vec_zlib(samples, 6);
                let mut smask = pdf.image_xobject(mask, &packed);
                smask.filter(Filter::FlateDecode);
                smask.width(w).height(h).bits_per_component(8);
                smask.pair(Name(b"ColorSpace"), Name(b"DeviceGray"));
                smask.finish();
                mask
            });
            let packed = miniz_oxide::deflate::compress_to_vec_zlib(colour, 6);
            let mut image = pdf.image_xobject(id, &packed);
            image.filter(Filter::FlateDecode);
            image.width(w).height(h).bits_per_component(8);
            image.pair(Name(b"ColorSpace"), colour_space(*components));
            if let Some(mask) = mask {
                image.s_mask(mask);
            }
            image.finish();
        }
    }
}

fn drawn(layout: &Layout) -> impl Iterator<Item = &ContentId> {
    layout
        .pages
        .iter()
        .flat_map(|p| &p.draws)
        .filter_map(|d| match d {
            Draw::Image { content_id, .. } => Some(content_id),
            _ => None,
        })
}

/// Each image the layout draws whose type the documents do not draw (GIF, WebP, BMP, TIFF,
/// EMF, ...), with its loss-report entry: the PDF shows a placeholder box in its place.
pub fn unrendered(
    layout: &Layout,
    images: &dyn ImagePort,
) -> Result<BTreeMap<ContentId, String>, PdfError> {
    let mut out = BTreeMap::new();
    for id in drawn(layout) {
        let bytes = images.image(id)?;
        if ImageKind::from_mime(bytes.mime).is_none() {
            let why = probe(bytes.bytes).err().map_or_else(
                || format!("image type {} not rendered", bytes.mime),
                cdoc_model::image_probe::ProbeError::not_rendered,
            );
            out.insert(id.clone(), why);
        }
    }
    Ok(out)
}

/// Adds one `XObject` per distinct image the layout draws, of the types it draws.
pub fn place(
    pdf: &mut Pdf,
    alloc: &mut Ref,
    layout: &Layout,
    images: &dyn ImagePort,
) -> Result<Placed, PdfError> {
    let mut placed = Placed {
        names: BTreeMap::new(),
        refs: BTreeMap::new(),
    };
    for content_id in drawn(layout) {
        if placed.names.contains_key(content_id.as_str()) {
            continue;
        }
        let bytes = images.image(content_id)?;
        if ImageKind::from_mime(bytes.mime).is_none() {
            continue;
        }
        let picture = Picture::decode(bytes.mime, bytes.bytes)
            .map_err(|e| PdfError::Image(content_id.as_str().to_owned(), e))?;
        let id = alloc.bump();
        write(pdf, alloc, id, &picture);
        let name = format!("Im{}", placed.names.len());
        placed.refs.insert(name.clone(), id);
        placed.names.insert(content_id.as_str().to_owned(), name);
    }
    Ok(placed)
}
