//! Archives built to cost more than they hold: duplicate names, entries sharing bytes, and
//! parts that inflate far past their compressed size. Each is refused before anything inflates.

use super::tests::{ours, OPC};
use super::*;

fn package(parts: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let mut p = Package::empty();
    for (name, bytes) in parts {
        p.put(name, bytes.clone());
    }
    p.write(OPC).expect("writes")
}

/// Every central directory entry's start, in order.
fn central_entries(bytes: &[u8]) -> Vec<usize> {
    (0..bytes.len() - 3)
        .filter(|&i| bytes[i..i + 4] == [0x50, 0x4b, 0x01, 0x02])
        .collect()
}

fn zip_detail(bytes: &[u8]) -> String {
    match ours(bytes) {
        Err(PackageError::Zip(detail)) => detail,
        other => panic!("expected a zip error, got {other:?}"),
    }
}

/// `from` renamed to `to` (the same length) in its local header and central directory entry.
fn renamed(bytes: &[u8], from: &[u8], to: &[u8]) -> Vec<u8> {
    let mut out = bytes.to_vec();
    let hits: Vec<usize> = (0..=bytes.len() - from.len())
        .filter(|&i| &bytes[i..i + from.len()] == from)
        .collect();
    assert_eq!(hits.len(), 2, "one local and one central name");
    for at in hits {
        out[at..at + to.len()].copy_from_slice(to);
    }
    out
}

#[test]
fn two_entries_with_one_name_are_refused_not_resolved_by_order() {
    let bytes = package(&[
        ("[Content_Types].xml", b"<T/>".to_vec()),
        ("b.xml", b"<b/>".to_vec()),
        ("cb.xml", b"<c/>".to_vec()),
        ("d.xml", b"<d/>".to_vec()),
    ]);
    assert!(ours(&bytes).is_ok());
    assert_eq!(
        zip_detail(&renamed(&bytes, b"d.xml", b"b.xml")),
        "duplicate entry b.xml"
    );
    assert_eq!(
        zip_detail(&renamed(&bytes, b"cb.xml", b"/b.xml")),
        "duplicate entry b.xml",
        "a leading slash names the same part"
    );
}

#[test]
fn entries_that_share_bytes_are_refused() {
    let bytes = package(&[
        ("[Content_Types].xml", b"<T/>".to_vec()),
        ("b.xml", vec![b'b'; 300]),
    ]);
    let dirs = central_entries(&bytes);
    let first_local = bytes[dirs[0] + 42..dirs[0] + 46].to_vec();
    let mut shared = bytes;
    shared[dirs[1] + 42..dirs[1] + 46].copy_from_slice(&first_local);
    assert_eq!(zip_detail(&shared), "entries overlap");
}

#[test]
fn a_part_inflating_past_the_ratio_is_refused_once_past_the_grace_size() {
    let limits = ReadLimits::generous();
    let past = usize::try_from(limits.ratio_grace_bytes).expect("small") + 1;
    let bomb = package(&[("[Content_Types].xml", vec![b' '; past])]);
    assert_eq!(
        Package::read(&bomb, &limits),
        Err(PackageError::TooLarge("compression ratio"))
    );
    let small = package(&[("[Content_Types].xml", vec![b' '; past - 1])]);
    assert!(
        Package::read(&small, &limits).is_ok(),
        "within the grace size"
    );
    let lax = ReadLimits {
        max_ratio: u64::MAX,
        ..limits
    };
    assert!(
        Package::read(&bomb, &lax).is_ok(),
        "the ratio is the caller's to set"
    );
}
