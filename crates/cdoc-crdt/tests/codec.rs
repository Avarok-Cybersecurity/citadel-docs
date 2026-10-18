//! The payload codec: model values round trip; floats are refused; malformed bytes are errors,
//! never panics. Also the static half of "no floats on any boundary".

mod common;

use cdoc_crdt::codec::{from_bytes, to_bytes, CodecError};
use cdoc_crdt::{ClientId, Doc, StateVector};
use cdoc_model::test_support::{fixtures, strategies};
use cdoc_model::Document;
use common::ROOM;
use proptest::prelude::*;

#[test]
fn the_kitchen_sink_round_trips_through_the_codec() {
    let d = fixtures::kitchen_sink();
    assert_eq!(from_bytes::<Document>(&to_bytes(&d).unwrap()).unwrap(), d);
}

#[test]
fn floats_are_refused() {
    assert_eq!(to_bytes(&1.5f64), Err(CodecError::Float));
    assert_eq!(to_bytes(&(1u8, vec![0.25f32])), Err(CodecError::Float));
}

#[test]
fn integers_keep_their_sign_and_range() {
    for v in [0i64, -1, 1, i64::MIN, i64::MAX] {
        assert_eq!(from_bytes::<i64>(&to_bytes(&v).unwrap()).unwrap(), v);
    }
    assert!(from_bytes::<u8>(&to_bytes(&300u32).unwrap()).is_err());
    assert!(from_bytes::<u32>(&to_bytes(&-1i32).unwrap()).is_err());
}

#[test]
fn trailing_bytes_and_truncation_are_errors() {
    let mut bytes = to_bytes(&"text").unwrap();
    bytes.push(0);
    assert!(from_bytes::<String>(&bytes).is_err());
    let bytes = to_bytes(&fixtures::kitchen_sink()).unwrap();
    for cut in [1, bytes.len() / 2, bytes.len() - 1] {
        assert!(from_bytes::<Document>(&bytes[..cut]).is_err());
    }
}

/// No source file of the crate names a float type, except the codec's refusal.
#[test]
fn the_crate_has_no_float_types() {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/src");
    let mut stack = vec![std::path::PathBuf::from(root)];
    let mut offenders = Vec::new();
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            for (n, line) in text.lines().enumerate() {
                let code = line.trim_start();
                // Allowed: comments, the serializer's two refusals, and the deserializer's
                // forwarding list (no float tag exists, so a float read always fails).
                let allowed = code.starts_with("//")
                    || code.starts_with("fn serialize_f32(")
                    || code.starts_with("fn serialize_f64(")
                    || code.starts_with("bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64");
                if !allowed && (code.contains("f32") || code.contains("f64")) {
                    offenders.push(format!("{}:{}: {line}", path.display(), n + 1));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "float types in the crate:\n{}",
        offenders.join("\n")
    );
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn every_generated_document_round_trips_through_the_codec(d in strategies::document()) {
        prop_assert_eq!(from_bytes::<Document>(&to_bytes(&d).unwrap()).unwrap(), d);
    }

    #[test]
    fn arbitrary_bytes_never_panic(bytes in prop::collection::vec(any::<u8>(), 0..64)) {
        let _ = from_bytes::<Document>(&bytes);
        let _ = StateVector::decode(&bytes);
        let _ = Doc::decode_snapshot(&bytes, ClientId(1));
        let _ = Doc::new(ClientId(1)).apply_update(&bytes, ROOM);
        let mut framed = b"CDC\x01\x00".to_vec();
        framed.extend_from_slice(&bytes);
        let _ = Doc::new(ClientId(1)).apply_update(&framed, ROOM);
        framed[4] = 1;
        let _ = Doc::decode_snapshot(&framed, ClientId(1));
    }
}
