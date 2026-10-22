//! Bullet and ordered lists into flat numbered paragraphs: the nesting depth is the level, an
//! item's first paragraph carries its number.

use crate::blocks::{Ctx, ListAt, Reader};
use crate::node::{whole, Node};
use cdoc_model::{NumberFormat, NumberingRef, MAX_LEVEL};
use serde_json::Value;

impl Reader {
    pub(crate) fn list(&mut self, node: Node<'_>, ctx: Ctx) {
        let ordered = node.kind == "orderedList";
        self.check_attrs(node, if ordered { &["start", "type"] } else { &[] });
        if ctx.quote && ctx.list.is_none() {
            self.lose(
                "a heading, rule, list or code block inside a block quote (the quote not kept)",
            );
        }
        self.number_pending(true);
        let depth = ctx.list.map_or(0, |outer| outer.depth.saturating_add(1));
        let level = match u8::try_from(depth) {
            Ok(level) if level <= MAX_LEVEL => level,
            _ => {
                self.lose("a list level beyond the ninth or out of sequence");
                MAX_LEVEL
            }
        };
        let num_id = if ordered {
            let format = self.list_format(node);
            let start = self.list_start(node);
            self.lists.ordered_list(format, level, start)
        } else {
            self.lists.bullet_list()
        };
        let list = ListAt {
            num_id,
            level,
            depth,
        };
        let inner = Ctx {
            quote: false,
            list: Some(list),
        };
        for value in self.children(node) {
            match Node::of(value) {
                Some(item) if item.kind == "listItem" => self.item(item, list, inner),
                Some(other) => self.block(other, inner),
                None => self.lose("a malformed node (skipped)"),
            }
        }
        self.flush_loose(inner);
    }

    fn item(&mut self, item: Node<'_>, list: ListAt, ctx: Ctx) {
        self.check_attrs(item, &[]);
        self.number = Some(NumberingRef {
            num_id: list.num_id,
            level: list.level,
        });
        self.blocks(item, ctx);
        // An empty item is an empty numbered paragraph, as the editor showed it.
        self.number_pending(false);
    }

    fn list_start(&mut self, node: Node<'_>) -> u32 {
        let Some(value) = node.attr("start") else {
            return 1;
        };
        if let Some(start) = whole(value).and_then(|n| u32::try_from(n).ok()) {
            return start;
        }
        self.lose("a list start below 0 or not a whole number (read as 1)");
        1
    }

    fn list_format(&mut self, node: Node<'_>) -> NumberFormat {
        let Some(value) = node.attr("type") else {
            return NumberFormat::Decimal;
        };
        let format = match value {
            Value::String(s) => match s.as_str() {
                "1" => Some(NumberFormat::Decimal),
                "a" => Some(NumberFormat::LowerLetter),
                "A" => Some(NumberFormat::UpperLetter),
                "i" => Some(NumberFormat::LowerRoman),
                "I" => Some(NumberFormat::UpperRoman),
                _ => None,
            },
            Value::Number(n) if n.as_u64() == Some(1) => Some(NumberFormat::Decimal),
            _ => None,
        };
        format.unwrap_or_else(|| {
            let shown = match value {
                Value::String(s) => crate::name(s),
                other => crate::name(&other.to_string()),
            };
            self.lose(&format!("list number format {shown} read as decimal"));
            NumberFormat::Decimal
        })
    }
}
