//! Typographic run settings Word stores beside the character formatting: which OpenType
//! ligatures a run uses (`w14:ligatures`).

use serde::{Deserialize, Serialize};

/// The ligature sets a run uses: Word's `w14:ligatures` values, spelled as OOXML spells them.
/// Each combines standard (`liga`), contextual (`clig`), historical (`hlig`) and
/// discretional (`dlig`) ligatures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Ligatures {
    /// No ligatures (explicitly, overriding an inherited setting).
    None,
    /// Standard only.
    Standard,
    /// Contextual only.
    Contextual,
    /// Historical only.
    Historical,
    /// Discretional only.
    Discretional,
    /// Standard and contextual: what Word sets on a new document.
    StandardContextual,
    /// Standard and historical.
    StandardHistorical,
    /// Contextual and historical.
    ContextualHistorical,
    /// Standard and discretional.
    StandardDiscretional,
    /// Contextual and discretional.
    ContextualDiscretional,
    /// Historical and discretional.
    HistoricalDiscretional,
    /// Standard, contextual and historical.
    StandardContextualHistorical,
    /// Standard, contextual and discretional.
    StandardContextualDiscretional,
    /// Standard, historical and discretional.
    StandardHistoricalDiscretional,
    /// Contextual, historical and discretional.
    ContextualHistoricalDiscretional,
    /// All four.
    All,
}

impl Ligatures {
    /// Every value, in OOXML's order.
    pub const ALL: [Self; 16] = [
        Self::None,
        Self::Standard,
        Self::Contextual,
        Self::Historical,
        Self::Discretional,
        Self::StandardContextual,
        Self::StandardHistorical,
        Self::ContextualHistorical,
        Self::StandardDiscretional,
        Self::ContextualDiscretional,
        Self::HistoricalDiscretional,
        Self::StandardContextualHistorical,
        Self::StandardContextualDiscretional,
        Self::StandardHistoricalDiscretional,
        Self::ContextualHistoricalDiscretional,
        Self::All,
    ];

    /// The OOXML spelling (`standardContextual`).
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Standard => "standard",
            Self::Contextual => "contextual",
            Self::Historical => "historical",
            Self::Discretional => "discretional",
            Self::StandardContextual => "standardContextual",
            Self::StandardHistorical => "standardHistorical",
            Self::ContextualHistorical => "contextualHistorical",
            Self::StandardDiscretional => "standardDiscretional",
            Self::ContextualDiscretional => "contextualDiscretional",
            Self::HistoricalDiscretional => "historicalDiscretional",
            Self::StandardContextualHistorical => "standardContextualHistorical",
            Self::StandardContextualDiscretional => "standardContextualDiscretional",
            Self::StandardHistoricalDiscretional => "standardHistoricalDiscretional",
            Self::ContextualHistoricalDiscretional => "contextualHistoricalDiscretional",
            Self::All => "all",
        }
    }

    /// The value an OOXML spelling names.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|l| l.name() == name)
    }

    /// Whether standard ligatures (`liga`: fi, fl, ff) are on.
    #[must_use]
    pub fn standard(self) -> bool {
        self == Self::All || self.name().starts_with("standard")
    }

    /// Whether contextual ligatures (`clig`) are on.
    #[must_use]
    pub fn contextual(self) -> bool {
        self == Self::All || self.name().contains("ontextual")
    }
}

#[cfg(test)]
mod tests {
    use super::Ligatures;

    #[test]
    fn names_round_trip_and_name_their_sets() {
        for l in Ligatures::ALL {
            assert_eq!(Ligatures::from_name(l.name()), Some(l));
        }
        let standard: Vec<_> = Ligatures::ALL
            .into_iter()
            .filter(|l| l.standard())
            .collect();
        assert_eq!(standard.len(), 8, "{standard:?}");
        let contextual = Ligatures::ALL
            .into_iter()
            .filter(|l| l.contextual())
            .count();
        assert_eq!(contextual, 8);
        assert!(!Ligatures::HistoricalDiscretional.standard());
        assert!(Ligatures::StandardContextual.standard() && Ligatures::All.contextual());
    }
}
