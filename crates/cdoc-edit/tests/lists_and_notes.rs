//! Lists from the Bullets and Numbering buttons on a document that has no list definitions,
//! and footnotes whose ids the editor picks: both made in one undo step and reported as
//! created, under ids two replicas do not pick alike.

mod common;

use cdoc_edit::wire::{decode_request, decode_result, encode_request, encode_result, Request};
use cdoc_edit::{Command, Created, EditError};
use cdoc_model::builtin::ListKind;
use cdoc_model::numbering::NumberFormat;
use common::ROOM;
use common::{caret, check, editor, gap, paragraphs, peer, props, select};

fn labels(e: &cdoc_edit::Editor<'_>) -> Vec<Option<String>> {
    cdoc_layout::list_labels(e.model()).expect("labels")
}

#[test]
fn a_built_in_list_is_defined_on_demand_and_reused() {
    let mut e = editor(&paragraphs(&["one", "two", "three"]), 1);
    assert_eq!(e.model().numbering.definitions.len(), 0);
    let (a, b) = (gap(&e, 0, 0), gap(&e, 1, 1));
    select(&mut e, a, b);
    let bullets = e
        .apply(Command::SetNumberingKind {
            kind: ListKind::Bullet,
            level: 0,
        })
        .unwrap();
    let [Created::Numbering(num_id)] = bullets.created[..] else {
        panic!("one list created: {:?}", bullets.created)
    };
    check(&e);
    assert_eq!(props(&e, 0).numbering.map(|n| n.num_id), Some(num_id));
    assert_eq!(props(&e, 1).numbering.map(|n| n.num_id), Some(num_id));
    let def = e.model().numbering.definition_for(num_id).unwrap();
    assert_eq!(def.levels, ListKind::Bullet.levels());
    assert_eq!(
        labels(&e)[..2],
        [Some("\u{2022}".into()), Some("\u{2022}".into())]
    );
    let at = gap(&e, 2, 0);
    caret(&mut e, at);
    let again = e
        .apply(Command::SetNumberingKind {
            kind: ListKind::Bullet,
            level: 1,
        })
        .unwrap();
    assert_eq!(again.created, [], "the definition is reused");
    assert_eq!(
        props(&e, 2).numbering.map(|n| (n.num_id, n.level)),
        Some((num_id, 1))
    );
    let numbered = e
        .apply(Command::SetNumberingKind {
            kind: ListKind::UpperRoman,
            level: 0,
        })
        .unwrap();
    assert_eq!(numbered.created.len(), 1);
    assert_eq!(e.model().numbering.definitions.len(), 2);
    assert_eq!(labels(&e)[2], Some("I.".into()));
    check(&e);
    e.undo().unwrap();
    assert_eq!(e.model().numbering.definitions.len(), 1, "one undo step");
    assert_eq!(props(&e, 2).numbering.map(|n| n.level), Some(1));
    e.undo().unwrap();
    e.undo().unwrap();
    assert_eq!(e.model().numbering.definitions.len(), 0);
    assert_eq!(e.model().numbering.instances.len(), 0);
    assert_eq!(props(&e, 0).numbering, None);
    check(&e);
    assert!(matches!(
        e.apply(Command::SetNumberingKind {
            kind: ListKind::Decimal,
            level: 9
        }),
        Err(EditError::InvalidArgument(_))
    ));
}

#[test]
fn every_template_has_nine_levels_led_by_its_kind() {
    for kind in ListKind::ALL {
        let levels = kind.levels();
        assert_eq!(levels.len(), 9);
        let first = levels[0].format;
        assert_eq!(
            first == NumberFormat::Bullet,
            kind == ListKind::Bullet,
            "{kind:?}"
        );
        let lefts: Vec<i32> = levels
            .iter()
            .map(|l| l.indent.as_ref().and_then(|i| i.left).map_or(0, |v| v.0))
            .collect();
        assert!(
            lefts.windows(2).all(|w| w[0] < w[1]),
            "each level further in"
        );
    }
}

#[test]
fn the_editor_picks_footnote_ids_and_reports_them() {
    let mut a = editor(&paragraphs(&["note here"]), 1);
    let mut b = peer(&a, 2);
    let first = a.apply(Command::InsertFootnote(None)).unwrap();
    let second = a.apply(Command::InsertFootnote(None)).unwrap();
    let ids: Vec<_> = [&first, &second]
        .iter()
        .map(|x| match x.created[..] {
            [Created::Footnote(id)] => id,
            _ => panic!("one footnote created"),
        })
        .collect();
    assert_ne!(ids[0], ids[1]);
    assert_eq!(
        a.model().footnotes.iter().map(|f| f.id).collect::<Vec<_>>(),
        ids
    );
    let theirs = b.apply(Command::InsertFootnote(None)).unwrap();
    let [Created::Footnote(their_id)] = theirs.created[..] else {
        panic!("one footnote created")
    };
    assert!(!ids.contains(&their_id), "replicas pick apart");
    let mut c = peer(&b, 3);
    let mut d = peer(&b, 4);
    let pick = |e: &mut cdoc_edit::Editor<'_>| match e
        .apply(Command::InsertFootnote(None))
        .unwrap()
        .created[..]
    {
        [Created::Footnote(id)] => id,
        _ => panic!("one footnote created"),
    };
    assert_ne!(
        pick(&mut c),
        pick(&mut d),
        "the same state, two replicas, two ids"
    );
    a.apply_remote(&theirs.update, ROOM).unwrap();
    for u in [&first.update, &second.update] {
        b.apply_remote(u, ROOM).unwrap();
    }
    assert_eq!(a.model(), b.model());
    assert_eq!(a.model().footnotes.len(), 3);
    check(&a);
    a.undo().unwrap();
    assert_eq!(common::referenced(a.model()).footnotes.len(), 2);
}

#[test]
fn the_records_carry_the_new_commands_and_what_they_created() {
    for command in [
        Command::InsertFootnote(None),
        Command::SetNumberingKind {
            kind: ListKind::LowerLetter,
            level: 3,
        },
    ] {
        let req = Request::Command(command);
        assert_eq!(decode_request(&encode_request(&req)).unwrap(), req);
    }
    assert_eq!(
        encode_request(&Request::Command(Command::InsertFootnote(None))),
        [0x25]
    );
    assert!(
        decode_request(&[0x19, 6, 0]).is_err(),
        "list kind 6 is not a kind"
    );
    let mut e = editor(&paragraphs(&["x"]), 1);
    let applied = e.apply(Command::InsertFootnote(None)).unwrap();
    let created = applied.created.clone();
    let outcome = e
        .outcome(applied.update, applied.dirty_pages, applied.created)
        .unwrap();
    let bytes = encode_result(&outcome);
    let back = decode_result(&bytes).unwrap();
    assert_eq!(back.created, created);
    let [Created::Footnote(id)] = created[..] else {
        unreachable!()
    };
    let varint_len = |v: u64| (v.max(1).ilog2() / 7 + 1) as usize;
    let section = cdoc_crdt::codec::to_bytes(&outcome.section).unwrap().len();
    // The section, a complete empty selection text (2 bytes), no remote section (1 byte).
    let tail = varint_len(section as u64) + section + 3;
    let id_len = varint_len(u64::from(id.0));
    let kind_at = bytes.len() - tail - id_len - 1;
    assert_eq!(bytes[kind_at], 0, "kind 0 is a footnote");
    let mut bad = bytes.clone();
    bad[kind_at] = 2;
    assert!(decode_result(&bad).is_err(), "created kind 2 is not a kind");
}
