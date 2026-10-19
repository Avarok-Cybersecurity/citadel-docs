//! The one failure of collection no peer can cause: its own snapshot not loading.

use super::{reload, GcError};
use crate::doc::Doc;
use crate::id::ClientId;

#[test]
fn a_collected_snapshot_that_does_not_load_is_an_error_not_a_panic() {
    let mut d = Doc::new(ClientId(1));
    d.insert_text(0, "abc").expect("inserts");
    let bytes = d.encode_snapshot();
    assert!(reload(&bytes, ClientId(1)).is_ok());
    let cut = &bytes[..bytes.len() - 1];
    assert!(matches!(
        reload(cut, ClientId(1)),
        Err(GcError::Snapshot(_))
    ));
}
