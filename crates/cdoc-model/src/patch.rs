//! Field-wise changes: what the ribbons send. Every field of a patch has three states, which
//! the JSON data model (and the binary codec) carries as absent, a value and `null`: keep the
//! current value, set it, clear it (back to what the style gives).

use serde::{Deserialize, Deserializer};

/// Keep (`None`), set (`Some(Some(v))`) or clear (`Some(None)`, `null` in the data model).
pub type Change<T> = Option<Option<T>>;

/// Reads a present field, `null` included, as a change; an absent one is `None` by `default`.
pub(crate) fn present<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Change<T>, D::Error> {
    Option::<T>::deserialize(d).map(Some)
}

/// The change that sets `v` when it is present and keeps the old value otherwise.
pub(crate) fn set<T: Clone>(v: Option<&T>) -> Change<T> {
    v.cloned().map(Some)
}

pub(crate) fn apply<T: Clone>(change: Option<&Option<T>>, old: Option<&T>) -> Option<T> {
    change.map_or_else(|| old.cloned(), Clone::clone)
}

/// Declares a patch struct whose every field is a [`Change`].
macro_rules! patch {
    ($(#[$doc:meta])* $name:ident { $($(#[$fdoc:meta])* $field:ident: $ty:ty,)* }) => {
        $(#[$doc])*
        #[derive(
            Debug, Clone, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
        )]
        #[serde(rename_all = "camelCase")]
        pub struct $name {
            $(
                $(#[$fdoc])*
                #[serde(default, deserialize_with = "crate::patch::present", skip_serializing_if = "Option::is_none")]
                pub $field: $crate::patch::Change<$ty>,
            )*
        }
    };
}

pub(crate) use patch;
