//! Reading an archive straight from its bytes: the end record, the central directory, each
//! entry's local header, then one inflate per part into a buffer of exactly the declared size,
//! checked against the declared CRC-32. Nothing is copied that is not part data.
//!
//! Refused loudly rather than misread: ZIP64, encryption, archives split over disks and any
//! compression but stored and deflate. Data descriptors need nothing: the central directory
//! holds the sizes and CRC they would repeat.

use super::cp437;
use crate::error::PackageError;
use crate::limits::ReadLimits;
use flate2::{Crc, Decompress, FlushDecompress, Status};
use std::collections::BTreeMap;

const END: u32 = 0x0605_4b50;
const CENTRAL: u32 = 0x0201_4b50;
const LOCAL: u32 = 0x0403_4b50;
const ZIP64_LOCATOR: u32 = 0x0706_4b50;
const END_LEN: usize = 22;
const CENTRAL_LEN: usize = 46;
const LOCAL_LEN: usize = 30;
const UTF8_NAMES: u16 = 1 << 11;
const ENCRYPTED: u16 = 1;

fn err(detail: impl Into<String>) -> PackageError {
    PackageError::Zip(detail.into())
}

fn u16_at(b: &[u8], at: usize) -> Result<u16, PackageError> {
    b.get(at..at.saturating_add(2))
        .map(|s| u16::from_le_bytes([s[0], s[1]]))
        .ok_or_else(|| err("truncated archive"))
}

fn u32_at(b: &[u8], at: usize) -> Result<u32, PackageError> {
    b.get(at..at.saturating_add(4))
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
        .ok_or_else(|| err("truncated archive"))
}

fn usize_at(b: &[u8], at: usize) -> Result<usize, PackageError> {
    u32_at(b, at).map(|v| v as usize)
}

/// Where the end of central directory record starts: the last signature that leaves room for
/// the record and its comment.
fn find_end(b: &[u8]) -> Result<usize, PackageError> {
    let last = b
        .len()
        .checked_sub(END_LEN)
        .ok_or_else(|| err("not a zip archive"))?;
    let first = last.saturating_sub(usize::from(u16::MAX));
    (first..=last)
        .rev()
        .find(|&at| u32_at(b, at).is_ok_and(|s| s == END))
        .ok_or_else(|| err("not a zip archive: no end of central directory"))
}

/// One central directory entry, as far as reading needs it.
struct Entry<'a> {
    name: &'a [u8],
    utf8: bool,
    method: u16,
    crc: u32,
    packed: usize,
    size: usize,
    local: usize,
}

fn entry(b: &[u8], at: usize, shift: usize) -> Result<(Entry<'_>, usize), PackageError> {
    if u32_at(b, at)? != CENTRAL {
        return Err(err("bad central directory entry signature"));
    }
    let flags = u16_at(b, at + 8)?;
    let (packed, size, local) = (
        u32_at(b, at + 20)?,
        u32_at(b, at + 24)?,
        u32_at(b, at + 42)?,
    );
    if [packed, size, local].contains(&u32::MAX) {
        return Err(err("zip64 archives are not supported"));
    }
    if flags & ENCRYPTED != 0 {
        return Err(err("encrypted entries are not supported"));
    }
    let name_len = usize::from(u16_at(b, at + 28)?);
    let rest = usize::from(u16_at(b, at + 30)?) + usize::from(u16_at(b, at + 32)?);
    let name = b
        .get(at.saturating_add(CENTRAL_LEN)..at.saturating_add(CENTRAL_LEN + name_len))
        .ok_or_else(|| err("truncated central directory"))?;
    let e = Entry {
        name,
        utf8: flags & UTF8_NAMES != 0,
        method: u16_at(b, at + 10)?,
        crc: u32_at(b, at + 16)?,
        packed: packed as usize,
        size: size as usize,
        local: (local as usize)
            .checked_add(shift)
            .ok_or_else(|| err("bad local header offset"))?,
    };
    Ok((e, at.saturating_add(CENTRAL_LEN + name_len + rest)))
}

fn name_of(e: &Entry<'_>) -> Result<String, PackageError> {
    if e.utf8 || e.name.is_ascii() {
        return std::str::from_utf8(e.name)
            .map(str::to_owned)
            .map_err(|_| err("an entry name flagged UTF-8 is not UTF-8"));
    }
    Ok(cp437::decode(e.name))
}

/// Where an entry lies (local header to end of data) and its compressed bytes.
fn data<'a>(b: &'a [u8], e: &Entry<'_>) -> Result<((usize, usize), &'a [u8]), PackageError> {
    if u32_at(b, e.local)? != LOCAL {
        return Err(err("bad local header signature"));
    }
    let skip = usize::from(u16_at(b, e.local.saturating_add(26))?)
        + usize::from(u16_at(b, e.local.saturating_add(28))?);
    let start = e.local.saturating_add(LOCAL_LEN + skip);
    let stop = start.saturating_add(e.packed);
    let packed = b
        .get(start..stop)
        .ok_or_else(|| err("truncated entry data"))?;
    Ok(((e.local, stop), packed))
}

