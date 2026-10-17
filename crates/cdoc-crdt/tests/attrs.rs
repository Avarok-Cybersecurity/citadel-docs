//! Attributes and block properties: multi-value registers with a deterministic winner.

use cdoc_crdt::AtomKind;
use cdoc_crdt::{from_model, to_model, Atom, ClientId, Doc, DocAttr, EditError};
use cdoc_model::{Alignment, Block, Document, DocumentMeta, ParagraphProperties};

fn base() -> Doc {
    from_model(&Document::blank(), ClientId(100)).unwrap()
}

fn replica(of: &Doc, client: u64) -> Doc {
    Doc::decode_snapshot(&of.encode_snapshot(), ClientId(client)).unwrap()
}

fn send(from: &Doc, to: &mut Doc) {
    to.apply_update(&from.encode_diff(&to.state_vector()))
        .unwrap();
}

fn title(t: &str) -> DocAttr {
    DocAttr::Meta(DocumentMeta {
        title: Some(t.to_owned()),
        language: None,
    })
}

fn title_of(d: &Doc) -> Option<String> {
    to_model(d).unwrap().meta.title
}

fn aligned(a: Alignment) -> Atom {
    Atom::Paragraph(ParagraphProperties {
        alignment: Some(a),
        ..ParagraphProperties::none()
    })
}

fn alignment_of(d: &Doc) -> Option<Alignment> {
    let m = to_model(d).unwrap();
    let Block::Paragraph(p) = &m.sections[0].blocks[0] else {
        unreachable!()
    };
    p.properties.alignment
}

#[test]
fn a_write_that_saw_another_overrides_it_whatever_the_clients() {
    let mut high = replica(&base(), 9);
    let mut low = replica(&base(), 1);
    high.set_attr(&title("first")).unwrap();
    send(&high, &mut low);
    low.set_attr(&title("second")).unwrap();
    send(&low, &mut high);
    assert_eq!(title_of(&high).as_deref(), Some("second"));
    assert_eq!(title_of(&low).as_deref(), Some("second"));
}

#[test]
fn concurrent_writes_resolve_to_the_greatest_client_everywhere() {
    let mut a = replica(&base(), 1);
    let mut b = replica(&base(), 2);
    a.set_attr(&title("from a")).unwrap();
    b.set_attr(&title("from b")).unwrap();
    send(&a, &mut b);
    send(&b, &mut a);
    assert_eq!(title_of(&a).as_deref(), Some("from b"));
    assert_eq!(title_of(&b).as_deref(), Some("from b"));
}

#[test]
fn the_winner_does_not_depend_on_arrival_order() {
    // w3 (client 3), then w1 (client 1) after seeing w3; w2 (client 2) concurrent with both.
    let b = base();
    let mut c3 = replica(&b, 3);
    let mut c1 = replica(&b, 1);
    let mut c2 = replica(&b, 2);
    c3.set_attr(&title("w3")).unwrap();
    send(&c3, &mut c1);
    c1.set_attr(&title("w1")).unwrap();
    c2.set_attr(&title("w2")).unwrap();
    let u3 = c3.encode_diff(&b.state_vector());
    let u1 = c1.encode_diff(&b.state_vector());
    let u2 = c2.encode_diff(&b.state_vector());
    let orders: [[&Vec<u8>; 3]; 3] = [[&u3, &u2, &u1], [&u2, &u1, &u3], [&u1, &u3, &u2]];
    for order in orders {
        let mut r = replica(&b, 7);
        for u in order {
            r.apply_update(u).unwrap();
        }
        assert!(!r.has_pending());
        // w3 is overwritten by w1; w1 and w2 are concurrent; client 2 > client 1.
        assert_eq!(title_of(&r).as_deref(), Some("w2"));
    }
}

#[test]
fn block_properties_change_and_converge() {
    let mut a = replica(&base(), 1);
    let mut b = replica(&base(), 2);
    a.set_block(1, &aligned(Alignment::Center)).unwrap();
    assert_eq!(alignment_of(&a), Some(Alignment::Center));
    b.set_block(1, &aligned(Alignment::Right)).unwrap();
    send(&a, &mut b);
    send(&b, &mut a);
    assert_eq!(alignment_of(&a), Some(Alignment::Right));
    assert_eq!(alignment_of(&b), Some(Alignment::Right));
    assert_eq!(a.encode_snapshot(), b.encode_snapshot());
}

#[test]
fn block_properties_must_match_the_block() {
    let mut d = base();
    d.insert_text(2, "x").unwrap();
    assert_eq!(
        d.set_block(2, &aligned(Alignment::Left)),
        Err(EditError::NotStructural { index: 2 })
    );
    assert_eq!(
        d.set_block(0, &aligned(Alignment::Left)),
        Err(EditError::KindMismatch {
            index: 0,
            expected: AtomKind::Paragraph,
            found: AtomKind::Section
        })
    );
}

#[test]
fn a_fresh_replica_has_no_attributes_until_it_syncs() {
    let mut d = Doc::new(ClientId(4));
    assert_eq!(to_model(&d), Err(cdoc_crdt::BridgeError::NoSection));
    send(&base(), &mut d);
    assert_eq!(to_model(&d).unwrap(), Document::blank());
}
