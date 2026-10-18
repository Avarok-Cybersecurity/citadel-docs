//! Round-trip helpers for the format crates' tests (feature `test-support`): a media port with
//! real behaviour (not a mock) and a first-difference finder. Never enabled by production code.

use crate::media::MediaSink;
use cdoc_model::{ContentId, Document, ImageBytes, ImageError, ImageLibrary, InlineContent};

/// An image's bytes in these tests: a PNG signature followed by its content id, so the id can
/// always be recovered from the bytes. [`library_for`] serves them (the workspace content
/// store, which real callers wire in); [`IdFromBytes`] is the reading side.
const SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";

/// Every image the document names, served as signature plus id.
#[must_use]
pub fn library_for(doc: &Document) -> ImageLibrary {
    let mut library = ImageLibrary::empty();
    for inline in doc.inlines() {
        if let InlineContent::Image(img) = &inline.content {
            let mut bytes = SIGNATURE.to_vec();
            bytes.extend_from_slice(img.content_id.as_str().as_bytes());
            library.add(img.content_id.clone(), "image/png", bytes);
        }
    }
    library
}

/// A stateless sink: the id is whatever the bytes say.
#[derive(Debug, Clone, Copy)]
pub struct IdFromBytes;

impl MediaSink for IdFromBytes {
    fn store(&mut self, image: ImageBytes<'_>) -> Result<ContentId, ImageError> {
        let rest = image
            .bytes
            .strip_prefix(SIGNATURE)
            .ok_or_else(|| ImageError("not a test image".to_owned()))?;
        String::from_utf8(rest.to_vec())
            .map(ContentId)
            .map_err(|e| ImageError(e.to_string()))
    }
}

fn diff_values(path: &str, left: &serde_json::Value, right: &serde_json::Value) -> Option<String> {
    use serde_json::Value::{Array, Object};
    match (left, right) {
        (Object(lo), Object(ro)) => {
            let keys: std::collections::BTreeSet<_> = lo.keys().chain(ro.keys()).collect();
            keys.into_iter()
                .find_map(|key| match (lo.get(key), ro.get(key)) {
                    (Some(lv), Some(rv)) => diff_values(&format!("{path}.{key}"), lv, rv),
                    (lv, rv) => Some(format!("{path}.{key}: {lv:?} != {rv:?}")),
                })
        }
        (Array(la), Array(ra)) => {
            if la.len() != ra.len() {
                return Some(format!("{path}: length {} != {}", la.len(), ra.len()));
            }
            la.iter()
                .zip(ra)
                .enumerate()
                .find_map(|(i, (lv, rv))| diff_values(&format!("{path}[{i}]"), lv, rv))
        }
        _ => (left != right).then(|| format!("{path}: {left} != {right}")),
    }
}

/// The first place two documents differ, as a JSON path with both values; `None` when equal.
///
/// # Panics
/// Never: model values always serialise.
#[must_use]
pub fn first_diff(left: &Document, right: &Document) -> Option<String> {
    let l = serde_json::to_value(left).expect("model serialises");
    let r = serde_json::to_value(right).expect("model serialises");
    diff_values("doc", &l, &r)
}
