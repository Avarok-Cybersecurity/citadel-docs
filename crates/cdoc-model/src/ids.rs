//! Identifiers. Each is a newtype so a style id cannot be passed where a numbering id is
//! expected. String ids are minted by the editor (through its randomness port); integer ids are
//! document-local counters, as in `.docx`.

use serde::{Deserialize, Serialize};

macro_rules! string_id {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub String);

        impl $name {
            /// Wrap a string.
            #[must_use]
            pub fn new(id: impl Into<String>) -> Self {
                Self(id.into())
            }

            /// The id as text.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

macro_rules! int_id {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(pub u32);
    };
}

string_id!(
    /// A named style, such as `Normal` or `Heading1`. The id is the key; the display name is
    /// separate.
    StyleId
);
string_id!(
    /// An image, addressed by content id in the workspace's content store.
    ContentId
);
string_id!(
    /// A comment thread or reply.
    CommentId
);
string_id!(
    /// A form or signature field.
    FieldId
);
string_id!(
    /// A tracked change.
    RevisionId
);

int_id!(
    /// A numbering instance a paragraph refers to.
    NumId
);
int_id!(
    /// An abstract numbering definition an instance refers to.
    AbstractNumId
);
int_id!(
    /// A footnote.
    FootnoteId
);
