//! Typed failures. Nothing in this crate panics on input; it says what was wrong.

/// Why a paragraph could not be broken.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BreakError {
    /// [`crate::LineWidths::per_line`] was given no widths.
    #[error("no line widths given")]
    NoLineWidths,
    /// A line width was zero or negative.
    #[error("line {line} has width {width}; widths must be positive")]
    NonPositiveWidth {
        /// The line.
        line: usize,
        /// Its width.
        width: i32,
    },
    /// The items do not end in a forced break (see [`crate::end_paragraph`]).
    #[error("the paragraph does not end in a forced break")]
    Unterminated,
    /// A glue item has negative stretch or shrink.
    #[error("glue at item {index} has negative stretch or shrink")]
    NegativeFlex {
        /// The item.
        index: usize,
    },
}

/// Why a pattern file could not be read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PatternError {
    /// A pattern has two digits in a row, no letters, or a character TeX would not accept.
    #[error("line {line}: malformed pattern `{pattern}`")]
    Malformed {
        /// One-based line in the source.
        line: usize,
        /// The offending token.
        pattern: String,
    },
    /// The same letters appear in two patterns.
    #[error("line {line}: duplicate pattern `{pattern}`")]
    Duplicate {
        /// One-based line in the source.
        line: usize,
        /// The offending token.
        pattern: String,
    },
    /// A `\patterns{` or `\hyphenation{` group is not closed.
    #[error("unclosed `\\{group}{{` group")]
    Unclosed {
        /// `patterns` or `hyphenation`.
        group: &'static str,
    },
    /// A control sequence other than `\patterns`, `\hyphenation`, `\message` or `\endinput`.
    #[error("line {line}: unsupported control sequence `\\{name}`")]
    Unsupported {
        /// One-based line in the source.
        line: usize,
        /// Its name.
        name: String,
    },
    /// The source has no `\patterns{` group.
    #[error("no \\patterns group")]
    NoPatterns,
    /// The `hyphenmins: typesetting:` header is missing or unreadable.
    #[error("no `hyphenmins: typesetting:` header")]
    NoHyphenMins,
    /// A left or right hyphen minimum of zero, which would allow a break before the word.
    #[error("hyphen minimums must be at least 1")]
    ZeroHyphenMin,
}
