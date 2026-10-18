//! Block containers (a section's body, a cell, a header, a footnote): paragraphs, tables, and
//! runs of list paragraphs nested into `text:list` / `text:list-item` by level, the way
//! `LibreOffice` nests them, each list naming its instance through `xml:id` and
//! `text:continue-list`.

use super::line::Story;
use super::para::SectionStart;
use super::Writer;
use cdoc_model::{AbstractNumId, Block, NumId, Paragraph};
use cdoc_pkg::xml::Node;
use cdoc_pkg::PackageError;

/// One list item's level and the paragraph nodes it holds.
type Item = (u8, Vec<Node>);

fn nest(items: &[Item], depth: u8) -> Vec<Node> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < items.len() {
        let mut item = Node::new("text:list-item");
        if items[i].0 == depth {
            item = item.children(items[i].1.iter().cloned());
            i += 1;
        }
        let deeper = items[i..]
            .iter()
            .take_while(|(level, _)| *level > depth)
            .count();
        if deeper > 0 {
            item =
                item.child(Node::new("text:list").children(nest(&items[i..i + deeper], depth + 1)));
            i += deeper;
        }
        out.push(item);
    }
    out
}

/// The list style name of definition `index` (0-based): names follow the definitions' order,
/// so two definitions sharing an id still get distinct styles.
#[must_use]
pub fn list_style_name(index: usize) -> String {
    format!("L{}", index + 1)
}

impl Writer<'_> {
    /// The instance and definition a paragraph's list membership resolves to.
    fn list_of(&mut self, p: &Paragraph) -> Option<(NumId, AbstractNumId)> {
        let r = p.properties.numbering?;
        let numbering = &self.doc.numbering;
        let found = numbering
            .instances
            .iter()
            .find(|i| i.id == r.num_id)
            .and_then(|i| {
                let def = numbering
                    .definitions
                    .iter()
                    .find(|d| d.id == i.abstract_id)?;
                (usize::from(r.level) < def.levels.len()).then_some((i.id, def.id))
            });
        if found.is_none() {
            self.log
                .lose("a paragraph in a list that is not defined; written without its list");
        }
        found
    }

    /// The `text:style-name` for definition `id`.
    pub(super) fn list_style(&self, id: AbstractNumId) -> Option<String> {
        let index = self
            .doc
            .numbering
            .definitions
            .iter()
            .position(|d| d.id == id)?;
        Some(list_style_name(index))
    }

    fn list(&mut self, num: NumId, def: AbstractNumId, items: &[Item]) -> Node {
        self.lists += 1;
        let id = format!("list{}", self.lists);
        let mut node = Node::new("text:list").attr("xml:id", &id);
        if let Some(style) = self.list_style(def) {
            node = node.attr("text:style-name", &style);
        }
        match self.first_list.get(&num) {
            Some(first) => node = node.attr("text:continue-list", first),
            None => {
                self.first_list.insert(num, id.clone());
            }
        }
        self.sidecar.lists.insert(id, num);
        node.children(nest(items, 0))
    }

    /// Write a container's blocks; the first carries `start` when the container is a section.
    pub(super) fn container(
        &mut self,
        blocks: &[Block],
        start: Option<&SectionStart>,
    ) -> Result<Vec<Node>, PackageError> {
        let mut story = Story::new();
        let mut out = Vec::new();
        let closes = |k: usize| !matches!(blocks.get(k + 1), Some(Block::Paragraph(_)));
        let mut i = 0;
        while i < blocks.len() {
            let here = if i == 0 { start } else { None };
            let p = match &blocks[i] {
                Block::Table(t) => {
                    out.push(self.table(t, here)?);
                    i += 1;
                    continue;
                }
                Block::Paragraph(p) => p,
            };
            let Some((num, def)) = self.list_of(p) else {
                out.extend(self.paragraph(p, &mut story, here, closes(i))?);
                i += 1;
                continue;
            };
            let mut items: Vec<Item> = Vec::new();
            while let Some(Block::Paragraph(q)) = blocks.get(i) {
                if q.properties.numbering.map(|r| r.num_id) != Some(num)
                    || self.list_of(q).is_none()
                {
                    break;
                }
                let level = q.properties.numbering.map_or(0, |r| r.level);
                let here = if i == 0 { start } else { None };
                items.push((level, self.paragraph(q, &mut story, here, closes(i))?));
                i += 1;
            }
            out.push(self.list(num, def, &items));
        }
        Ok(out)
    }
}
