//! The line breaker's parameters, named as TeX names them. There is no `Default`: a caller
//! picks [`Params::tex_plain`] or [`Params::word_like`] (or builds one field by field), so the
//! choice is visible where it is made.

/// TeX's paragraph parameters. Badness and penalty values are TeX's (badness 100 is a line
/// stretched by its full stretchability); `emergency_stretch` is a width in the caller's unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Params {
    /// `\pretolerance`: the badness limit of the first pass, which tries no pattern
    /// hyphenation. Negative skips the first pass.
    pub pretolerance: i32,
    /// `\tolerance`: the badness limit of the second and emergency passes.
    pub tolerance: i32,
    /// `\linepenalty`: added to every line's badness before squaring; favours fewer lines.
    pub line_penalty: i32,
    /// `\hyphenpenalty`: the cost of a break at a [`crate::HyphenKind::Pattern`] or
    /// [`crate::HyphenKind::Soft`] hyphen.
    pub hyphen_penalty: i32,
    /// `\exhyphenpenalty`: the cost of a break after a hyphen already in the text.
    pub ex_hyphen_penalty: i32,
    /// `\adjdemerits`: added when adjacent lines' fitness classes are not adjacent.
    pub adj_demerits: i64,
    /// `\doublehyphendemerits`: added when two consecutive lines end in hyphens.
    pub double_hyphen_demerits: i64,
    /// `\finalhyphendemerits`: added when the second-last line ends in a hyphen.
    pub final_hyphen_demerits: i64,
    /// `\looseness`: how many lines longer (positive) or shorter (negative) than optimal the
    /// paragraph should be, when that is feasible.
    pub looseness: i32,
    /// `\emergencystretch`: stretch added to every line in a third pass when the second finds
    /// no feasible breaks. Zero disables the third pass.
    pub emergency_stretch: i32,
}

impl Params {
    /// Plain TeX's values (`plain.tex`): pretolerance 100, tolerance 200, line penalty 10,
    /// hyphen penalties 50, adjacent, double and final hyphen demerits 10000, 10000 and 5000,
    /// no looseness, no emergency stretch.
    #[must_use]
    pub const fn tex_plain() -> Self {
        Self {
            pretolerance: 100,
            tolerance: 200,
            line_penalty: 10,
            hyphen_penalty: 50,
            ex_hyphen_penalty: 50,
            adj_demerits: 10_000,
            double_hyphen_demerits: 10_000,
            final_hyphen_demerits: 5_000,
            looseness: 0,
            emergency_stretch: 0,
        }
    }

    /// The look of a word processor's justified text: like Word, it never refuses a line for
    /// being loose (tolerance 10000, the badness ceiling, so only overfull lines are
    /// infeasible), and it hyphenates reluctantly (hyphen penalty 200). Lines are still chosen
    /// for the whole paragraph, so it spaces more evenly than Word's line-at-a-time setter.
    #[must_use]
    pub const fn word_like() -> Self {
        Self {
            pretolerance: 200,
            tolerance: 10_000,
            line_penalty: 10,
            hyphen_penalty: 200,
            ex_hyphen_penalty: 50,
            adj_demerits: 10_000,
            double_hyphen_demerits: 10_000,
            final_hyphen_demerits: 5_000,
            looseness: 0,
            emergency_stretch: 0,
        }
    }
}
