//! Measurement units. Every unit is an integer in the resolution Word itself uses, so a value
//! read from a `.docx` and written back is the same number, and the layout crate (which works
//! in hundredths of a point) converts with exact integer arithmetic.

use serde::{Deserialize, Serialize};

/// A length in **hundredths of a PostScript point** (`Pt100`): 7200 is an inch, 2835 is a
/// centimetre, 5 is a twip. Negative values are legal where Word allows them (indents).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Length(pub i32);

impl Length {
    /// Hundredths of a point per inch.
    pub const PER_INCH: i32 = 7200;
    /// Hundredths of a point per twip (a twentieth of a point).
    pub const PER_TWIP: i32 = 5;
    /// Hundredths of a point per point.
    pub const PER_POINT: i32 = 100;

    /// A length from twips, the unit `.docx` uses for page geometry and indents. Exact.
    #[must_use]
    pub const fn from_twips(twips: i32) -> Self {
        Self(twips * Self::PER_TWIP)
    }

    /// A length from whole points. Exact.
    #[must_use]
    pub const fn from_points(points: i32) -> Self {
        Self(points * Self::PER_POINT)
    }

    /// A length from inches, as a float; rounded to the nearest hundredth of a point.
    #[must_use]
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a page dimension is far smaller than i32 hundredths"
    )]
    pub fn from_inches(inches: f32) -> Self {
        Self((inches * 7200.0).round() as i32)
    }

    /// A length from millimetres, rounded to the nearest hundredth of a point.
    #[must_use]
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a page dimension is far smaller than i32 hundredths"
    )]
    pub fn from_mm(mm: f32) -> Self {
        Self((mm * 72.0 / 25.4 * 100.0).round() as i32)
    }

    /// The length in points, for a backend that writes decimals.
    #[must_use]
    #[expect(
        clippy::cast_precision_loss,
        reason = "hundredths of a point fit f32 exactly for any page"
    )]
    pub fn to_points(self) -> f32 {
        self.0 as f32 / Self::PER_POINT as f32
    }

    /// Whether the length is zero or positive.
    #[must_use]
    pub const fn is_non_negative(self) -> bool {
        self.0 >= 0
    }

    /// Whether the length is strictly positive.
    #[must_use]
    pub const fn is_positive(self) -> bool {
        self.0 > 0
    }
}

/// A font size in **half-points**, the resolution Word's size box offers: 24 is 12 pt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FontSize(pub u16);

impl FontSize {
    /// A size from whole points.
    #[must_use]
    pub const fn from_points(points: u16) -> Self {
        Self(points * 2)
    }

    /// The size in points.
    #[must_use]
    pub fn to_points(self) -> f32 {
        f32::from(self.0) / 2.0
    }

    /// Whether the size is usable; a zero size is a validation error.
    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.0 > 0
    }
}

/// A border width in **eighths of a point**, as `.docx` stores it: 4 is half a point.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BorderWidth(pub u16);

impl BorderWidth {
    /// The width in points.
    #[must_use]
    pub fn to_points(self) -> f32 {
        f32::from(self.0) / 8.0
    }
}

/// An opaque sRGB colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Color {
    /// Red channel.
    pub r: u8,
    /// Green channel.
    pub g: u8,
    /// Blue channel.
    pub b: u8,
}

impl Color {
    /// A colour from its channels.
    #[must_use]
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// `#RRGGBB`, upper case, the spelling shared with CSS and OOXML.
    #[must_use]
    pub fn hex(self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }

    /// Parse `#RRGGBB` or `RRGGBB`, case-insensitive.
    pub fn parse_hex(text: &str) -> Result<Self, ColorParseError> {
        let digits = text.strip_prefix('#').unwrap_or(text);
        if digits.len() != 6 || !digits.is_ascii() {
            return Err(ColorParseError(text.to_owned()));
        }
        let channel = |range: std::ops::Range<usize>| {
            u8::from_str_radix(&digits[range], 16).map_err(|_| ColorParseError(text.to_owned()))
        };
        Ok(Self::rgb(channel(0..2)?, channel(2..4)?, channel(4..6)?))
    }
}

/// A colour string that is not `#RRGGBB`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("not a #RRGGBB colour: {0:?}")]
pub struct ColorParseError(pub String);

impl TryFrom<String> for Color {
    type Error = ColorParseError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse_hex(&value)
    }
}

impl From<Color> for String {
    fn from(value: Color) -> Self {
        value.hex()
    }
}

/// A table or cell width, as Word expresses it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TableWidth {
    /// Sized to content.
    Auto,
    /// A fixed length.
    Fixed {
        /// The width.
        length: Length,
    },
    /// A percentage of the enclosing width, in **fiftieths of a percent**: 5000 is 100 %.
    Percent {
        /// Fiftieths of a percent.
        fiftieths: u16,
    },
}
