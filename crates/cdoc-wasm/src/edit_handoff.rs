//! The editor's document handed to another artefact as bytes (ARCHITECTURE.md, "WASM
//! artefacts"): its model JSON, and the bytes of the images its pages draw. The PDF artefact
//! turns the two back into the same PDF the full binary writes from the live replica, because
//! its layout is the same code over the same document and faces.

use crate::edit_bindings::EditorHandle;
use crate::edit_facade::EditSession;
use crate::{convert::json, ImageStore};
use cdoc_docx::MemoryMedia;
use cdoc_layout::Draw;
use cdoc_model::ImagePort;
use wasm_bindgen::prelude::*;

impl EditSession {
    /// The live replica's document as model JSON.
    #[must_use]
    pub fn model_json(&self) -> String {
        json(self.0.model())
    }

    /// Copies into `store` every image the editor holds that its pages draw, by content id.
    /// An image it draws but does not hold (the content store has it) is the caller's to add.
    pub fn drawn_images(&self, store: &mut MemoryMedia) {
        let drawn = self.0.layout().pages.iter().flat_map(|p| &p.draws);
        for draw in drawn {
            if let Draw::Image { content_id, .. } = draw {
                if let Ok(image) = self.0.images().image(content_id) {
                    store.insert(content_id.clone(), image.mime, image.bytes.to_vec());
                }
            }
        }
    }
}

#[wasm_bindgen]
impl EditorHandle {
    /// The document as model JSON, for an artefact that does not edit (the PDF artefact:
    /// `model_to_pm`, then `pm_to_pdf`).
    #[wasm_bindgen(js_name = modelJson)]
    #[must_use]
    pub fn model_json(&self) -> String {
        self.0.model_json()
    }

    /// Adds to `images` every image this editor holds that its pages draw, for the PDF
    /// artefact's `RenderAssets.addImage`.
    #[wasm_bindgen(js_name = drawnImages)]
    pub fn drawn_images(&self, images: &mut ImageStore) {
        self.0.drawn_images(&mut images.0);
    }
}
