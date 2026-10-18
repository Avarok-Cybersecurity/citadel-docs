//! Paragraph patches from the ribbons: a patch sets only the fields it names, inside `spacing`
//! and `indent` too; `null` clears one; and the record carries the difference between absent
//! and `null`.

mod common;

use cdoc_edit::wire::{decode_request, encode_request, Request};
use cdoc_edit::Command;
use cdoc_model::{
    Alignment, FirstLine, Indent, IndentPatch, Length, LineSpacing, ParagraphPatch,
    ParagraphProperties, Spacing, SpacingPatch,
};
use common::{check, editor, paragraphs, props};

fn styled() -> cdoc_model::Document {
    let mut d = paragraphs(&["text"]);
    let cdoc_model::Block::Paragraph(p) = &mut d.sections[0].blocks[0] else {
        unreachable!()
    };
    p.properties.spacing = Some(Spacing {
        before: Some(Length(100)),
        after: Some(Length(200)),
        line: Some(LineSpacing::Multiple { lines: 240 }),
    });
    p.properties.indent = Some(Indent {
        left: Some(Length(720)),
        right: Some(Length(360)),
        first_line: Some(FirstLine::Hanging { by: Length(180) }),
    });
    d
}

fn patched(patch: ParagraphPatch) -> ParagraphProperties {
    let mut e = editor(&styled(), 1);
    e.apply(Command::SetParagraphProperty(patch)).unwrap();
    check(&e);
    props(&e, 0)
}

#[test]
fn each_spacing_field_is_set_alone() {
    let line = LineSpacing::Exact {
        height: Length(1400),
    };
    let fields: [(SpacingPatch, Spacing); 3] = [
        (
            SpacingPatch {
                before: Some(Some(Length(1))),
                ..SpacingPatch::default()
            },
            Spacing {
                before: Some(Length(1)),
                after: Some(Length(200)),
                line: Some(LineSpacing::Multiple { lines: 240 }),
            },
        ),
        (
            SpacingPatch {
                after: Some(Some(Length(2))),
                ..SpacingPatch::default()
            },
            Spacing {
                before: Some(Length(100)),
                after: Some(Length(2)),
                line: Some(LineSpacing::Multiple { lines: 240 }),
            },
        ),
        (
            SpacingPatch {
                line: Some(Some(line)),
                ..SpacingPatch::default()
            },
            Spacing {
                before: Some(Length(100)),
                after: Some(Length(200)),
                line: Some(line),
            },
        ),
    ];
    for (patch, want) in fields {
        let got = patched(ParagraphPatch {
            spacing: Some(Some(patch)),
            ..ParagraphPatch::default()
        });
        assert_eq!(got.spacing, Some(want));
        assert_eq!(got.indent, Some(styled_indent()), "indent untouched");
    }
}

fn styled_indent() -> Indent {
    Indent {
        left: Some(Length(720)),
        right: Some(Length(360)),
        first_line: Some(FirstLine::Hanging { by: Length(180) }),
    }
}

#[test]
fn each_indent_field_is_set_or_cleared_alone() {
    let got = patched(ParagraphPatch {
        indent: Some(Some(IndentPatch {
            first_line: Some(Some(FirstLine::Indent { by: Length(360) })),
            ..IndentPatch::default()
        })),
        ..ParagraphPatch::default()
    });
    let indent = got.indent.unwrap();
    assert_eq!(
        indent.first_line,
        Some(FirstLine::Indent { by: Length(360) })
    );
    assert_eq!(
        (indent.left, indent.right),
        (Some(Length(720)), Some(Length(360)))
    );
    for (patch, cleared) in [
        (
            IndentPatch {
                left: Some(None),
                ..IndentPatch::default()
            },
            0,
        ),
        (
            IndentPatch {
                right: Some(None),
                ..IndentPatch::default()
            },
            1,
        ),
        (
            IndentPatch {
                first_line: Some(None),
                ..IndentPatch::default()
            },
            2,
        ),
    ] {
        let i = patched(ParagraphPatch {
            indent: Some(Some(patch)),
            ..ParagraphPatch::default()
        })
        .indent
        .unwrap();
        let present = [i.left.is_some(), i.right.is_some(), i.first_line.is_some()];
        let mut want = [true; 3];
        want[cleared] = false;
        assert_eq!(present, want);
    }
}

#[test]
fn null_clears_a_field_and_clearing_every_field_drops_the_group() {
    let got = patched(ParagraphPatch {
        spacing: Some(Some(SpacingPatch {
            before: Some(None),
            after: Some(None),
            line: Some(None),
        })),
        alignment: Some(Some(Alignment::Center)),
        ..ParagraphPatch::default()
    });
    assert_eq!(got.spacing, None);
    assert_eq!(got.alignment, Some(Alignment::Center));
    let got = patched(ParagraphPatch {
        indent: Some(None),
        ..ParagraphPatch::default()
    });
    assert_eq!(got.indent, None);
    assert!(got.spacing.is_some());
}

#[test]
fn the_record_keeps_absent_and_null_apart() {
    let patch = ParagraphPatch {
        spacing: Some(Some(SpacingPatch {
            before: Some(None),
            line: Some(Some(LineSpacing::AtLeast {
                height: Length(300),
            })),
            ..SpacingPatch::default()
        })),
        alignment: Some(None),
        ..ParagraphPatch::default()
    };
    let req = Request::Command(Command::SetParagraphProperty(patch));
    assert_eq!(decode_request(&encode_request(&req)).unwrap(), req);
}

#[test]
fn a_version_1_properties_payload_reads_as_the_patch_that_sets_its_fields() {
    let v1 = ParagraphProperties {
        alignment: Some(Alignment::Right),
        spacing: Some(Spacing {
            before: None,
            after: Some(Length(50)),
            line: None,
        }),
        ..ParagraphProperties::none()
    };
    let mut bytes = vec![0x09];
    let value = cdoc_crdt::codec::to_bytes(&v1).unwrap();
    bytes.push(u8::try_from(value.len()).unwrap());
    bytes.extend(value);
    let Request::Command(Command::SetParagraphProperty(patch)) = decode_request(&bytes).unwrap()
    else {
        panic!("not a paragraph patch")
    };
    assert_eq!(patch, ParagraphPatch::from(&v1));
    let got = patched(patch);
    assert_eq!(got.alignment, Some(Alignment::Right));
    assert_eq!(
        got.spacing.unwrap().before,
        Some(Length(100)),
        "a v1 payload no longer clears before"
    );
}
