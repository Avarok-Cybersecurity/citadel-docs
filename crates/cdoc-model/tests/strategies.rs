//! The test strategies generate values in the resolution formats store, so exporters can write
//! them without rounding.

use cdoc_model::test_support::strategies::{
    document, length, non_negative_length, positive_length, whole_points,
};
use cdoc_model::Length;
use proptest::prelude::*;

proptest! {
    #[test]
    fn lengths_are_whole_twips(a in length(), b in positive_length(), c in non_negative_length()) {
        for l in [a, b, c] {
            prop_assert_eq!(l.0 % Length::PER_TWIP, 0);
        }
        prop_assert!(b.is_positive() && c.is_non_negative());
    }

    #[test]
    fn whole_points_are_whole_points(l in whole_points()) {
        prop_assert_eq!(l.0 % Length::PER_POINT, 0);
        prop_assert!(l.is_non_negative());
    }

    #[test]
    fn document_margins_and_spacing_are_unsigned_twips(doc in document()) {
        for section in &doc.sections {
            for m in section.properties.page.margins.all() {
                prop_assert!(m.is_non_negative() && m.0 % Length::PER_TWIP == 0);
            }
        }
        for p in doc.paragraphs() {
            if let Some(s) = &p.properties.spacing {
                for v in [s.before, s.after].into_iter().flatten() {
                    prop_assert!(v.is_non_negative() && v.0 % Length::PER_TWIP == 0);
                }
            }
        }
    }
}
