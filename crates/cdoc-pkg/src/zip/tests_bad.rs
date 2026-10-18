//! Damaged and unsupported archives: each is an error that says what is wrong, never a panic,
//! and never a part with the wrong bytes.

use super::tests::{crate_archive, ours, OPC};
use super::*;
use zip::CompressionMethod;

fn one_part(method: CompressionMethod) -> Vec<u8> {
    let text = "<w:p><w:r><w:t>hello</w:t></w:r></w:p>".repeat(40);
    crate_archive(&[("word/document.xml", text.as_bytes(), method)], false)
}

/// Where the end record and the first central directory entry start.
fn records(bytes: &[u8]) -> (usize, usize) {
    let end = (0..bytes.len() - 21)
        .rev()
        .find(|&i| bytes[i..i + 4] == [0x50, 0x4b, 0x05, 0x06])
        .expect("an end record");
    let dir = u32::from_le_bytes(bytes[end + 16..end + 20].try_into().expect("4 bytes"));
    (end, dir as usize)
}

fn zip_error(result: Result<BTreeMap<String, Vec<u8>>, PackageError>) -> String {
    match result {
        Err(PackageError::Zip(detail)) => detail,
        other => panic!("expected a zip error, got {other:?}"),
    }
}

#[test]
fn every_truncation_of_an_archive_is_an_error_not_a_panic() {
    let bytes = Package::write(&{
        let mut p = Package::empty();
        p.put("[Content_Types].xml", b"<Types/>".to_vec());
        p.put("word/document.xml", vec![b'a'; 3000]);
        p
    }, OPC)
    .expect("writes");
    for len in 0..bytes.len() {
        assert!(ours(&bytes[..len]).is_err(), "a {len}-byte prefix read as an archive");
    }
}

#[test]
fn a_wrong_crc_is_reported_for_deflated_and_stored_entries() {
    for method in [CompressionMethod::Deflated, CompressionMethod::Stored] {
        let mut bytes = one_part(method);
        let (_, dir) = records(&bytes);
        bytes[dir + 16] ^= 0x01;
        assert!(zip_error(ours(&bytes)).contains("CRC"), "{method:?}");
    }
    let mut bytes = one_part(CompressionMethod::Stored);
    let at = bytes.windows(5).position(|w| w == b"hello").expect("stored text");
    bytes[at] = b'j';
    assert!(zip_error(ours(&bytes)).contains("CRC"), "damaged stored data");
}

#[test]
fn a_central_directory_offset_off_by_one_is_refused() {
    let mut bytes = one_part(CompressionMethod::Deflated);
    let (end, dir) = records(&bytes);
    bytes[end + 16..end + 20].copy_from_slice(&u32::try_from(dir + 1).expect("small").to_le_bytes());
    assert!(ours(&bytes).is_err());
}

#[test]
fn zip64_markers_are_refused_by_name() {
    let base = one_part(CompressionMethod::Deflated);
    let (end, dir) = records(&base);
    let patches: [(usize, &[u8]); 4] = [
        (end + 10, &[0xFF, 0xFF]),
        (end + 16, &[0xFF; 4]),
        (dir + 20, &[0xFF; 4]),
        (dir + 24, &[0xFF; 4]),
    ];
    for (at, patch) in patches {
        let mut bytes = base.clone();
        bytes[at..at + patch.len()].copy_from_slice(patch);
        assert!(zip_error(ours(&bytes)).contains("zip64"), "patch at {at}");
    }
    let mut located = base[..end].to_vec();
    located.extend_from_slice(&[0x50, 0x4b, 0x06, 0x07]);
    located.extend_from_slice(&[0; 16]);
    located.extend_from_slice(&base[end..]);
    assert!(zip_error(ours(&located)).contains("zip64"), "a zip64 locator");
}

#[test]
fn encryption_and_unknown_methods_are_refused_by_name() {
    let base = one_part(CompressionMethod::Deflated);
    let (_, dir) = records(&base);
    let mut encrypted = base.clone();
    encrypted[dir + 8] |= 1;
    assert!(zip_error(ours(&encrypted)).contains("encrypted"));
    let mut bzip2 = base;
    bzip2[dir + 10] = 12;
    assert!(zip_error(ours(&bzip2)).contains("method 12"));
}

#[test]
fn sizes_that_disagree_with_the_data_are_refused() {
    let base = one_part(CompressionMethod::Deflated);
    let (_, dir) = records(&base);
    for delta in [-1i64, 1] {
        let mut bytes = base.clone();
        let size = u32::from_le_bytes(bytes[dir + 24..dir + 28].try_into().expect("4 bytes"));
        let lied = u32::try_from(i64::from(size) + delta).expect("small");
        bytes[dir + 24..dir + 28].copy_from_slice(&lied.to_le_bytes());
        assert!(zip_error(ours(&bytes)).contains("size"), "delta {delta}");
    }
}

#[test]
fn not_an_archive_says_so() {
    for bytes in [&b""[..], b"PK", b"definitely not a zip at all, not even close"] {
        assert!(zip_error(ours(bytes)).contains("not a zip"));
    }
}
