// Adapted from rMazing crates/cc-export/src/layout/svg_path.rs
// Copyright (c) Thomas Braun. Relicensed MIT OR Apache-2.0 by the copyright holder, 2026-10-04.

//! A subset of SVG path data, read into the IR's segments: how the layout's own vector marks
//! (the check box tick) are written down legibly instead of as coordinate tables.
//!
//! Absolute `M L H V Q C Z` only. Numbers are in the path's own units and multiplied by
//! `scale` hundredths of a point per unit, so one drawing serves every size.

use crate::ir::{Pt100, Segment};

/// Path data that is not in the supported subset.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PathError {
    /// A number did not parse.
    #[error("not a number in path data: {0:?}")]
    BadNumber(String),
    /// A command outside `M L H V Q C Z`.
    #[error("unsupported path command {0:?}")]
    UnknownCommand(char),
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "path coordinates are a few points"
)]
fn at(value: f32, scale: Pt100) -> Pt100 {
    #[expect(clippy::cast_precision_loss, reason = "scale is small")]
    let scaled = value * scale as f32;
    scaled.round() as Pt100
}

/// The path data, read into segments.
pub fn segments(d: &str, scale: Pt100) -> Result<Vec<Segment>, PathError> {
    let mut out = Vec::new();
    let mut nums: Vec<f32> = Vec::new();
    let mut command = None;
    let mut cursor = (0.0_f32, 0.0_f32);
    let mut pending = String::new();
    let flush = |pending: &mut String, nums: &mut Vec<f32>| -> Result<(), PathError> {
        if !pending.is_empty() {
            nums.push(
                pending
                    .parse()
                    .map_err(|_| PathError::BadNumber(pending.clone()))?,
            );
            pending.clear();
        }
        Ok(())
    };
    for ch in d.chars().chain(std::iter::once('\0')) {
        if ch.is_ascii_alphabetic() || ch == '\0' {
            flush(&mut pending, &mut nums)?;
            if let Some(c) = command {
                emit(c, &nums, &mut cursor, scale, &mut out)?;
            }
            nums.clear();
            command = Some(ch);
        } else if ch == ' ' || ch == ',' {
            flush(&mut pending, &mut nums)?;
        } else if ch == '-' && !pending.is_empty() && !pending.ends_with('e') {
            flush(&mut pending, &mut nums)?;
            pending.push(ch);
        } else {
            pending.push(ch);
        }
    }
    Ok(out)
}

fn emit(
    command: char,
    nums: &[f32],
    cursor: &mut (f32, f32),
    scale: Pt100,
    out: &mut Vec<Segment>,
) -> Result<(), PathError> {
    let point = |x: f32, y: f32| (at(x, scale), at(y, scale));
    match command {
        'M' | 'L' => {
            for (i, pair) in nums.as_chunks::<2>().0.iter().enumerate() {
                let (x, y) = point(pair[0], pair[1]);
                out.push(if command == 'M' && i == 0 {
                    Segment::Move(x, y)
                } else {
                    Segment::Line(x, y)
                });
                *cursor = (pair[0], pair[1]);
            }
        }
        'H' | 'V' => {
            for v in nums {
                *cursor = if command == 'H' {
                    (*v, cursor.1)
                } else {
                    (cursor.0, *v)
                };
                let (x, y) = point(cursor.0, cursor.1);
                out.push(Segment::Line(x, y));
            }
        }
        'Q' => {
            for q in nums.as_chunks::<4>().0 {
                let (cx, cy) = *cursor;
                let c1 = (cx + 2.0 / 3.0 * (q[0] - cx), cy + 2.0 / 3.0 * (q[1] - cy));
                let c2 = (
                    q[2] + 2.0 / 3.0 * (q[0] - q[2]),
                    q[3] + 2.0 / 3.0 * (q[1] - q[3]),
                );
                let (x1, y1) = point(c1.0, c1.1);
                let (x2, y2) = point(c2.0, c2.1);
                let (x3, y3) = point(q[2], q[3]);
                out.push(Segment::Cubic([x1, y1, x2, y2, x3, y3]));
                *cursor = (q[2], q[3]);
            }
        }
        'C' => {
            for c in nums.as_chunks::<6>().0 {
                let (x1, y1) = point(c[0], c[1]);
                let (x2, y2) = point(c[2], c[3]);
                let (x3, y3) = point(c[4], c[5]);
                out.push(Segment::Cubic([x1, y1, x2, y2, x3, y3]));
                *cursor = (c[4], c[5]);
            }
        }
        'Z' => out.push(Segment::Close),
        '\0' => {}
        other => return Err(PathError::UnknownCommand(other)),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tick_reads_into_scaled_segments() {
        let s = segments("M0 5 L4 9 L10 1", 100).expect("parses");
        assert_eq!(
            s,
            vec![
                Segment::Move(0, 500),
                Segment::Line(400, 900),
                Segment::Line(1000, 100)
            ]
        );
    }

    #[test]
    fn horizontal_vertical_and_close_track_the_cursor() {
        let s = segments("M1 1 H5 V3 Z", 10).expect("parses");
        assert_eq!(
            s,
            vec![
                Segment::Move(10, 10),
                Segment::Line(50, 10),
                Segment::Line(50, 30),
                Segment::Close
            ]
        );
    }

    #[test]
    fn arcs_and_bad_numbers_are_refused_by_name() {
        assert_eq!(
            segments("M0 0 A1 1 0 0 1 2 2", 1),
            Err(PathError::UnknownCommand('A'))
        );
        assert!(matches!(
            segments("M0 1.2.3", 1),
            Err(PathError::BadNumber(_))
        ));
    }
}
