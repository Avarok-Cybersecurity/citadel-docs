//! Shared by the integration tests: the fixture fonts, and rustybuzz configured to shape with
//! exactly the features this shaper implements, as the oracle.

#![allow(dead_code)]

use cdoc_shape::{
    Direction, Face, Feature, Features, Glyph, Language, Script, ShapeParams, Shaper,
};
use std::sync::OnceLock;

pub const CARLITO: &[u8] = include_bytes!("../../../../fonts/Carlito-Regular.ttf");
pub const CARLITO_BOLD: &[u8] = include_bytes!("../../../../fonts/Carlito-Bold.ttf");
pub const CARLITO_ITALIC: &[u8] = include_bytes!("../../../../fonts/Carlito-Italic.ttf");
pub const NOTO: &[u8] = include_bytes!("../../../../fonts/NotoSans-LGC-Subset.ttf");

/// Every face the oracle is run against, by name.
pub const FACES: [(&str, &[u8]); 4] = [
    ("Carlito", CARLITO),
    ("Carlito Bold", CARLITO_BOLD),
    ("Carlito Italic", CARLITO_ITALIC),
    ("Noto Sans subset", NOTO),
];

/// The fixture faces, ours and rustybuzz's, parsed once per test binary.
pub fn faces() -> &'static [(&'static str, Face<'static>, rustybuzz::Face<'static>)] {
    static FACES_PARSED: OnceLock<Vec<(&str, Face<'static>, rustybuzz::Face<'static>)>> =
        OnceLock::new();
    FACES_PARSED.get_or_init(|| {
        FACES
            .iter()
            .map(|&(name, bytes)| {
                let ours = Face::parse(bytes, 0).expect("face");
                let theirs = rustybuzz::Face::from_slice(bytes, 0).expect("rustybuzz face");
                (name, ours, theirs)
            })
            .collect()
    })
}

pub fn params(script: Script, features: Features) -> ShapeParams {
    ShapeParams {
        script,
        language: Language::default_system(),
        direction: Direction::LeftToRight,
        features,
    }
}

pub fn ours(face: &Face<'_>, text: &str, params: ShapeParams) -> Vec<Glyph> {
    Shaper::new(face)
        .shape(text, &params)
        .expect("shapes")
        .glyphs
}

fn rb_script(script: Script) -> rustybuzz::Script {
    match script {
        Script::Latin => rustybuzz::script::LATIN,
        Script::Greek => rustybuzz::script::GREEK,
        Script::Cyrillic => rustybuzz::script::CYRILLIC,
        Script::Common | Script::Unsupported => rustybuzz::script::COMMON,
    }
}

pub fn rb_features(features: Features) -> Vec<rustybuzz::Feature> {
    let off = cdoc_shape::HARFBUZZ_DEFAULTS_NOT_IMPLEMENTED
        .iter()
        .map(|t| rustybuzz::Feature::new(rustybuzz::ttf_parser::Tag::from_bytes(t), 0, ..));
    let ours = Feature::ALL.iter().map(|f| {
        let tag = rustybuzz::ttf_parser::Tag::from_bytes(&f.tag().to_bytes());
        rustybuzz::Feature::new(tag, u32::from(features.contains(*f)), ..)
    });
    off.chain(ours).collect()
}

pub fn rustybuzz(face: &rustybuzz::Face<'_>, text: &str, params: ShapeParams) -> Vec<Glyph> {
    let mut buffer = rustybuzz::UnicodeBuffer::new();
    buffer.push_str(text);
    buffer.set_direction(rustybuzz::Direction::LeftToRight);
    buffer.set_script(rb_script(params.script));
    let shaped = rustybuzz::shape(face, &rb_features(params.features), buffer);
    shaped
        .glyph_infos()
        .iter()
        .zip(shaped.glyph_positions())
        .map(|(i, p)| Glyph {
            id: u16::try_from(i.glyph_id).expect("glyph id"),
            cluster: i.cluster,
            x_advance: p.x_advance,
            y_advance: p.y_advance,
            x_offset: p.x_offset,
            y_offset: p.y_offset,
        })
        .collect()
}
