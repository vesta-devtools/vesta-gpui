use std::ops::Range;

use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use vesta_gpui::StyledTextSpan;

use super::MarkdownError;
use super::table::Cell;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) struct Link {
    pub range: Range<usize>,
    pub destination: String,
}

#[derive(Default, PartialEq, Eq, Hash)]
pub(super) struct Content {
    pub text: String,
    pub spans: Vec<StyledTextSpan>,
    pub marker: String,
    pub heading: u8,
    pub code: bool,
    pub quote: bool,
    pub indent: usize,
    pub tooltip: String,
    pub links: Vec<Link>,
    pub cell: Option<Cell>,
}

#[derive(Default)]
struct Builder {
    blocks: Vec<Content>,
    current: Content,
    lists: Vec<Option<u64>>,
    quote: usize,
    bold: usize,
    italic: usize,
    links: Vec<(usize, String)>,
    table: Option<Table>,
}

struct Table {
    alignments: Vec<pulldown_cmark::Alignment>,
    column: usize,
    header: bool,
}

impl Builder {
    fn flush(&mut self) {
        if !self.current.text.is_empty()
            || !self.current.marker.is_empty()
            || self.current.cell.is_some()
        {
            self.blocks.push(std::mem::take(&mut self.current));
        }
    }

    fn append(&mut self, text: &str, mono: bool) {
        let start = self.current.text.len();
        self.current.text.push_str(text);
        self.current.quote = self.quote > 0;
        self.current.indent = self.lists.len().saturating_sub(1);
        self.current.spans.push(StyledTextSpan {
            range: start..self.current.text.len(),
            bold: self.bold > 0
                || self.current.heading > 0
                || self.current.cell.is_some_and(|cell| cell.header),
            italic: self.italic > 0,
            monospace: mono,
        });
    }
}

pub(super) fn parse(text: &str) -> Result<Vec<Content>, MarkdownError> {
    let mut builder = Builder::default();
    for event in Parser::new_ext(text, Options::ENABLE_TASKLISTS | Options::ENABLE_TABLES) {
        match event {
            Event::Start(Tag::Table(alignments)) => {
                builder.flush();
                builder.table = Some(Table {
                    alignments,
                    column: 0,
                    header: false,
                });
            }
            Event::Start(Tag::TableHead) => {
                if let Some(table) = &mut builder.table {
                    table.header = true;
                    table.column = 0;
                }
            }
            Event::Start(Tag::TableRow) => {
                if let Some(table) = &mut builder.table {
                    table.header = false;
                    table.column = 0;
                }
            }
            Event::Start(Tag::TableCell) => {
                builder.flush();
                if let Some(table) = &builder.table {
                    builder.current.cell = Some(Cell {
                        column: table.column,
                        columns: table.alignments.len(),
                        header: table.header,
                        alignment: Cell::alignment(
                            table
                                .alignments
                                .get(table.column)
                                .copied()
                                .unwrap_or(pulldown_cmark::Alignment::None),
                        ),
                    });
                }
            }
            Event::End(TagEnd::TableCell) => {
                builder.flush();
                if let Some(table) = &mut builder.table {
                    table.column += 1;
                }
            }
            Event::End(TagEnd::Table) => {
                builder.flush();
                builder.table = None;
            }
            Event::Start(Tag::Heading { level, .. }) => {
                builder.flush();
                builder.current.heading = level as u8;
            }
            Event::Start(Tag::CodeBlock(kind)) => {
                builder.flush();
                builder.current.code = true;
                if let CodeBlockKind::Fenced(language) = kind {
                    builder.current.tooltip = language.to_string();
                }
            }
            Event::Start(Tag::BlockQuote(_)) => {
                builder.flush();
                builder.quote += 1;
            }
            Event::Start(Tag::List(first)) => {
                builder.flush();
                builder.lists.push(first);
            }
            Event::Start(Tag::Item) => {
                builder.flush();
                builder.current.marker = if let Some(Some(number)) = builder.lists.last_mut() {
                    let marker = format!("{number}.");
                    *number = number.saturating_add(1);
                    marker
                } else {
                    "•".to_owned()
                };
            }
            Event::Start(Tag::Strong) => builder.bold += 1,
            Event::Start(Tag::Emphasis) => builder.italic += 1,
            Event::Start(Tag::Link { dest_url, .. }) => {
                builder.current.tooltip.push_str(&format!("\n{dest_url}"));
                builder
                    .links
                    .push((builder.current.text.len(), dest_url.into_string()));
            }
            Event::End(TagEnd::Link) => {
                if let Some((start, destination)) = builder.links.pop() {
                    let end = builder.current.text.len();
                    if start < end {
                        builder.current.links.push(Link {
                            range: start..end,
                            destination,
                        });
                    }
                }
            }
            Event::Start(Tag::Image { dest_url, .. }) => {
                builder.append("[", false);
                builder.current.tooltip.push_str(&format!("\n{dest_url}"));
            }
            Event::End(TagEnd::Image) => builder.append("]", false),
            Event::End(TagEnd::Strong) => builder.bold = builder.bold.saturating_sub(1),
            Event::End(TagEnd::Emphasis) => builder.italic = builder.italic.saturating_sub(1),
            Event::End(TagEnd::BlockQuote(_)) => {
                builder.flush();
                builder.quote = builder.quote.saturating_sub(1);
            }
            Event::End(TagEnd::List(_)) => {
                builder.flush();
                builder.lists.pop();
            }
            Event::End(
                TagEnd::Paragraph
                | TagEnd::Heading(_)
                | TagEnd::CodeBlock
                | TagEnd::Item
                | TagEnd::HtmlBlock,
            ) => builder.flush(),
            Event::Text(text) => builder.append(&text, false),
            Event::Code(text) => builder.append(&text, true),
            Event::Html(text) | Event::InlineHtml(text) => builder.append(&text, true),
            Event::SoftBreak => builder.append(" ", false),
            Event::HardBreak => builder.append("\n", false),
            Event::TaskListMarker(checked) => {
                builder.append(if checked { "☑ " } else { "☐ " }, false)
            }
            Event::Rule => {
                builder.flush();
                builder.append("────────", false);
                builder.flush();
            }
            _ => {}
        }
    }
    builder.flush();
    if let Some(block) = builder
        .blocks
        .iter()
        .position(|block| block.text.len() > vesta_gpui::TEXT_INPUT_MAX_BYTES)
    {
        return Err(MarkdownError::ParagraphTooLong { block });
    }
    Ok(builder.blocks)
}
