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

    /// The shipped patterns for a BCP 47 language tag, matched case-insensitively on the
    /// primary language and, for English, the region: `en-US` and a bare `en` are American;
    /// `en-GB` and the other Commonwealth regions (`AU`, `NZ`, `IE`, `ZA`, `IN`) British; any
    /// `de` is the reformed 1996 spelling; any `fr` or `es` the one set each has. `None` for a
    /// language without patterns here.
    #[must_use]
    pub fn from_bcp47(tag: &str) -> Option<Language> {
        let mut parts = tag.split(['-', '_']).map(str::to_ascii_lowercase);
        let primary = parts.next()?;
        let region = parts.find(|p| p.len() == 2);
        match primary.as_str() {
            "en" => Some(match region.as_deref() {
                Some("gb" | "au" | "nz" | "ie" | "za" | "in") => Language::EnGb,
                _ => Language::EnUs,
            }),
            "de" => Some(Language::De1996),
            "fr" => Some(Language::Fr),
            "es" => Some(Language::Es),
            _ => None,
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

#[cfg(test)]
mod tests {
    use super::Language;

    #[test]
    fn bcp47_tags_pick_the_pattern_set() {
        let cases = [
            ("en-US", Some(Language::EnUs)),
            ("en", Some(Language::EnUs)),
            ("EN-gb", Some(Language::EnGb)),
            ("en-Latn-AU", Some(Language::EnGb)),
            ("de-CH", Some(Language::De1996)),
            ("fr-CA", Some(Language::Fr)),
            ("es-419", Some(Language::Es)),
            ("nl-NL", None),
            ("", None),
        ];
        for (tag, want) in cases {
            assert_eq!(Language::from_bcp47(tag), want, "{tag}");
        }
    }
}
