//! Test support (feature `test-support`): the shipped default faces, compiled in, registered
//! the way the UI registers them from `fonts/manifest.json` (Carlito, with Calibri and Calibri
//! Light served by it). Never enabled by production code.

use crate::FontLibrary;

/// The family the default set falls back to.
pub const FALLBACK: &str = "Carlito";

/// The aliases `fonts/manifest.json` declares; `cdoc-wasm` checks the two agree.
pub const ALIASES: [(&str, &str); 2] = [("Calibri", "Carlito"), ("Calibri Light", "Carlito")];

/// The four Carlito faces as `(bold, italic, bytes)`.
pub const FACES: [(bool, bool, &[u8]); 4] = [
    (
        false,
        false,
        include_bytes!("../../../fonts/Carlito-Regular.ttf"),
    ),
    (
        true,
        false,
        include_bytes!("../../../fonts/Carlito-Bold.ttf"),
    ),
    (
        false,
        true,
        include_bytes!("../../../fonts/Carlito-Italic.ttf"),
    ),
    (
        true,
        true,
        include_bytes!("../../../fonts/Carlito-BoldItalic.ttf"),
    ),
];

/// The default set as a font port.
#[must_use]
pub fn default_fonts() -> FontLibrary {
    let mut lib = FontLibrary::new(FALLBACK);
    for (bold, italic, bytes) in FACES {
        lib.add(FALLBACK, bold, italic, bytes.to_vec());
    }
    for (name, target) in ALIASES {
        lib.alias(name, target);
    }
    lib
}
