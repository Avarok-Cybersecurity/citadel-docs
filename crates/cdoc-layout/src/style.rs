//! What a run looks like once the cascade has resolved it: a face, a size, a colour and the
//! decorations drawn with the glyphs.

use crate::fonts::Fonts;
use crate::ir::{FaceId, Pt100};
use crate::LayoutError;
use cdoc_model::{Color, RunProperties, UnderlineStyle, VerticalAlign};
use cdoc_shape::{Feature, Features};

/// "Automatic" text colour.
pub const INK: Color = Color::rgb(0, 0, 0);

/// Capitalisation applied when the text is set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Caps {
    /// As typed.
    AsTyped,
    /// Every letter upper case.
    All,
    /// Lower-case letters as reduced capitals.
    Small,
}

/// A resolved run style.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextStyle {
    /// The face.
    pub face: FaceId,
    /// The size the glyphs are set at (already reduced for super/subscript).
    pub size: Pt100,
    /// The size before super/subscript reduction: what line height is measured from.
    pub line_size: Pt100,
    /// Glyph colour.
    pub colour: Color,
    /// Underline and its colour.
    pub underline: Option<(UnderlineStyle, Color)>,
    /// Strikethrough.
    pub strike: bool,
    /// Highlight behind the glyphs.
    pub highlight: Option<Color>,
    /// Baseline shift, positive upwards.
    pub rise: Pt100,
    /// Capitalisation.
    pub caps: Caps,
    /// The OpenType features the glyphs are shaped with.
    pub features: Features,
    /// The hyphenation patterns for the run's language (its own, else the document's);
    /// `None` when the font port has none registered for it, or no language is given.
    pub patterns: Option<cdoc_para::Language>,
}

/// The features a run is shaped with. Where the cascade leaves ligatures or kerning unset
/// the run gets [`Features::word_default`]: Word's own choice, which it writes into every new
/// document's defaults (`w14:ligatures standardContextual`, `w:kern`), not one made here.
/// Historical and discretional ligatures are not implemented by the shaper and are not
/// applied.
#[must_use]
pub fn features(run: &RunProperties) -> Features {
    let ligatures = run.ligatures;
    let kern = match (run.kern, run.size) {
        (None, _) => true,
        (Some(min), size) => min.0 != 0 && size.is_some_and(|s| s.0 >= min.0),
    };
    Features::word_default()
        .with(
            Feature::Liga,
            ligatures.is_none_or(cdoc_model::Ligatures::standard),
        )
        .with(
            Feature::Clig,
            ligatures.is_none_or(cdoc_model::Ligatures::contextual),
        )
        .with(Feature::Kern, kern)
}

/// The pattern set for a BCP 47 tag.
#[must_use]
pub fn patterns_for(tag: Option<&str>) -> Option<cdoc_para::Language> {
    tag.and_then(cdoc_para::Language::from_bcp47)
}

impl TextStyle {
    /// The style for fully resolved run properties; `language` is the document's, which a run
    /// without one of its own is in.
    pub fn resolve(
        run: &RunProperties,
        fonts: &mut Fonts<'_>,
        language: Option<&str>,
    ) -> Result<Self, LayoutError> {
        let family = run.font.as_deref().ok_or(LayoutError::NoFont)?;
        let half_points = run.size.ok_or(LayoutError::NoFont)?;
        let face = fonts.resolve(
            family,
            run.bold.unwrap_or(false),
            run.italic.unwrap_or(false),
        )?;
        let line_size = i32::from(half_points.0) * 50;
        let (size, rise) = match run.vertical_align {
            Some(VerticalAlign::Superscript) => (line_size * 65 / 100, line_size * 33 / 100),
            Some(VerticalAlign::Subscript) => (line_size * 65 / 100, -(line_size * 14 / 100)),
            Some(VerticalAlign::Baseline) | None => (line_size, 0),
        };
        let colour = run.color.unwrap_or(INK);
        let underline = run
            .underline
            .as_ref()
            .filter(|u| u.style != UnderlineStyle::None)
            .map(|u| (u.style, u.color.unwrap_or(colour)));
        let caps = if run.all_caps == Some(true) {
            Caps::All
        } else if run.small_caps == Some(true) {
            Caps::Small
        } else {
            Caps::AsTyped
        };
        Ok(Self {
            face,
            size,
            line_size,
            colour,
            underline,
            strike: run.strike.unwrap_or(false),
            highlight: run.highlight,
            rise,
            caps,
            features: features(run),
            patterns: patterns_for(run.lang.as_deref().or(language))
                .filter(|l| fonts.hyphenator(*l).is_some()),
        })
    }

    /// The same style at a different face, for a label or a field set in another font.
    #[must_use]
    pub fn superscript(&self) -> Self {
        Self {
            size: self.line_size * 65 / 100,
            rise: self.line_size * 33 / 100,
            ..self.clone()
        }
    }

    /// Ascent of this style's line box.
    #[must_use]
    pub fn ascent(&self, fonts: &Fonts<'_>) -> Pt100 {
        fonts
            .ascent(self.face, self.line_size)
            .max(fonts.ascent(self.face, self.size) + self.rise)
    }

    /// Descent of this style's line box.
    #[must_use]
    pub fn descent(&self, fonts: &Fonts<'_>) -> Pt100 {
        fonts
            .descent(self.face, self.line_size)
            .max(fonts.descent(self.face, self.size) - self.rise)
    }
}

/// Text after capitalisation: pieces of `(text, reduced)`, where `reduced` pieces are set at
/// small-capital size.
#[must_use]
pub fn capitalise(text: &str, caps: Caps) -> Vec<(String, bool)> {
    match caps {
        Caps::AsTyped => vec![(text.to_owned(), false)],
        Caps::All => vec![(text.to_uppercase(), false)],
        Caps::Small => {
            let mut out: Vec<(String, bool)> = Vec::new();
            for ch in text.chars() {
                let reduced = ch.is_lowercase();
                let piece: String = if reduced {
                    ch.to_uppercase().collect()
                } else {
                    ch.to_string()
                };
                match out.last_mut() {
                    Some((s, r)) if *r == reduced => s.push_str(&piece),
                    _ => out.push((piece, reduced)),
                }
            }
            out
        }
    }
}

/// Small-capital size: four fifths of the run's.
#[must_use]
pub const fn small_cap_size(size: Pt100) -> Pt100 {
    size * 4 / 5
}
