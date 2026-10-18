//! What an image file is and how many pixels it has, read from its header alone. The editor
//! uses it to accept an uploaded image, the PDF writer to check a JPEG before embedding it
//! verbatim. Decoding pixels stays with whoever draws them.

/// An image type the documents carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImageKind {
    /// PNG.
    Png,
    /// JPEG (baseline or progressive).
    Jpeg,
}

impl ImageKind {
    /// The MIME type.
    #[must_use]
    pub const fn mime(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
        }
    }
}

/// An image's type and size in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ImageHeader {
    /// The type, from the file's signature.
    pub kind: ImageKind,
    /// Width in pixels, at least 1.
    pub width: u32,
    /// Height in pixels, at least 1.
    pub height: u32,
}

/// Why a file is not an image the documents can carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ProbeError {
    /// A known image type the documents do not carry, or no known signature at all.
    #[error("unsupported image type: {0}")]
    Unsupported(&'static str),
    /// The signature is right but the header is cut short or describes no pixels.
    #[error("not a complete {0:?} file")]
    Malformed(ImageKind),
}

/// Signatures of types refused by name.
const OTHERS: [(&[u8], &str); 6] = [
    (b"GIF8", "GIF"),
    (b"BM", "BMP"),
    (b"II*\0", "TIFF"),
    (b"MM\0*", "TIFF"),
    (b"<svg", "SVG"),
    (b"<?xml", "SVG or XML"),
];

const PNG: &[u8; 8] = b"\x89PNG\r\n\x1a\n";

/// The JPEG's size and components from its first start-of-frame marker.
#[must_use]
pub fn jpeg_frame(bytes: &[u8]) -> Option<(u32, u32, u8)> {
    if !bytes.starts_with(&[0xFF, 0xD8]) || !bytes.ends_with(&[0xFF, 0xD9]) {
        return None;
    }
    let mut i = 2;
    while i + 4 <= bytes.len() {
        if bytes[i] != 0xFF {
            return None;
        }
        let marker = bytes[i + 1];
        let length = usize::from(u16::from_be_bytes([bytes[i + 2], bytes[i + 3]]));
        let is_frame = matches!(marker, 0xC0..=0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF);
        if is_frame {
            let frame = bytes.get(i + 4..i + 2 + length)?;
            let height = u32::from(u16::from_be_bytes([*frame.get(1)?, *frame.get(2)?]));
            let width = u32::from(u16::from_be_bytes([*frame.get(3)?, *frame.get(4)?]));
            let components = *frame.get(5)?;
            let usable = width > 0 && height > 0 && matches!(components, 1 | 3 | 4);
            return usable.then_some((width, height, components));
        }
        i += 2 + length;
    }
    None
}

/// The PNG's size from its `IHDR` chunk, which must come first.
fn png_size(bytes: &[u8]) -> Option<(u32, u32)> {
    let ihdr = bytes.get(8..24)?;
    if ihdr[..8] != [0, 0, 0, 13, b'I', b'H', b'D', b'R'] {
        return None;
    }
    let width = u32::from_be_bytes(ihdr[8..12].try_into().ok()?);
    let height = u32::from_be_bytes(ihdr[12..16].try_into().ok()?);
    (width > 0 && height > 0).then_some((width, height))
}

/// The type and size of `bytes`, from the signature, never from a declared type.
pub fn probe(bytes: &[u8]) -> Result<ImageHeader, ProbeError> {
    let (kind, size) = if bytes.starts_with(PNG) {
        (ImageKind::Png, png_size(bytes))
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        (ImageKind::Jpeg, jpeg_frame(bytes).map(|(w, h, _)| (w, h)))
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        return Err(ProbeError::Unsupported("WebP"));
    } else {
        let name = OTHERS
            .iter()
            .find(|(sig, _)| bytes.starts_with(sig))
            .map_or("unknown", |(_, name)| name);
        return Err(ProbeError::Unsupported(name));
    };
    let (width, height) = size.ok_or(ProbeError::Malformed(kind))?;
    Ok(ImageHeader {
        kind,
        width,
        height,
    })
}
