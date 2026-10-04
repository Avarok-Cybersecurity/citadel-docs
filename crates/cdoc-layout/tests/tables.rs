//! Tables: grid widths, spans, borders, shading, vertical alignment and header rows repeated
//! across pages.

mod support;

use cdoc_layout::Draw;
use cdoc_model::{
    Border, BorderStyle, BorderWidth, Borders, CellProperties, Color, Length, Shading, Table,
    TableCell, TableProperties, TableRow, TableWidth,
};

fn cell(text: &str, col_span: u32, header: bool) -> TableCell {
    TableCell {
        properties: CellProperties {
            col_span,
            ..CellProperties::single()
        },
        header,
        blocks: vec![support::para(text)],
    }
}

fn row(cells: Vec<TableCell>) -> TableRow {
    TableRow {
        height: None,
        cant_split: false,
        cells,
    }
}

fn table(width: TableWidth, rows: Vec<TableRow>) -> Table {
    Table {
        properties: TableProperties {
            style: None,
            width,
            alignment: None,
            borders: Some(Borders::all(Border {
                style: BorderStyle::Single,
                width: BorderWidth(8),
                color: Some(Color::rgb(0, 0, 0)),
                space: Length(0),
            })),
            cell_padding: Some(Length(500)),
        },
        column_widths: vec![Length(1000), Length(3000)],
        rows,
    }
}

const LEFT: i32 = 7200;
const WIDTH: i32 = 59530 - 14400;

fn x_of(layout: &cdoc_layout::Layout, text: &str) -> i32 {
    layout.pages[0]
        .runs()
        .find(|r| r.text == text)
        .map(|r| r.x)
        .expect("placed")
}

#[test]
fn grid_columns_scale_to_a_full_width_table_and_cells_sit_inside_their_padding() {
    let t = table(
        TableWidth::Percent { fiftieths: 5000 },
        vec![row(vec![cell("a", 1, false), cell("b", 1, false)])],
    );
    let layout = support::laid_out(&support::doc_with(vec![cdoc_model::Block::Table(t)]));
    assert_eq!(x_of(&layout, "a"), LEFT + 500);
    assert_eq!(
        x_of(&layout, "b"),
        LEFT + WIDTH / 4 + 500,
        "1:3 grid over the full width"
    );
}

#[test]
fn a_spanning_cell_covers_both_columns_and_borders_box_every_cell() {
    let t = table(
        TableWidth::Fixed {
            length: Length(40_000),
        },
        vec![
            row(vec![cell("wide", 2, false)]),
            row(vec![cell("a", 1, false), cell("b", 1, false)]),
        ],
    );
    let layout = support::laid_out(&support::doc_with(vec![cdoc_model::Block::Table(t)]));
    let rules: Vec<_> = layout.pages[0]
        .draws
        .iter()
        .filter_map(|d| match d {
            Draw::Rule {
                from, to, width, ..
            } => Some((*from, *to, *width)),
            _ => None,
        })
        .collect();
    assert_eq!(rules.len(), 12, "four edges round each of three cells");
    assert!(rules.iter().all(|r| r.2 == 100), "1 pt rules");
    let top = rules[0];
    assert_eq!(
        (top.0 .0, top.1 .0),
        (LEFT, LEFT + 40_000),
        "the spanning cell's top edge is the table's width"
    );
    assert_eq!(x_of(&layout, "b"), LEFT + 10_000 + 500);
}

#[test]
fn shading_fills_behind_the_cell_text() {
    let mut c = cell("shaded", 1, false);
    c.properties.shading = Some(Shading {
        fill: Color::rgb(0xEE, 0xEE, 0xEE),
    });
    let t = table(
        TableWidth::Percent { fiftieths: 5000 },
        vec![row(vec![c, cell("plain", 1, false)])],
    );
    let layout = support::laid_out(&support::doc_with(vec![cdoc_model::Block::Table(t)]));
    let draws = &layout.pages[0].draws;
    let fill = draws.iter().position(
        |d| matches!(d, Draw::Fill { colour, .. } if *colour == Color::rgb(0xEE, 0xEE, 0xEE)),
    );
    let text = draws
        .iter()
        .position(|d| matches!(d, Draw::Text(r) if r.text == "shaded"));
    assert!(fill.expect("fill") < text.expect("text"), "painted behind");
}

#[test]
fn header_rows_repeat_at_the_top_of_each_continuation_page() {
    let mut rows = vec![row(vec![cell("Head", 2, true)])];
    rows.extend((0..80).map(|i| row(vec![cell(&format!("r{i}"), 1, false), cell("x", 1, false)])));
    let t = table(TableWidth::Auto, rows);
    let layout = support::laid_out(&support::doc_with(vec![cdoc_model::Block::Table(t)]));
    assert!(layout.pages.len() >= 2);
    for page in &layout.pages {
        let first = page.runs().next().expect("runs");
        assert_eq!(first.text, "Head", "each page opens with the header row");
    }
}

#[test]
fn rows_of_a_long_table_never_run_into_the_bottom_margin() {
    let rows = (0..80)
        .map(|i| row(vec![cell(&format!("r{i}"), 1, false), cell("x", 1, false)]))
        .collect();
    let t = table(TableWidth::Percent { fiftieths: 5000 }, rows);
    let layout = support::laid_out(&support::doc_with(vec![cdoc_model::Block::Table(t)]));
    assert!(layout.pages.len() >= 2);
    for page in &layout.pages {
        assert!(
            page.runs().all(|r| r.y <= page.height - 7200),
            "a row ran into the margin"
        );
    }
}
