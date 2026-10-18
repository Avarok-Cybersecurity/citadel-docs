// Adapted from rMazing crates/cc-export/src/layout/picture.rs
// Copyright (c) Thomas Braun. Relicensed MIT OR Apache-2.0 by the copyright holder, 2026-10-04.

//! An image the document draws, checked before it reaches a stream.
//!
//! JPEG is carried through untouched: a PDF embeds a JPEG stream verbatim (`/DCTDecode`) and
//! the preview shows the same bytes as a data URI, so the two backends draw one byte string.
//! PNG is decoded, because a PDF cannot hold a PNG file: its pixels become a Flate stream and
//! its alpha, if any, a soft mask.
//!
//! Checked here rather than trusted, because these bytes are written into a PDF stream and
//! never parsed again by anything this project controls: a truncated stream would make a file
//! a reader's viewer refuses to open.

use cdoc_model::image_probe::jpeg_frame;
use png::{ColorType, Transformations};

/// Pixels ready for an image `XObject`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pixels {
    /// A JPEG file, verbatim, with its component count (1 gray, 3 RGB, 4 CMYK).
    Jpeg {
        /// The file.
        bytes: Vec<u8>,
        /// Colour components.
        components: u8,
    },
    /// Decoded 8-bit samples.
    Raw {
        /// Gray or RGB samples, row by row.
        colour: Vec<u8>,
        /// 1 or 3.
        components: u8,
        /// One alpha sample per pixel, when the image has transparency.
        alpha: Option<Vec<u8>>,
    },
}

/// An image, decoded as far as embedding needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picture {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// The samples.
    pub pixels: Pixels,
}

/// Bytes that are not the image their MIME type says.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PictureError {
    /// Not a complete baseline or progressive JPEG.
    #[error("not a complete JPEG")]
    BadJpeg,
    /// The PNG did not decode.
    #[error("not a readable PNG: {0}")]
    BadPng(String),
    /// A type other than JPEG or PNG.
    #[error("unsupported image type {0:?}")]
    Unsupported(String),
}

fn png(bytes: &[u8]) -> Result<Picture, PictureError> {
    let bad = |e: &dyn std::fmt::Display| PictureError::BadPng(e.to_string());
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().map_err(|e| bad(&e))?;
    let size = reader
        .output_buffer_size()
        .ok_or_else(|| PictureError::BadPng("image too large".to_owned()))?;
    let mut buffer = vec![0; size];
    let frame = reader.next_frame(&mut buffer).map_err(|e| bad(&e))?;
    let samples = &buffer[..frame.buffer_size()];
    let (components, has_alpha) = match frame.color_type {
        ColorType::Grayscale => (1, false),
        ColorType::GrayscaleAlpha => (1, true),
        ColorType::Rgb => (3, false),
        ColorType::Rgba => (3, true),
        ColorType::Indexed => return Err(PictureError::BadPng("unexpanded palette".to_owned())),
    };
    let (colour, alpha) = if has_alpha {
        let stride = usize::from(components) + 1;
        let colour = samples
            .chunks_exact(stride)
            .flat_map(|px| px[..stride - 1].iter().copied())
            .collect();
        let alpha = samples
            .chunks_exact(stride)
            .map(|px| px[stride - 1])
            .collect();
        (colour, Some(alpha))
    } else {
        (samples.to_vec(), None)
    };
    Ok(Picture {
        width: frame.width,
        height: frame.height,
        pixels: Pixels::Raw {
            colour,
            components,
            alpha,
        },
    })
}

impl Picture {
    /// Decodes `bytes` as the declared `mime` type.
    pub fn decode(mime: &str, bytes: &[u8]) -> Result<Self, PictureError> {
        match mime {
            "image/jpeg" => {
                let (width, height, components) = jpeg_frame(bytes).ok_or(PictureError::BadJpeg)?;
                Ok(Self {
                    width,
                    height,
                    pixels: Pixels::Jpeg {
                        bytes: bytes.to_vec(),
                        components,
                    },
                })
            }
            "image/png" => png(bytes),
            other => Err(PictureError::Unsupported(other.to_owned())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// SOI, an SOF0 of 4 × 3 with three components, EOI.
    fn jpeg() -> Vec<u8> {
        let mut bytes = vec![
            0xFF, 0xD8, 0xFF, 0xC0, 0x00, 0x11, 0x08, 0x00, 0x03, 0x00, 0x04, 0x03,
        ];
        bytes.extend([0x01, 0x22, 0x00, 0x02, 0x11, 0x01, 0x03, 0x11, 0x01]);
        bytes.extend([0xFF, 0xD9]);
        bytes
    }

    #[test]
    fn a_jpeg_frame_gives_its_size_and_components() {
        let p = Picture::decode("image/jpeg", &jpeg()).expect("a jpeg");
        assert_eq!((p.width, p.height), (4, 3));
        assert!(matches!(p.pixels, Pixels::Jpeg { components: 3, .. }));
    }

    #[test]
    fn what_is_not_the_declared_type_is_refused_before_it_reaches_a_stream() {
        assert_eq!(
            Picture::decode("image/jpeg", b"\x89PNG\r\n\x1a\n"),
            Err(PictureError::BadJpeg)
        );
        let mut cut = jpeg();
        cut.truncate(cut.len() - 1);
        assert_eq!(
            Picture::decode("image/jpeg", &cut),
            Err(PictureError::BadJpeg)
        );
        assert!(matches!(
            Picture::decode("image/png", &jpeg()),
            Err(PictureError::BadPng(_))
        ));
        assert!(matches!(
            Picture::decode("image/gif", &jpeg()),
            Err(PictureError::Unsupported(_))
        ));
    }
}
