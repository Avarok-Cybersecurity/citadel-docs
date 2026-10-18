//! Package imports at the boundary: a `.docx` or `.odt` to ProseMirror JSON with its report,
//! read within the limits the caller passes.

use crate::convert::{json, options, problems};
use cdoc_docx::{read_docx, Imported, MemoryMedia, ReadLimits};
use cdoc_odt::read_odt;
use cdoc_pm::to_pm_json;
use serde::Deserialize;

/// A package import: `{"pm", "report", "problems", "kind"}`, the kind being `"document"` or
/// `"template"`.
fn imported_package(read: &Imported) -> String {
    format!(
        "{{\"pm\":{},\"report\":{},\"problems\":{},\"kind\":{}}}",
        to_pm_json(&read.document),
        json(&read.report),
        json(&problems(&read.document)),
        json(&read.kind)
    )
}

/// Package import options: `{"limits": <ReadLimits>}`, the bounds on what the read inflates
/// and parses. Required: the right bounds for a browser tab are not a server's.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PackageReadOptions {
    limits: ReadLimits,
}

/// The read limits of package import options.
pub(crate) fn read_limits(options_json: &str) -> Result<ReadLimits, String> {
    let o: PackageReadOptions = options(options_json, "package read")?;
    Ok(o.limits)
}

/// A `.docx`, `.dotx`, `.docm` or `.dotm` to `{"pm", "report", "problems", "kind"}`. Embedded
/// images are added to `media` under provisional ids; macros are refused and reported.
/// Options: `{"limits": <ReadLimits>}`.
pub fn docx_to_pm(
    bytes: &[u8],
    media: &mut MemoryMedia,
    options_json: &str,
) -> Result<String, String> {
    let limits = read_limits(options_json)?;
    let read = read_docx(bytes, &limits, media).map_err(|e| e.to_string())?;
    Ok(imported_package(&read))
}

/// A `.odt` or `.ott` to `{"pm", "report", "problems", "kind"}`. Embedded images are added
/// to `media` under provisional ids. Options: `{"limits": <ReadLimits>}`.
pub fn odt_to_pm(
    bytes: &[u8],
    media: &mut MemoryMedia,
    options_json: &str,
) -> Result<String, String> {
    let limits = read_limits(options_json)?;
    let read = read_odt(bytes, &limits, media).map_err(|e| e.to_string())?;
    Ok(imported_package(&read))
}
