//! The pattern files shipped with the crate, each unmodified from hyph-utf8 (TeX Live) with
//! its licence header intact. Only files under MIT or all-permissive terms are included.

/// A language with shipped patterns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Language {
    /// American English: `hyph-en-us.tex` (Kuiken's `ushyphmax` with Knuth's exceptions;
    /// all-permissive licence).
    EnUs,
    /// British English: `hyph-en-gb.tex` (Wujastyk and Toal; MIT).
    EnGb,
    /// German, reformed spelling: `hyph-de-1996.tex` (Lemberg et al.; MIT).
    De1996,
    /// French: `hyph-fr.tex` (Flipo, Gaulle, Reutenauer; MIT).
    Fr,
    /// Spanish: `hyph-es.tex` (Bezos, `CervanTeX`; MIT).
    Es,
}

impl Language {
    /// Every shipped language.
    pub const ALL: [Language; 5] = [
        Language::EnUs,
        Language::EnGb,
        Language::De1996,
        Language::Fr,
        Language::Es,
    ];

    /// The BCP 47 style tag hyph-utf8 files the language under.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Language::EnUs => "en-us",
            Language::EnGb => "en-gb",
            Language::De1996 => "de-1996",
            Language::Fr => "fr",
            Language::Es => "es",
        }
    }

    /// The pattern file's text.
    #[must_use]
    pub const fn patterns(self) -> &'static str {
        match self {
            Language::EnUs => include_str!("../../patterns/hyph-en-us.tex"),
            Language::EnGb => include_str!("../../patterns/hyph-en-gb.tex"),
            Language::De1996 => include_str!("../../patterns/hyph-de-1996.tex"),
            Language::Fr => include_str!("../../patterns/hyph-fr.tex"),
            Language::Es => include_str!("../../patterns/hyph-es.tex"),
        }
    }
}
