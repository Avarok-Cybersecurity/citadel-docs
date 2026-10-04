//! Errors. Anything the format or the caller's ports make impossible is an error; anything
//! merely not representable is a [`cdoc_model::Report`] entry instead.

use cdoc_model::ImageError;

/// Why reading or writing a `.docx` failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DocxError {
    /// The bytes are not a readable ZIP archive, or the archive could not be written.
    #[error("zip: {0}")]
    Zip(String),
    /// A part is not well-formed XML.
    #[error("{part}: {detail}")]
    Xml {
        /// The part name.
        part: String,
        /// What the parser said.
        detail: String,
    },
    /// A part the format requires is absent.
    #[error("missing part {0}")]
    MissingPart(String),
    /// A part is well-formed but means something the reader cannot follow.
    #[error("{part}: {detail}")]
    Invalid {
        /// The part name.
        part: String,
        /// What is wrong.
        detail: String,
    },
    /// The package is larger than the limits the caller set.
    #[error("package exceeds the {0} limit")]
    TooLarge(&'static str),
    /// The media port failed.
    #[error(transparent)]
    Media(#[from] ImageError),
}
