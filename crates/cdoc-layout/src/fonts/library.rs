//! An in-memory font port: faces registered by family and style, aliases from the names
//! documents use to the families supplied, an explicit fallback family, and the hyphenation
//! patterns registered by language (compiled once, when registered).
//!
//! The substitution policy lives here once, for the WASM facade and the tests alike: the exact
//! face; else the alias's face; else the fallback family's; within a family, the nearest style
//! (dropping italic, then bold).

use super::{FaceSource, FontError, FontPort};
use crate::ir::FaceKey;
use cdoc_para::{Hyphenator, Language, PatternError};
use std::collections::BTreeMap;

/// Faces held in memory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontLibrary {
    faces: BTreeMap<FaceKey, (String, Vec<u8>)>,
    aliases: BTreeMap<String, String>,
    fallback: String,
    patterns: BTreeMap<Language, Hyphenator>,
}

fn fold(family: &str) -> String {
    family.trim().to_lowercase()
}

impl FontLibrary {
    /// An empty library whose last resort is `fallback` (which must be added before use).
    #[must_use]
    pub fn new(fallback: &str) -> Self {
        Self {
            faces: BTreeMap::new(),
            aliases: BTreeMap::new(),
            fallback: fold(fallback),
            patterns: BTreeMap::new(),
        }
    }

    /// Registers a face. Family names match case-insensitively.
    pub fn add(&mut self, family: &str, bold: bool, italic: bool, bytes: Vec<u8>) {
        let key = FaceKey {
            family: fold(family),
            bold,
            italic,
        };
        self.faces.insert(key, (family.trim().to_owned(), bytes));
    }

    /// Serves `target` whenever `name` is asked for (e.g. `Calibri` → `Carlito`).
    pub fn alias(&mut self, name: &str, target: &str) {
        self.aliases.insert(fold(name), fold(target));
    }

    /// Compiles and registers `language`'s hyph-utf8 pattern file (the text of
    /// `cdoc-para/patterns/<Language::file>`); a second file for a language replaces the first.
    pub fn add_patterns(&mut self, language: Language, source: &str) -> Result<(), PatternError> {
        self.patterns
            .insert(language, Hyphenator::from_file(source)?);
        Ok(())
    }

    fn nearest(
        &self,
        family: &str,
        bold: bool,
        italic: bool,
    ) -> Option<(&FaceKey, &(String, Vec<u8>))> {
        [
            (bold, italic),
            (bold, false),
            (false, italic),
            (false, false),
        ]
        .into_iter()
        .find_map(|(b, i)| {
            self.faces.get_key_value(&FaceKey {
                family: family.to_owned(),
                bold: b,
                italic: i,
            })
        })
    }
}

impl FontPort for FontLibrary {
    fn face(&self, family: &str, bold: bool, italic: bool) -> Result<FaceSource<'_>, FontError> {
        let asked = fold(family);
        let candidates = [
            Some(asked.clone()),
            self.aliases.get(&asked).cloned(),
            Some(self.fallback.clone()),
        ];
        candidates
            .into_iter()
            .flatten()
            .find_map(|f| self.nearest(&f, bold, italic))
            .map(|(key, (name, bytes))| FaceSource {
                family: name,
                bold: key.bold,
                italic: key.italic,
                bytes,
            })
            .ok_or_else(|| {
                FontError::Missing(FaceKey {
                    family: family.to_owned(),
                    bold,
                    italic,
                })
            })
    }

    fn hyphenator(&self, language: Language) -> Option<&Hyphenator> {
        self.patterns.get(&language)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn library() -> FontLibrary {
        let mut lib = FontLibrary::new("Carlito");
        lib.add("Carlito", false, false, vec![1]);
        lib.add("Carlito", true, false, vec![2]);
        lib.add("Mono", false, false, vec![3]);
        lib.alias("Calibri", "Carlito");
        lib.alias("Consolas", "Mono");
        lib
    }

    #[test]
    fn exact_alias_and_fallback_are_tried_in_that_order() {
        let lib = library();
        assert_eq!(lib.face("MONO", false, false).expect("exact").bytes, [3]);
        let mono = lib.face("Consolas", false, false).expect("alias");
        assert_eq!(
            (mono.family, mono.bytes),
            ("Mono", &[3][..]),
            "the alias, not the fallback"
        );
        let aliased = lib.face("Calibri", true, false).expect("alias");
        assert_eq!((aliased.family, aliased.bytes), ("Carlito", &[2][..]));
        assert_eq!(
            lib.face("Georgia", false, false).expect("fallback").bytes,
            [1]
        );
    }

    #[test]
    fn a_missing_style_falls_to_the_nearest_and_says_so() {
        let lib = library();
        let face = lib.face("Carlito", true, true).expect("nearest");
        assert_eq!((face.bold, face.italic), (true, false));
    }

    #[test]
    fn an_empty_library_refuses_by_name() {
        let lib = FontLibrary::new("Nothing");
        assert!(
            matches!(lib.face("Calibri", false, false), Err(FontError::Missing(k)) if k.family == "Calibri")
        );
    }
}
