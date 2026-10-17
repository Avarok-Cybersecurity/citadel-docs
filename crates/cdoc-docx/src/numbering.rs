//! `word/numbering.xml`: abstract definitions, their levels, and numbered instances.

use crate::props::{
    alignment_from, alignment_name, num, number_format_from, number_format_name, read_indent,
    read_rpr, write_indent, write_rpr,
};
use crate::report::Log;
use crate::xml::{Node, WordNode};
use cdoc_model::{
    AbstractNumId, AbstractNumbering, Alignment, Level, LevelOverride, NumId, NumberFormat,
    Numbering, NumberingInstance,
};

/// `w:numbering`: every definition, then every instance, as the schema requires.
#[must_use]
pub fn write_numbering(numbering: &Numbering, log: &mut Log) -> Node {
    let definitions = numbering.definitions.iter().map(|d| {
        let levels = d
            .levels
            .iter()
            .enumerate()
            .map(|(i, l)| write_level(i, l, log));
        Node::new("w:abstractNum")
            .num("w:abstractNumId", d.id.0)
            .child(Node::val("w:multiLevelType", "multilevel"))
            .children(levels.collect::<Vec<_>>())
    });
    let instances = numbering.instances.iter().map(|i| {
        let overrides = i.overrides.iter().map(|o| {
            Node::new("w:lvlOverride")
                .num("w:ilvl", o.level)
                .child(Node::val("w:startOverride", &o.start.to_string()))
        });
        Node::new("w:num")
            .num("w:numId", i.id.0)
            .child(Node::val("w:abstractNumId", &i.abstract_id.0.to_string()))
            .children(overrides)
    });
    Node::new("w:numbering")
        .children(definitions.collect::<Vec<_>>())
        .children(instances)
}

fn write_level(index: usize, level: &Level, log: &mut Log) -> Node {
    let mut node = Node::new("w:lvl")
        .num("w:ilvl", index)
        .child(Node::val("w:start", &level.start.to_string()))
        .child(Node::val("w:numFmt", number_format_name(level.format)))
        .child(Node::val("w:lvlText", &level.text))
        .child(Node::val("w:lvlJc", alignment_name(level.alignment)));
    if let Some(indent) = &level.indent {
        node = node.child(Node::new("w:pPr").child(write_indent(indent, log)));
    }
    if let Some(run) = &level.run {
        node = node.child(write_rpr(run));
    }
    node
}

/// Read `w:numbering`.
pub fn read_numbering(root: &Node, log: &mut Log) -> Numbering {
    let mut numbering = Numbering::empty();
    for child in root.elems() {
        match child.name.as_str() {
            "w:abstractNum" => numbering.definitions.extend(read_definition(child, log)),
            "w:num" => numbering.instances.extend(read_instance(child, log)),
            other => log.unknown(&format!("{}/{other}", root.name)),
        }
    }
    numbering
}

fn read_definition(node: &Node, log: &mut Log) -> Option<AbstractNumbering> {
    let id = AbstractNumId(num(node, "w:abstractNumId", log)?);
    let mut levels: Vec<(u8, Level)> = Vec::new();
    for child in node.elems() {
        match child.name.as_str() {
            "w:lvl" => levels.push((
                num(child, "w:ilvl", log).unwrap_or(0),
                read_level(child, log),
            )),
            "w:multiLevelType" => {}
            other => log.unknown(&format!("w:abstractNum/{other}")),
        }
    }
    levels.sort_by_key(|(i, _)| *i);
    if levels
        .iter()
        .enumerate()
        .any(|(pos, (i, _))| usize::from(*i) != pos)
    {
        log.lose("abstractNum levels are not consecutive from 0; renumbered");
    }
    Some(AbstractNumbering {
        id,
        levels: levels.into_iter().map(|(_, l)| l).collect(),
    })
}

fn read_level(node: &Node, log: &mut Log) -> Level {
    let mut level = Level {
        start: 0,
        format: NumberFormat::Decimal,
        text: String::new(),
        alignment: Alignment::Left,
        indent: None,
        run: None,
    };
    for child in node.elems() {
        match child.name.as_str() {
            "w:start" => level.start = num(child, "w:val", log).unwrap_or(0),
            "w:numFmt" => {
                level.format = child
                    .value()
                    .and_then(number_format_from)
                    .unwrap_or_else(|| {
                        log.lose("number format approximated as decimal");
                        NumberFormat::Decimal
                    });
            }
            "w:lvlText" => child
                .value()
                .unwrap_or_default()
                .clone_into(&mut level.text),
            "w:lvlJc" => {
                level.alignment = child
                    .value()
                    .and_then(alignment_from)
                    .unwrap_or(Alignment::Left);
            }
            "w:pPr" => level.indent = read_level_indent(child, log),
            "w:rPr" => level.run = Some(read_rpr(child, log)),
            other => log.unknown(&format!("w:lvl/{other}")),
        }
    }
    level
}

fn read_level_indent(ppr: &Node, log: &mut Log) -> Option<cdoc_model::Indent> {
    let indent = ppr.find("w:ind").map(|n| read_indent(n, log));
    let others = ppr.elems().filter(|n| n.name != "w:ind").count();
    if others > 0 {
        log.lose("w:lvl/w:pPr beyond w:ind");
    }
    indent
}

fn read_instance(node: &Node, log: &mut Log) -> Option<NumberingInstance> {
    let id = NumId(num(node, "w:numId", log)?);
    let mut abstract_id = None;
    let mut overrides = Vec::new();
    for child in node.elems() {
        match child.name.as_str() {
            "w:abstractNumId" => abstract_id = num(child, "w:val", log).map(AbstractNumId),
            "w:lvlOverride" => {
                let level = num(child, "w:ilvl", log).unwrap_or(0);
                match child
                    .find("w:startOverride")
                    .and_then(|n| num(n, "w:val", log))
                {
                    Some(start) => overrides.push(LevelOverride { level, start }),
                    None => log.lose("w:lvlOverride without w:startOverride"),
                }
            }
            other => log.unknown(&format!("w:num/{other}")),
        }
    }
    Some(NumberingInstance {
        id,
        abstract_id: abstract_id?,
        overrides,
    })
}
