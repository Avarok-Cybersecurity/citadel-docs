//! ODF measures to and from the model's integer units, with integer arithmetic only.
//!
//! **Reading.** A length is a decimal number and a unit (`cm`, `mm`, `in`, `inch`, `pt`, `pc`,
//! `px`). It is converted exactly as a rational number to the target unit (hundredths of a
//! point, half-points, eighths of a point) and rounded once, to the nearest unit, halves away
//! from zero: `2.54cm` is 7200 hundredths, `1cm` is 2834.6… and becomes 2835, `-0.005pt` is
//! -0.5 and becomes -1. A `px` is a CSS pixel, three quarters of a point. Percentages are read
//! the same way, scaled to the model's unit (240ths of a line, fiftieths of a percent).
//!
//! **Writing.** Every model length is written in points with as many decimals as it needs
//! (`12pt`, `12.5pt`, `0.05pt`), which is exact, so `read(write(x)) == x` for every value.
//! Line spacing multiples are written as percentages with four decimals; reading rounds back to
//! the same 240th because the written value is within 0.00005 % of the exact one.

/// A decimal number as mantissa and scale: `-12.345` is `(-12345, 3)`.
fn decimal(text: &str) -> Option<(i128, u32)> {
    let (negative, digits) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text.strip_prefix('+').unwrap_or(text)),
    };
    let (whole, fraction) = digits.split_once('.').unwrap_or((digits, ""));
    if whole.is_empty() && fraction.is_empty() {
        return None;
    }
    if !whole
        .bytes()
        .chain(fraction.bytes())
        .all(|b| b.is_ascii_digit())
    {
        return None;
    }
    if whole.len() + fraction.len() > 30 {
        return None;
    }
    let mut mantissa: i128 = 0;
    for b in whole.bytes().chain(fraction.bytes()) {
        mantissa = mantissa * 10 + i128::from(b - b'0');
    }
    let scale = u32::try_from(fraction.len()).ok()?;
    Some((if negative { -mantissa } else { mantissa }, scale))
}

/// `n / d` rounded to the nearest integer, halves away from zero; `d > 0`.
fn round_ratio(n: i128, d: i128) -> i128 {
    let twice = 2 * n.abs() + d;
    let magnitude = twice / (2 * d);
    if n < 0 {
        -magnitude
    } else {
        magnitude
    }
}

/// Points per unit, as a fraction.
fn points_per(unit: &str) -> Option<(i128, i128)> {
    Some(match unit {
        "pt" => (1, 1),
        "in" | "inch" => (72, 1),
        "cm" => (3600, 127),
        "mm" => (360, 127),
        "pc" => (12, 1),
        "px" => (3, 4),
        _ => return None,
    })
}

/// A length in target units, where one point is `per_point` of them.
fn length_in(text: &str, per_point: i128) -> Option<i64> {
    let text = text.trim();
    let split = text
        .find(|c: char| c.is_ascii_alphabetic())
        .unwrap_or(text.len());
    let (number, unit) = text.split_at(split);
    let (mantissa, scale) = decimal(number)?;
    let (num, den) = points_per(unit)?;
    let value = round_ratio(mantissa * num * per_point, den * 10i128.pow(scale));
    i64::try_from(value).ok()
}

fn to_i32(value: i64) -> Option<i32> {
    i32::try_from(value).ok()
}

/// A length in hundredths of a point.
#[must_use]
pub fn length(text: &str) -> Option<i32> {
    length_in(text, 100).and_then(to_i32)
}

/// A length whose sign matters even at zero (`fo:text-indent`): the value and whether it was
/// written negative, so `-0pt` (a zero hang) and `0pt` (a zero indent) differ.
#[must_use]
pub fn signed_length(text: &str) -> Option<(i32, bool)> {
    Some((length(text)?, text.trim().starts_with('-')))
}

/// A font size in half-points.
#[must_use]
pub fn half_points(text: &str) -> Option<u16> {
    length_in(text, 2).and_then(|v| u16::try_from(v).ok())
}

/// A border width in eighths of a point. A visible width that rounds to zero is one eighth.
#[must_use]
pub fn eighths(text: &str) -> Option<u16> {
    let exact = length_in(text, 8)?;
    let positive = length_in(text, 1_000_000).is_some_and(|v| v > 0);
    let value = if exact == 0 && positive { 1 } else { exact };
    u16::try_from(value).ok()
}

/// A percentage scaled to a unit with `per_percent` units in one percent (`num / den`).
#[must_use]
pub fn percent(text: &str, num: i128, den: i128) -> Option<i64> {
    let (mantissa, scale) = decimal(text.trim().strip_suffix('%')?.trim())?;
    i64::try_from(round_ratio(mantissa * num, den * 10i128.pow(scale))).ok()
}

/// `value / 10^decimals` as a decimal string without trailing zeros.
fn fixed(value: i64, decimals: u32, suffix: &str) -> String {
    let sign = if value < 0 { "-" } else { "" };
    let magnitude = value.unsigned_abs();
    let base = 10u64.pow(decimals);
    let whole = magnitude / base;
    let fraction = magnitude % base;
    if fraction == 0 {
        return format!("{sign}{whole}{suffix}");
    }
    let width = usize::try_from(decimals).unwrap_or(0);
    let digits = format!("{fraction:0width$}");
    format!("{sign}{whole}.{}{suffix}", digits.trim_end_matches('0'))
}

/// Hundredths of a point as an ODF length.
#[must_use]
pub fn write_length(pt100: i32) -> String {
    fixed(i64::from(pt100), 2, "pt")
}

/// A zero-sensitive signed length: `negative` with a zero value writes `-0pt`.
#[must_use]
pub fn write_signed_length(pt100: i32, negative: bool) -> String {
    if negative && pt100 == 0 {
        "-0pt".to_owned()
    } else if negative {
        write_length(-pt100)
    } else {
        write_length(pt100)
    }
}

/// Half-points as an ODF length.
#[must_use]
pub fn write_half_points(half_points: u16) -> String {
    fixed(i64::from(half_points) * 5, 1, "pt")
}

/// Eighths of a point as an ODF length.
#[must_use]
pub fn write_eighths(eighths: u16) -> String {
    fixed(i64::from(eighths) * 125, 3, "pt")
}

/// Fiftieths of a percent as an ODF percentage.
#[must_use]
pub fn write_fiftieths(fiftieths: u16) -> String {
    fixed(i64::from(fiftieths) * 2, 2, "%")
}

/// 240ths of a line as an ODF percentage, four decimals.
#[must_use]
pub fn write_line_multiple(lines: u32) -> String {
    let ten_thousandths = round_ratio(i128::from(lines) * 1_000_000, 240);
    fixed(i64::try_from(ten_thousandths).unwrap_or(i64::MAX), 4, "%")
}

/// An ODF percentage as 240ths of a line.
#[must_use]
pub fn line_multiple(text: &str) -> Option<u32> {
    percent(text, 240, 100).and_then(|v| u32::try_from(v).ok())
}

/// An ODF percentage as fiftieths of a percent.
#[must_use]
pub fn fiftieths(text: &str) -> Option<u16> {
    percent(text, 50, 1).and_then(|v| u16::try_from(v).ok())
}

#[cfg(test)]
#[path = "units_tests.rs"]
mod tests;
