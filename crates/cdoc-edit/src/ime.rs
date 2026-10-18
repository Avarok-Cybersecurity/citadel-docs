//! Input-method composition. **Choice: a local overlay.** While a composition is open its text
//! is not in the CRDT: peers never see provisional text, the history never holds it, and a
//! cancelled composition leaves nothing to undo. The UI draws [`Composition::text`] at the
//! caret rectangle the editor reports (underlined, as platforms do) and the layout is not
//! redone per composition keystroke; [`crate::Editor::composition_commit`] inserts the final
//! text as one ordinary typing command. The cost is that text after the caret does not reflow
//! around the provisional text until commit, which matches what most canvas editors accept.

use crate::position::Position;

/// An open composition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Composition {
    /// Where the composed text will be inserted (anchored like any position).
    pub at: Position,
    /// The provisional text.
    pub text: String,
    /// The caret inside the provisional text, in characters.
    pub caret: usize,
}
