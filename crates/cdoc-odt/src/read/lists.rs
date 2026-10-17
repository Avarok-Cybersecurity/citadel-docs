//! Lists: each top-level `text:list` belongs to an instance (named in the sidecar, or by
//! `text:continue-list` / `text:continue-numbering`, or new), each paragraph takes the list's
//! nesting depth as its level, clamped to the levels its definition has.

use super::body::Blocks;
use super::ctx::Story;
use super::Reader;
use cdoc_model::{NumId, NumberingInstance, NumberingRef, MAX_LEVEL};
use cdoc_pkg::xml::Node;
use cdoc_pkg::PackageError;

impl Reader<'_> {
    /// The instance a top-level `text:list` belongs to.
    fn instance(&mut self, n: &Node) -> Option<NumId> {
        let xml_id = n.get("xml:id");
        if self.sidecar.instances.is_some() {
            if let Some(num) = xml_id.and_then(|id| self.sidecar.lists.get(id)) {
                return Some(*num);
            }
        }
        let style = n.get("text:style-name");
        let Some(abstract_id) = style.and_then(|s| self.abstract_for(s)) else {
            self.log
                .lose("a list without a list style read without numbering");
            return None;
        };
        let continued = n
            .get("text:continue-list")
            .and_then(|c| self.list_nums.get(c))
            .copied()
            .or_else(|| {
                (n.get("text:continue-numbering") == Some("true"))
                    .then(|| style.and_then(|s| self.last_list.get(s)).copied())?
            });
        let num = continued.unwrap_or_else(|| {
            let id = NumId(u32::try_from(self.instances.len() + 1).unwrap_or(u32::MAX));
            self.instances.push(NumberingInstance {
                id,
                abstract_id,
                overrides: Vec::new(),
            });
            id
        });
        if let Some(id) = xml_id {
            self.list_nums.insert(id.to_owned(), num);
        }
        if let Some(s) = style {
            self.last_list.insert(s.to_owned(), num);
        }
        Some(num)
    }

    fn level(&mut self, num: NumId, depth: u8) -> u8 {
        let defined = self
            .instances
            .iter()
            .chain(self.sidecar.instances.iter().flatten())
            .find(|i| i.id == num)
            .and_then(|i| self.level_counts.get(&i.abstract_id))
            .copied()
            .unwrap_or(1);
        let top = u8::try_from(defined.saturating_sub(1))
            .unwrap_or(MAX_LEVEL)
            .min(MAX_LEVEL);
        if depth > top {
            self.log
                .lose("a list nested deeper than its style defines read at the deepest level");
        }
        depth.min(top)
    }

    pub(super) fn list(
        &mut self,
        n: &Node,
        story: &mut Story,
        out: &mut Blocks,
        depth: u8,
        num: Option<NumId>,
    ) -> Result<(), PackageError> {
        let num = if depth == 0 { self.instance(n) } else { num };
        for item in n.elems() {
            let header = match item.name.as_str() {
                "text:list-item" => false,
                "text:list-header" => true,
                other => {
                    self.log.lose(&format!("{other} in a list"));
                    continue;
                }
            };
            if header {
                self.log
                    .lose("a list header read as paragraphs without numbers");
            }
            if item.get("text:start-value").is_some() {
                self.log.lose("a list item's restart value");
            }
            let mut paragraphs = 0;
            for c in item.elems() {
                match c.name.as_str() {
                    "text:list" => self.list(c, story, out, depth.saturating_add(1), num)?,
                    "text:p" | "text:h" => {
                        let split = c
                            .get("text:style-name")
                            .is_some_and(|s| self.sidecar.continuations.contains_key(s));
                        paragraphs += usize::from(!split);
                        if paragraphs == 2 && !split && !header {
                            self.log.lose(
                                "a list item's further paragraphs read as items of their own",
                            );
                        }
                        let numbering = num.filter(|_| !header).map(|num_id| NumberingRef {
                            num_id,
                            level: self.level(num_id, depth),
                        });
                        self.paragraph(c, story, out, numbering)?;
                    }
                    _ => self.block(c, story, out)?,
                }
            }
        }
        Ok(())
    }
}
