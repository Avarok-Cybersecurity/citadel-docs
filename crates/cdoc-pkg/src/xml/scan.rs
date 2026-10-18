//! Byte searches for the tokeniser. Text runs are long, so they are searched a word at a time
//! for both of the bytes that end them (`<` and `&`) in one pass; tags and attribute values are
//! short, where a plain loop beats any search's start-up cost.

const LANES: usize = 8;
const ONES: u64 = u64::from_ne_bytes([0x01; LANES]);
const HIGHS: u64 = u64::from_ne_bytes([0x80; LANES]);

/// Whether any byte of `word` is zero (the classic bit trick: exact, no false positives once
/// the lowest set lane is taken, which is all a search needs).
const fn has_zero(word: u64) -> bool {
    word.wrapping_sub(ONES) & !word & HIGHS != 0
}

/// The first index at or after `from` holding `a` or `b`.
pub(super) fn find2(bytes: &[u8], from: usize, a: u8, b: u8) -> Option<usize> {
    let wa = u64::from_ne_bytes([a; LANES]);
    let wb = u64::from_ne_bytes([b; LANES]);
    let (chunks, tail) = bytes.get(from..)?.as_chunks::<LANES>();
    let mut base = from;
    for chunk in chunks {
        let word = u64::from_ne_bytes(*chunk);
        if has_zero(word ^ wa) || has_zero(word ^ wb) {
            return chunk
                .iter()
                .position(|c| *c == a || *c == b)
                .map(|i| base + i);
        }
        base += LANES;
    }
    tail.iter()
        .position(|c| *c == a || *c == b)
        .map(|i| base + i)
}

/// The first index at or after `from` holding `c`; for short spans.
pub(super) fn find(bytes: &[u8], from: usize, c: u8) -> Option<usize> {
    bytes
        .get(from..)?
        .iter()
        .position(|x| *x == c)
        .map(|i| from + i)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find2_finds_the_first_of_either_byte_in_every_lane_and_the_tail() {
        for len in 0..40 {
            for at in 0..len {
                for (hit, other) in [(b'<', b'&'), (b'&', b'<')] {
                    let mut text = vec![b'x'; len];
                    text[at] = hit;
                    if at + 3 < len {
                        text[at + 3] = other;
                    }
                    for from in 0..=at {
                        assert_eq!(
                            find2(&text, from, b'<', b'&'),
                            Some(at),
                            "{len} {at} {from}"
                        );
                    }
                    assert_eq!(
                        find2(&text, at + 1, b'<', b'&'),
                        (at + 3 < len).then_some(at + 3)
                    );
                }
            }
            assert_eq!(find2(&vec![b'x'; len], 0, b'<', b'&'), None);
        }
    }

    #[test]
    fn bytes_with_the_high_bit_set_are_not_mistaken_for_either() {
        let text = "é€😀 plain <".as_bytes();
        assert_eq!(find2(text, 0, b'<', b'&'), Some(text.len() - 1));
        assert_eq!(
            find2(&[0xBC, 0xA6, 0x3C ^ 0x80, 0x26 ^ 0x80], 0, b'<', b'&'),
            None
        );
    }

    #[test]
    fn a_start_past_the_end_finds_nothing() {
        assert_eq!(find2(b"<", 2, b'<', b'&'), None);
        assert_eq!(find(b"<", 2, b'<'), None);
        assert_eq!(find(b"ab>", 0, b'>'), Some(2));
    }
}
