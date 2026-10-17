//! Which link targets a painter honours. One rule for every painter (the PDF writer, the SVG
//! preview, the canvas editor) so a link that is dead in one is dead in all.

/// The schemes a link may use in the file or the preview. `javascript:`, `data:`, `file:` and
/// everything else are dropped: a document from another user is untrusted input.
#[must_use]
pub fn allowed_link(href: &str) -> bool {
    let lower = href.trim_start().to_ascii_lowercase();
    ["https://", "http://", "mailto:"]
        .iter()
        .any(|scheme| lower.starts_with(scheme))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_web_and_mail_links_are_honoured() {
        assert!(allowed_link("https://avarok.net"));
        assert!(allowed_link("  MAILTO:a@b.c"));
        assert!(!allowed_link("javascript:alert(1)"));
        assert!(!allowed_link("data:text/html,x"));
        assert!(!allowed_link("file:///etc/passwd"));
    }
}
