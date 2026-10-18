//! Entry names without the UTF-8 flag are IBM code page 437, as the ZIP format defines. Bytes
//! below 0x80 are ASCII; the rest come from this table.

const HIGH: &str = "ÇüéâäàåçêëèïîìÄÅÉæÆôöòûùÿÖÜ¢£¥₧ƒáíóúñÑªº¿⌐¬½¼¡«»░▒▓│┤╡╢╖╕╣║╗╝╜╛┐└┴┬├─┼╞╟╚╔╩╦╠═╬╧╨╤╥╙╘╒╓╫╪┘┌█▄▌▐▀αßΓπΣσµτΦΘΩδ∞φε∩≡±≥≤⌠⌡÷≈°∙·√ⁿ²■\u{a0}";

/// A code page 437 name as text.
pub(super) fn decode(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&b| match b.checked_sub(0x80) {
            None => char::from(b),
            Some(high) => HIGH.chars().nth(usize::from(high)).unwrap_or('\u{FFFD}'),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_covers_every_high_byte_and_ascii_passes_through() {
        assert_eq!(HIGH.chars().count(), 128);
        assert_eq!(decode(b"word/a.xml"), "word/a.xml");
        assert_eq!(decode(&[0x80, 0x82, 0xE1, 0xFF]), "\u{C7}\u{E9}\u{DF}\u{A0}");
    }
}
