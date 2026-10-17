//! Small hand-built paragraphs for the unit-style tests.

#![allow(dead_code)]

use cdoc_para::{end_paragraph, HyphenKind, Item};

/// Words of the given widths separated by glue (100, stretch 50, shrink 30), ended as `\par`
/// ends a paragraph; word `at` is split in half by a hyphen (width 40) of `kind`.
pub fn words(widths: &[i32], hyphen: Option<(usize, HyphenKind)>) -> Vec<Item<()>> {
    let mut items = Vec::new();
    for (k, w) in widths.iter().enumerate() {
        if k > 0 {
            items.push(Item::glue(100, 50, 30));
        }
        match hyphen {
            Some((at, kind)) if at == k => {
                items.push(Item::boxed(w / 2, ()));
                items.push(Item::Hyphen { width: 40, kind });
                items.push(Item::boxed(w - w / 2, ()));
            }
            _ => items.push(Item::boxed(*w, ())),
        }
    }
    end_paragraph(&mut items);
    items
}

/// The break positions.
pub fn ends(b: &cdoc_para::Breaking) -> Vec<usize> {
    b.lines.iter().map(|l| l.end).collect()
}
