//! `[Content_Types].xml`: the MIME type of each part.

use crate::error::DocxError;
use crate::xml;
use cdoc_pkg::ReadLimits;

/// Default types by extension and overrides by part name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentTypes {
    defaults: Vec<(String, String)>,
    overrides: Vec<(String, String)>,
}

impl ContentTypes {
    /// Parse the part.
    pub fn parse(bytes: &[u8], limits: &ReadLimits) -> Result<Self, DocxError> {
        let tree = xml::parse("[Content_Types].xml", bytes, limits)?;
        let mut types = Self {
            defaults: Vec::new(),
            overrides: Vec::new(),
        };
        for n in tree.root().elems() {
            match (n.name(), n.get("ContentType")) {
                ("Default", Some(ct)) => {
                    if let Some(ext) = n.get("Extension") {
                        types
                            .defaults
                            .push((ext.to_ascii_lowercase(), ct.to_owned()));
                    }
                }
                ("Override", Some(ct)) => {
                    if let Some(name) = n.get("PartName") {
                        types
                            .overrides
                            .push((name.trim_start_matches('/').to_owned(), ct.to_owned()));
                    }
                }
                _ => {}
            }
        }
        Ok(types)
    }

    /// The content type of a part, if declared.
    #[must_use]
    pub fn of(&self, part: &str) -> Option<&str> {
        if let Some((_, ct)) = self.overrides.iter().find(|(n, _)| n == part) {
            return Some(ct);
        }
        let ext = part.rsplit_once('.')?.1.to_ascii_lowercase();
        self.defaults
            .iter()
            .find(|(e, _)| *e == ext)
            .map(|(_, ct)| ct.as_str())
    }
}
