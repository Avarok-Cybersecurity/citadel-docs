//! Flags as `--name value`, every one required unless a command says otherwise. No defaults: a
//! missing flag stops the run naming it.

use std::collections::BTreeMap;

/// Parsed flags; a flag may repeat.
pub struct Args {
    flags: BTreeMap<String, Vec<String>>,
}

impl Args {
    /// Parses `--name value` pairs.
    pub fn parse(raw: &[String]) -> Result<Self, String> {
        let mut flags: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut it = raw.iter();
        while let Some(flag) = it.next() {
            let name = flag
                .strip_prefix("--")
                .ok_or_else(|| format!("expected a --flag, found {flag:?}"))?;
            let value = it.next().ok_or_else(|| format!("--{name} needs a value"))?;
            flags
                .entry(name.to_owned())
                .or_default()
                .push(value.clone());
        }
        Ok(Self { flags })
    }

    /// The one value of `--name`.
    pub fn one(&self, name: &str) -> Result<&str, String> {
        match self.flags.get(name).map(Vec::as_slice) {
            Some([v]) => Ok(v),
            Some(_) => Err(format!("--{name} given more than once")),
            None => Err(format!("missing --{name}")),
        }
    }

    /// Every value of `--name` (at least one).
    pub fn many(&self, name: &str) -> Result<&[String], String> {
        self.flags
            .get(name)
            .map(Vec::as_slice)
            .ok_or_else(|| format!("missing --{name}"))
    }

    /// `--pages 1,10,100` as numbers.
    pub fn pages(&self) -> Result<Vec<u32>, String> {
        self.one("pages")?
            .split(',')
            .map(|p| {
                p.trim()
                    .parse::<u32>()
                    .map_err(|e| format!("--pages {p:?}: {e}"))
            })
            .collect()
    }
}
