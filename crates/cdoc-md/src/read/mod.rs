//! Markdown to model, as a walk over `pulldown-cmark` events.

mod blocks;
mod events;
mod inline;
mod table;

use crate::options::MdReadOptions;
use crate::write::PART;
use cdoc_model::{
    Alignment, Block, Document, Footnote, Inline, NumId, Paragraph, ParagraphProperties, Report,
    StyleId,
};
use pulldown_cmark::{Event, Options, Parser};
use table::TableBuild;

/// The paragraph being filled.
enum Pending {
    Paragraph,
    Heading(u8),
    Cell(Option<Alignment>),
}

/// An open list.
struct ListState {
    num_id: NumId,
    level: u8,
}

/// Inline formatting in force.
struct Format {
    bold: u32,
    italic: u32,
    strike: u32,
    link: Option<cdoc_model::Link>,
    image: Option<(String, String)>,
}

struct Reader<'a> {
    opts: &'a MdReadOptions,
    report: Report,
    out: Vec<Block>,
    saved: Vec<Vec<Block>>,
    inlines: Vec<Inline>,
    pending: Option<Pending>,
    quote_depth: u32,
    lists: Vec<ListState>,
    list_builder: cdoc_model::builtin::UniformLists,
    item_first: bool,
    fmt: Format,
    table: Option<TableBuild>,
    footnotes: Vec<Footnote>,
    named_labels: Vec<String>,
    in_footnote: bool,
    current_footnote: Option<String>,
    code: Option<String>,
    html: bool,
}

impl Reader<'_> {
    fn lose(&mut self, what: &str) {
        self.report.note(PART, what);
    }

    fn style(id: &str) -> StyleId {
        StyleId::new(id)
    }

    fn properties(&mut self, pending: &Pending) -> ParagraphProperties {
        let mut props = ParagraphProperties::none();
        match pending {
            Pending::Heading(n) => props.style = Some(Self::style(&format!("Heading{n}"))),
            Pending::Cell(alignment) => props.alignment = *alignment,
            Pending::Paragraph => {
                props.style = Some(Self::style("Normal"));
                if self.in_footnote {
                    props.style = Some(Self::style("FootnoteText"));
                }
                if self.quote_depth > 0 {
                    props.style = Some(Self::style("Quote"));
                }
                if let Some(list) = self.lists.last() {
                    props.style = Some(Self::style("ListParagraph"));
                    if self.item_first {
                        props.numbering = Some(cdoc_model::NumberingRef {
                            num_id: list.num_id,
                            level: list.level,
                        });
                        self.item_first = false;
                    }
                }
            }
        }
        props
    }

    fn begin(&mut self, pending: Pending) {
        self.flush();
        self.pending = Some(pending);
    }

    fn ensure_paragraph(&mut self) {
        if self.pending.is_none() {
            self.pending = Some(Pending::Paragraph);
        }
    }

    /// Finish the open paragraph, if any, into the current container.
    fn flush(&mut self) {
        let Some(pending) = self.pending.take() else {
            return;
        };
        let properties = self.properties(&pending);
        let inlines = std::mem::take(&mut self.inlines);
        match pending {
            Pending::Cell(_) => self.cell_paragraph(Paragraph {
                properties,
                inlines,
            }),
            _ => self.out.push(Block::Paragraph(Paragraph {
                properties,
                inlines,
            })),
        }
    }

    fn event(&mut self, event: Event<'_>) {
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(end) => self.end(end),
            Event::Text(t) => self.text(&t, false),
            Event::Code(t) => self.text(&t, true),
            Event::SoftBreak => self.text(" ", false),
            Event::HardBreak => self.line_break(),
            Event::InlineHtml(h) => self.inline_html(&h),
            Event::Html(_) => {}
            Event::FootnoteReference(label) => self.footnote_ref(&label),
            Event::Rule => self.rule(),
            _ => self.lose("construct this reader does not model"),
        }
    }
}

/// Read Markdown (`CommonMark` with GFM tables, strikethrough and footnotes). The report names
/// each kind of construct the model does not hold.
#[must_use]
pub fn md_to_document(md: &str, opts: &MdReadOptions) -> (Document, Report) {
    let mut reader = Reader {
        opts,
        report: Report::empty(),
        out: Vec::new(),
        saved: Vec::new(),
        inlines: Vec::new(),
        pending: None,
        quote_depth: 0,
        lists: Vec::new(),
        list_builder: cdoc_model::builtin::UniformLists::new(),
        item_first: false,
        fmt: Format {
            bold: 0,
            italic: 0,
            strike: 0,
            link: None,
            image: None,
        },
        table: None,
        footnotes: Vec::new(),
        named_labels: Vec::new(),
        in_footnote: false,
        current_footnote: None,
        code: None,
        html: false,
    };
    let options =
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_FOOTNOTES;
    for event in Parser::new_ext(md, options) {
        reader.event(event);
    }
    reader.flush();
    let mut doc = Document::blank();
    if !reader.out.is_empty() {
        doc.sections[0].blocks = reader.out;
    }
    doc.numbering = reader.list_builder.finish();
    doc.footnotes = reader.footnotes;
    doc.normalize();
    (doc, reader.report)
}