fn inflate(packed: &[u8], size: usize) -> Result<Vec<u8>, PackageError> {
    let mut out = Vec::with_capacity(size + 1);
    let mut inflater = Decompress::new(false);
    let status = inflater
        .decompress_vec(packed, &mut out, FlushDecompress::Finish)
        .map_err(|e| err(e.to_string()))?;
    match (status, out.len() == size) {
        (Status::StreamEnd, true) => Ok(out),
        (Status::StreamEnd, false) => Err(err("an entry is not its declared size")),
        _ if out.len() > size => Err(err("an entry is larger than its declared size")),
        _ => Err(err("truncated deflate stream")),
    }
}

/// One part to inflate: its name, its central directory entry and where its local header and
/// data lie.
struct Part<'a> {
    name: String,
    entry: Entry<'a>,
    span: (usize, usize),
    packed: &'a [u8],
}

/// Every non-directory entry of the central directory, checked against `limits` before any
/// is inflated: duplicate names (after dropping a leading slash) and entries whose bytes
/// overlap (the overlapping-entry zip bomb) are refused.
fn parts<'a>(b: &'a [u8], end: usize, limits: &ReadLimits) -> Result<Vec<Part<'a>>, PackageError> {
    if end >= 20 && u32_at(b, end - 20)? == ZIP64_LOCATOR {
        return Err(err("zip64 archives are not supported"));
    }
    if u16_at(b, end + 4)? != 0 || u16_at(b, end + 6)? != 0 {
        return Err(err("archives split over several disks are not supported"));
    }
    let count = usize::from(u16_at(b, end + 10)?);
    let (dir_len, dir_at) = (usize_at(b, end + 12)?, usize_at(b, end + 16)?);
    if count == usize::from(u16::MAX) || dir_at == u32::MAX as usize {
        return Err(err("zip64 archives are not supported"));
    }
    if count > limits.max_parts {
        return Err(PackageError::TooLarge("part count"));
    }
    let shift = end
        .checked_sub(dir_at.saturating_add(dir_len))
        .ok_or_else(|| err("the central directory runs past its end record"))?;
    let mut found = Vec::with_capacity(count);
    let (mut at, mut total) = (dir_at.saturating_add(shift), 0u64);
    for _ in 0..count {
        let (e, next) = entry(b, at, shift)?;
        at = next;
        let name = name_of(&e)?;
        if name.ends_with(['/', '\\']) {
            continue;
        }
        let size = e.size as u64;
        if size > limits.max_part_bytes {
            return Err(PackageError::TooLarge("part size"));
        }
        total += size;
        if total > limits.max_total_bytes {
            return Err(PackageError::TooLarge("total size"));
        }
        if size > limits.ratio_grace_bytes
            && size > (e.packed as u64).saturating_mul(limits.max_ratio)
        {
            return Err(PackageError::TooLarge("compression ratio"));
        }
        let (span, packed) = data(b, &e)?;
        let name = name.trim_start_matches('/').to_owned();
        found.push(Part {
            name,
            entry: e,
            span,
            packed,
        });
    }
    refuse_duplicates_and_overlaps(&found)?;
    Ok(found)
}

fn refuse_duplicates_and_overlaps(parts: &[Part<'_>]) -> Result<(), PackageError> {
    let mut names: Vec<&str> = parts.iter().map(|p| p.name.as_str()).collect();
    names.sort_unstable();
    if let Some([name, _]) = names.windows(2).find(|w| w[0] == w[1]) {
        return Err(err(format!("duplicate entry {name}")));
    }
    let mut spans: Vec<(usize, usize)> = parts.iter().map(|p| p.span).collect();
    spans.sort_unstable();
    if spans.windows(2).any(|w| w[1].0 < w[0].1) {
        return Err(err("entries overlap"));
    }
    Ok(())
}

/// Every non-directory entry by name (no leading slash).
pub(super) fn read(
    b: &[u8],
    limits: &ReadLimits,
) -> Result<BTreeMap<String, Vec<u8>>, PackageError> {
    let end = find_end(b)?;
    let mut out = BTreeMap::new();
    for Part {
        name,
        entry: e,
        packed,
        ..
    } in parts(b, end, limits)?
    {
        let bytes = match e.method {
            0 if e.packed == e.size => packed.to_vec(),
            0 => return Err(err("a stored entry whose sizes disagree")),
            8 => inflate(packed, e.size)?,
            m => return Err(err(format!("unsupported compression method {m}"))),
        };
        let mut crc = Crc::new();
        crc.update(&bytes);
        if crc.sum() != e.crc {
            return Err(err(format!("bad CRC-32 in {name}")));
        }
        out.insert(name, bytes);
    }
    Ok(out)
}
