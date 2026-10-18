//! Page setup from the Layout ribbon: a patch on every section the selection touches, as one
//! undo step, refused when it would make a page the model rejects.

mod common;

use cdoc_edit::{Command, EditError};
use cdoc_model::{Columns, Length, Margins, Orientation, PageSetupPatch, PageSize};
use common::{check, editor, paragraphs};
use proptest::prelude::*;

fn pages(e: &cdoc_edit::Editor<'_>) -> Vec<cdoc_model::PageSetup> {
    e.model()
        .sections
        .iter()
        .map(|s| s.properties.page.clone())
        .collect()
}

#[test]
fn landscape_turns_the_page_and_one_undo_turns_it_back() {
    let mut e = editor(&paragraphs(&["one", "two"]), 1);
    let before = pages(&e);
    let (w, h) = e.page_size(0).unwrap();
    e.apply(Command::SetSectionProperty(PageSetupPatch {
        orientation: Some(Orientation::Landscape),
        columns: Some(Columns {
            count: 2,
            ..Columns::ONE
        }),
        ..PageSetupPatch::none()
    }))
    .unwrap();
    check(&e);
    let after = pages(&e);
    assert_eq!(after[0].size, before[0].size.rotated());
    assert_eq!(after[0].columns.count, 2);
    assert_eq!(e.page_size(0).unwrap(), (h, w), "the layout follows");
    e.undo().unwrap();
    assert_eq!(pages(&e), before);
    e.redo().unwrap();
    assert_eq!(pages(&e), after);
    check(&e);
}

#[test]
fn every_section_the_selection_touches_changes_and_no_other() {
    let d = cdoc_model::test_support::fixtures::kitchen_sink();
    assert!(d.sections.len() >= 2);
    let mut e = editor(&d, 1);
    let before = pages(&e);
    let patch = PageSetupPatch {
        size: Some(PageSize::LETTER_PORTRAIT),
        ..PageSetupPatch::none()
    };
    e.apply(Command::SetSectionProperty(patch)).unwrap();
    let after = pages(&e);
    assert_eq!(after[0].size, PageSize::LETTER_PORTRAIT);
    assert_eq!(after[1..], before[1..], "the caret is in the first section");
    e.apply(Command::SelectAll).unwrap();
    e.apply(Command::SetSectionProperty(patch)).unwrap();
    assert!(pages(&e)
        .iter()
        .all(|p| p.size == PageSize::LETTER_PORTRAIT));
    check(&e);
}

#[test]
fn a_patch_the_model_would_reject_changes_nothing() {
    let mut e = editor(&paragraphs(&["one"]), 1);
    let before = e.model().clone();
    let zero_columns = PageSetupPatch {
        columns: Some(Columns {
            count: 0,
            ..Columns::ONE
        }),
        ..PageSetupPatch::none()
    };
    let negative = PageSetupPatch {
        margins: Some(Margins {
            top: Length(-1),
            ..Margins::NORMAL
        }),
        ..PageSetupPatch::none()
    };
    for patch in [zero_columns, negative, PageSetupPatch::none()] {
        let r = e.apply(Command::SetSectionProperty(patch));
        assert!(matches!(r, Err(EditError::InvalidArgument(_))), "{r:?}");
    }
    assert_eq!(e.model(), &before);
    assert!(e.undo().unwrap().is_none());
}

fn patch() -> impl Strategy<Value = PageSetupPatch> {
    let size = (1_000i32..100_000, 1_000i32..100_000).prop_map(|(w, h)| PageSize {
        width: Length(w),
        height: Length(h),
        orientation: if w > h {
            Orientation::Landscape
        } else {
            Orientation::Portrait
        },
    });
    let orientation = prop_oneof![Just(Orientation::Portrait), Just(Orientation::Landscape)];
    let margins = (0i32..5_000).prop_map(|m| Margins {
        top: Length(m),
        left: Length(m),
        ..Margins::NORMAL
    });
    let columns = (1u16..4).prop_map(|count| Columns {
        count,
        ..Columns::ONE
    });
    (
        proptest::option::of(size),
        proptest::option::of(orientation),
        proptest::option::of(margins),
        proptest::option::of(columns),
    )
        .prop_map(|(size, orientation, margins, columns)| PageSetupPatch {
            size,
            orientation,
            margins,
            columns,
        })
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 32, ..ProptestConfig::default() })]

    /// Any patch: undo gives the model back exactly, redo the patched one.
    #[test]
    fn page_setup_undo_and_redo_are_exact(p in patch()) {
        let mut e = editor(&cdoc_model::test_support::fixtures::kitchen_sink(), 1);
        let before = e.model().clone();
        if e.apply(Command::SetSectionProperty(p)).is_err() {
            prop_assert!(p.is_none());
            return Ok(());
        }
        let after = e.model().clone();
        prop_assert_eq!(&after.sections[0].properties.page, &p.apply(&before.sections[0].properties.page));
        e.undo().unwrap();
        prop_assert_eq!(e.model(), &before);
        e.redo().unwrap();
        prop_assert_eq!(e.model(), &after);
        check(&e);
    }
}
