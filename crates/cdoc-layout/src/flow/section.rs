//! Where a section starts: on the same page (continuous, a new column region below what is
//! there), on the next page, or on the next odd or even page (with a blank page between when
//! the parity is wrong).

use super::Flow;
use crate::context::Ctx;
use crate::geometry::Geometry;
use crate::ir::Page;
use crate::LayoutError;

impl Flow {
    /// Starts section `index` with its own geometry, after the break its start kind asks for.
    pub fn start_section(
        &mut self,
        ctx: &mut Ctx<'_, '_>,
        index: usize,
        geometry: Geometry,
        start: cdoc_model::SectionStart,
    ) -> Result<(), LayoutError> {
        use cdoc_model::SectionStart;
        if start == SectionStart::Continuous && geometry.page_width == self.geometry.page_width {
            self.finish_column(ctx)?;
            self.finish_region();
            self.region_top = self.region_reach.max(self.y);
            self.region_reach = self.region_top;
            self.y = self.region_top;
            self.column = 0;
            self.geometry = geometry;
            return Ok(());
        }
        if self.page_started() {
            self.new_page(ctx)?;
        }
        let parity = match start {
            SectionStart::OddPage => Some(1),
            SectionStart::EvenPage => Some(0),
            SectionStart::NextPage | SectionStart::Continuous => None,
        };
        if parity.is_some_and(|p| (self.sheets.len() % 2) != p) {
            self.new_page(ctx)?;
        }
        self.geometry = geometry;
        let sheet = self.sheets.last_mut().expect("never empty");
        sheet.page = Page::new(self.geometry.page_width, self.geometry.page_height);
        sheet.section = index;
        sheet.first_of_section = true;
        self.region_top = self.geometry.top;
        self.region_reach = self.region_top;
        self.y = self.region_top;
        Ok(())
    }
}
