//! Ids of the built-in styles, so callers never spell them as literals.

use crate::ids::StyleId;

macro_rules! builtin {
    ($(#[$doc:meta])* $name:ident = $id:literal) => {
        $(#[$doc])*
        #[must_use]
        pub fn $name() -> StyleId {
            StyleId::new($id)
        }
    };
}

builtin!(
    /// Body text.
    normal = "Normal"
);
builtin!(
    /// The document title.
    title = "Title"
);
builtin!(
    /// A block quotation.
    quote = "Quote"
);
builtin!(
    /// A list paragraph.
    list_paragraph = "ListParagraph"
);
builtin!(
    /// Footnote body text.
    footnote_text = "FootnoteText"
);
builtin!(
    /// Header text.
    header = "Header"
);
builtin!(
    /// Footer text.
    footer = "Footer"
);
builtin!(
    /// Hyperlink text (character style).
    hyperlink = "Hyperlink"
);

/// The heading style for a level `1..=6`, or `None` outside that range.
#[must_use]
pub fn heading(level: u8) -> Option<StyleId> {
    (1..=6)
        .contains(&level)
        .then(|| StyleId::new(format!("Heading{level}")))
}
