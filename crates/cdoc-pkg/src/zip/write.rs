//! Writing an archive into one buffer: each local header, then the entry deflated straight
//! after it (the header's CRC and sizes patched once they are known), then the central
//! directory and the end record. No data descriptors, no extra fields, fixed timestamps, so
//! equal packages give equal bytes.

use super::Compression;
use crate::error::PackageError;
use flate2::{Compress, Crc, FlushCompress, Status};

/// The deflate level (zlib's scale, 0 to 9).
///
/// 6, zlib's default and what Word's own files are close to. Measured on the 500-page corpus
/// document part (2.9 MB): level 1 deflates 2.7x faster but writes a 46 % larger file, level 2
/// 2x faster and 12 % larger; 9 is 1.8x slower for 2 % smaller. A saved file is stored, synced
/// and attached far more often than it is written, so the size is worth the time.
pub const DEFLATE_LEVEL: u32 = 6;

/// 1980-01-01 00:00, the format's epoch, in MS-DOS date form.
const EPOCH_DATE: u16 = 0x21;
/// Unix, spec 1.0 (stored) or 2.0 (deflate): what the `zip` crate wrote before, kept so files
/// stay byte-for-byte what they were.
const MADE_BY_UNIX: u16 = 3 << 8;
/// A regular file, `rw-r--r--`.
const FILE_MODE: u32 = 0o100_644 << 16;
const UTF8_NAMES: u16 = 1 << 11;

fn err(detail: &str) -> PackageError {
    PackageError::Zip(detail.to_owned())
}

fn u32_of(n: usize) -> Result<u32, PackageError> {
    u32::try_from(n).map_err(|_| err("too large for an archive without zip64"))
}

struct Written {
    version: u16,
    flags: u16,
    method: u16,
    crc: u32,
    packed: u32,
    size: u32,
    offset: u32,
}

/// The header fields both records share, from `version` to the name length.
fn common(out: &mut Vec<u8>, w: &Written, name: &str) -> Result<(), PackageError> {
    let name_len = u16::try_from(name.len()).map_err(|_| err("an entry name is too long"))?;
    for v in [w.version, w.flags, w.method, 0, EPOCH_DATE] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    for v in [w.crc, w.packed, w.size] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(&name_len.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    Ok(())
}

fn deflate(bytes: &[u8], out: &mut Vec<u8>) -> Result<(), PackageError> {
    let mut deflater = Compress::new(flate2::Compression::new(DEFLATE_LEVEL), false);
    out.reserve(bytes.len() / 2 + 64);
    loop {
        let consumed = usize::try_from(deflater.total_in()).map_err(|_| err("too large"))?;
        let rest = bytes.get(consumed..).unwrap_or_default();
        let status = deflater
            .compress_vec(rest, out, FlushCompress::Finish)
            .map_err(|e| PackageError::Zip(e.to_string()))?;
        if status == Status::StreamEnd {
            return Ok(());
        }
        out.reserve(out.capacity().max(64));
    }
}

/// The archive of `entries` in the order given.
pub(super) fn write<'p>(
    entries: impl Iterator<Item = (&'p str, &'p [u8], Compression)>,
) -> Result<Vec<u8>, PackageError> {
    let mut out = Vec::new();
    let mut directory: Vec<(&str, Written)> = Vec::new();
    for (name, bytes, compression) in entries {
        let (version, method) = match compression {
            Compression::Stored => (10, 0),
            Compression::Deflated => (20, 8),
        };
        let mut crc = Crc::new();
        crc.update(bytes);
        let mut w = Written {
            version,
            flags: if name.is_ascii() { 0 } else { UTF8_NAMES },
            method,
            crc: crc.sum(),
            packed: 0,
            size: u32_of(bytes.len())?,
            offset: u32_of(out.len())?,
        };
        out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        let sizes_at = out.len() + 14;
        common(&mut out, &w, name)?;
        out.extend_from_slice(name.as_bytes());
        let data_at = out.len();
        match compression {
            Compression::Stored => out.extend_from_slice(bytes),
            Compression::Deflated => deflate(bytes, &mut out)?,
        }
        w.packed = u32_of(out.len() - data_at)?;
        out[sizes_at..sizes_at + 4].copy_from_slice(&w.packed.to_le_bytes());
        directory.push((name, w));
    }
    let dir_at = u32_of(out.len())?;
    for (name, w) in &directory {
        out.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        out.extend_from_slice(&(MADE_BY_UNIX | w.version).to_le_bytes());
        common(&mut out, w, name)?;
        out.extend_from_slice(&[0; 6]);
        out.extend_from_slice(&FILE_MODE.to_le_bytes());
        out.extend_from_slice(&w.offset.to_le_bytes());
        out.extend_from_slice(name.as_bytes());
    }
    let count = u16::try_from(directory.len()).map_err(|_| err("too many entries"))?;
    let dir_len = u32_of(out.len())? - dir_at;
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&dir_len.to_le_bytes());
    out.extend_from_slice(&dir_at.to_le_bytes());
    out.extend_from_slice(&[0; 2]);
    Ok(out)
}
