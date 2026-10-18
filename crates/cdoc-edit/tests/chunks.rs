//! The chunks record: round-trips, and refuses a truncation, trailing bytes and a bad magic.

use cdoc_crdt::DecodeError;
use cdoc_edit::wire::{decode_chunks, encode_chunks};

#[test]
fn the_chunks_record_round_trips_and_refuses_what_is_not_one() {
    let chunks = vec![b"CDC\x02\x00one".to_vec(), Vec::new(), vec![7; 300]];
    let record = encode_chunks(&chunks);
    assert_eq!(decode_chunks(&record), Ok(chunks));
    assert_eq!(decode_chunks(&encode_chunks(&[])), Ok(Vec::new()));
    for cut in 0..record.len() {
        assert!(decode_chunks(&record[..cut]).is_err(), "cut at {cut}");
    }
    let mut trailing = record.clone();
    trailing.push(0);
    assert_eq!(decode_chunks(&trailing), Err(DecodeError::Trailing(1)));
    let mut magic = record;
    magic[3] = 2;
    assert_eq!(decode_chunks(&magic), Err(DecodeError::BadMagic));
}
