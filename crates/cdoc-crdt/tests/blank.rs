//! The blank document is the same replica whoever opens it: its base items are authored by
//! the reserved client, so blanks opened apart take each other's updates; and no replica may
//! write as that client.

use cdoc_crdt::{blank, from_model, to_model, BridgeError, ClientId, Doc};
use cdoc_model::Document;

#[test]
fn every_blank_is_the_same_document() {
    let (a, b) = (blank(ClientId(7)).unwrap(), blank(ClientId(9)).unwrap());
    assert_eq!(a.encode_snapshot(), b.encode_snapshot());
    assert_eq!((a.client(), b.client()), (ClientId(7), ClientId(9)));
    let mut want = Document::blank();
    want.normalize();
    assert_eq!(to_model(&a).unwrap(), want);
    let sv = a.state_vector();
    assert_eq!(
        sv.iter().map(|(c, _)| c).collect::<Vec<_>>(),
        [ClientId::BLANK]
    );
}

#[test]
fn blanks_opened_apart_converge() {
    let (mut a, mut b) = (blank(ClientId(1)).unwrap(), blank(ClientId(2)).unwrap());
    let before = a.state_vector();
    let end = a.len();
    a.insert_text(end, "hello").unwrap();
    let update = a.encode_diff(&before);
    b.apply_update(&update).unwrap();
    assert_eq!(to_model(&a).unwrap(), to_model(&b).unwrap());
    let fresh = Doc::decode_snapshot(&a.encode_snapshot(), ClientId(3)).unwrap();
    assert_eq!(fresh.encode_snapshot(), b.encode_snapshot());
}

#[test]
fn no_replica_writes_as_the_reserved_client() {
    assert_eq!(
        blank(ClientId::BLANK).err(),
        Some(BridgeError::ReservedClient)
    );
    assert_eq!(
        from_model(&Document::blank(), ClientId::BLANK).err(),
        Some(BridgeError::ReservedClient)
    );
    assert_eq!(ClientId::BLANK, ClientId(0));
}
