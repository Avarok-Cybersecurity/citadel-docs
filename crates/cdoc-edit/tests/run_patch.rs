//! Character patches from the font group: a patch sets the fields it names, `null` clears
//! one back to the style's (no highlight, the style's colour), Clear Formatting clears them
//! all, and the record keeps absent and `null` apart.

mod common;

use cdoc_edit::wire::{decode_request, encode_request, Request};
use cdoc_edit::Command;
use cdoc_model::{Color, FontSize, Ligatures, RunPatch, RunProperties};
use common::{check, editor, gap, paragraphs, runs, select};

const RED: Color = Color::rgb(0xC0, 0, 0);
const YELLOW: Color = Color::rgb(0xFF, 0xFF, 0);

fn styled() -> cdoc_edit::Editor<'static> {
    let mut e = editor(&paragraphs(&["marked text"]), 1);
    let (a, b) = (gap(&e, 0, 0), gap(&e, 0, 6));
    select(&mut e, a, b);
    e.apply(Command::SetRunProperty(RunPatch {
        color: Some(Some(RED)),
        highlight: Some(Some(YELLOW)),
        bold: Some(Some(true)),
        kern: Some(Some(FontSize(16))),
        ligatures: Some(Some(Ligatures::None)),
        lang: Some(Some("de-DE".to_owned())),
        ..RunPatch::default()
    }))
    .unwrap();
    e
}

#[test]
fn a_patch_from_properties_sets_kerning_ligatures_and_language() {
    let p = RunProperties {
        kern: Some(FontSize(16)),
        ligatures: Some(Ligatures::None),
        lang: Some("de-DE".to_owned()),
        ..RunProperties::none()
    };
    assert_eq!(RunPatch::from(&p).sets(), p);
    let e = styled();
    let r = &runs(&e, 0)[0].1;
    assert_eq!(
        (r.kern, r.ligatures, r.lang.as_deref()),
        (p.kern, p.ligatures, Some("de-DE"))
    );
}

#[test]
fn null_clears_one_field_and_keeps_the_rest() {
    let mut e = styled();
    e.apply(Command::SetRunProperty(RunPatch {
        highlight: Some(None),
        ..RunPatch::default()
    }))
    .unwrap();
    check(&e);
    let r = &runs(&e, 0)[0].1;
    assert_eq!(
        (r.color, r.highlight, r.bold),
        (Some(RED), None, Some(true))
    );
    e.undo().unwrap();
    assert_eq!(runs(&e, 0)[0].1.highlight, Some(YELLOW));
}

#[test]
fn clear_formatting_clears_every_direct_property() {
    let mut e = styled();
    e.apply(Command::SetRunProperty(RunPatch::clear_all()))
        .unwrap();
    check(&e);
    assert_eq!(
        runs(&e, 0),
        [("marked text".to_owned(), RunProperties::none())]
    );
}

#[test]
fn at_a_caret_the_patch_applies_to_the_next_typed_text() {
    let mut e = styled();
    let at = gap(&e, 0, 3);
    common::caret(&mut e, at);
    e.apply(Command::SetRunProperty(RunPatch {
        color: Some(None),
        ..RunPatch::default()
    }))
    .unwrap();
    e.apply(Command::InsertText("Z".into())).unwrap();
    let z = runs(&e, 0).into_iter().find(|(t, _)| t == "Z").unwrap().1;
    assert_eq!((z.color, z.highlight), (None, Some(YELLOW)));
    assert!(e
        .apply(Command::SetRunProperty(RunPatch::default()))
        .is_err());
}

#[test]
fn the_record_keeps_absent_and_null_apart_and_reads_version_1() {
    let patch = RunPatch {
        color: Some(None),
        size: Some(Some(cdoc_model::FontSize(28))),
        ..RunPatch::default()
    };
    let req = Request::Command(Command::SetRunProperty(patch));
    assert_eq!(decode_request(&encode_request(&req)).unwrap(), req);
    let v1 = RunProperties {
        italic: Some(true),
        ..RunProperties::none()
    };
    let value = cdoc_crdt::codec::to_bytes(&v1).unwrap();
    let mut bytes = vec![0x08, u8::try_from(value.len()).unwrap()];
    bytes.extend(value);
    assert_eq!(
        decode_request(&bytes).unwrap(),
        Request::Command(Command::SetRunProperty(RunPatch::from(&v1)))
    );
}
