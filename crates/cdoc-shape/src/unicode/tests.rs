use super::*;

#[test]
fn categories_and_combining_classes_come_from_the_tables() {
    assert_eq!(Props::of(u32::from('a')).category(), Category::Other);
    assert_eq!(Props::of(0x20).category(), Category::Space);
    assert_eq!(Props::of(0x2003).category(), Category::Space);
    let acute = Props::of(0x0301);
    assert!(acute.is_mark() && acute.is_continuation());
    assert_eq!(acute.combining_class(), 230);
    assert_eq!(Props::of(0x0327).combining_class(), 202);
    assert_eq!(Props::of(0x0903).category(), Category::SpacingMark);
    assert_eq!(Props::of(0x20DD).category(), Category::EnclosingMark);
}

#[test]
fn joiners_are_ignorable_and_the_grapheme_joiner_is_hidden() {
    let zwj = Props::of(0x200D);
    assert!(zwj.is_ignorable() && !zwj.is_hidden());
    assert_eq!(zwj.category(), Category::Format);
    assert!(Props::of(0x034F).is_hidden());
    assert!(!Props::of(u32::from('a')).is_ignorable());
    // HarfBuzz leaves the Hangul fillers visible.
    assert!(!Props::of(0x3164).is_ignorable());
}

#[test]
fn scripts_fold_to_what_itemisation_needs() {
    assert_eq!(script('a'), CharScript::Latin);
    assert_eq!(script('ŝ'), CharScript::Latin);
    assert_eq!(script('α'), CharScript::Greek);
    assert_eq!(script('ж'), CharScript::Cyrillic);
    assert_eq!(script('1'), CharScript::Common);
    assert_eq!(script('\u{301}'), CharScript::Inherited);
    assert_eq!(script('ע'), CharScript::Other);
    assert_eq!(script('\u{E000}'), CharScript::Common);
}

#[test]
fn canonical_pairs_decompose_and_recompose() {
    assert_eq!(decompose(0xE9), Some((0x65, 0x301)));
    assert_eq!(compose(0x65, 0x301), Some(0xE9));
    assert_eq!(decompose(0x1E09), Some((0xE7, 0x301)));
    assert_eq!(decompose(0x212B), Some((0xC5, 0)));
    assert_eq!(compose(0x65, 0x65), None);
    // Listed in CompositionExclusions.txt: decomposes, never recomposes.
    assert_eq!(decompose(0x2ADC), Some((0x2ADD, 0x338)));
    assert_eq!(compose(0x2ADD, 0x338), None);
    // A non-starter decomposition: HarfBuzz, unlike NFC, still recomposes it.
    assert_eq!(compose(0x308, 0x301), Some(0x344));
}

#[test]
fn spaces_have_fallback_widths_and_letters_none() {
    assert_eq!(space_width(0x2003), Some(SpaceWidth::EmDivided(1)));
    assert_eq!(space_width(0x2009), Some(SpaceWidth::EmDivided(5)));
    assert_eq!(space_width(0x2007), Some(SpaceWidth::Figure));
    assert_eq!(space_width(u32::from('a')), None);
}

#[test]
fn emoji_are_extended_pictographic() {
    assert!(is_extended_pictographic(0x1F600));
    assert!(is_extended_pictographic(0x00A9));
    assert!(!is_extended_pictographic(u32::from('a')));
}
