//! Typed cell structure over ordinary stable text blocks; rows are page units.
use super::*;
use std::ops::Range;
use vesta_gpui::StyledTextAlignment;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) struct Cell {
    pub column: usize,
    pub columns: usize,
    pub header: bool,
    pub alignment: StyledTextAlignment,
}

impl Cell {
    pub fn alignment(value: pulldown_cmark::Alignment) -> StyledTextAlignment {
        match value {
            pulldown_cmark::Alignment::None => StyledTextAlignment::Start,
            pulldown_cmark::Alignment::Left => StyledTextAlignment::Left,
            pulldown_cmark::Alignment::Center => StyledTextAlignment::Center,
            pulldown_cmark::Alignment::Right => StyledTextAlignment::Right,
        }
    }
}

pub(super) fn wrap(
    document: &MarkdownView,
    scope: ElementScope,
    range: Range<usize>,
    elements: Vec<Element>,
    theme: WidgetTheme,
) -> Vec<Element> {
    use vesta_gpui::{Semantic, SemanticRole};
    let mut cells = document.blocks_in(range).zip(elements).peekable();
    let mut result = Vec::new();
    while let Some((block, element)) = cells.next() {
        if block.content.cell.is_none() {
            result.push(element);
            continue;
        }
        let id = block.id.0;
        let mut table = Element::container(
            scope.element(id + 5),
            scope.identity(id + 5, format!("markdown.table.{id}")).key,
        )
        .semantic(Semantic::new(SemanticRole::Table, ""))
        .styled(
            Style::default()
                .axis(Axis::Column)
                .size(Dimension::Percent(1.0), Dimension::Auto)
                .min_width(Dimension::Px(0.0)),
        );
        let mut current = Some((block, element));
        while let Some((first, element)) = current.take() {
            let id = first.id.0;
            let mut row = Element::container(
                scope.element(id + 4),
                scope
                    .identity(id + 4, format!("markdown.table.row.{id}"))
                    .key,
            )
            .semantic(Semantic::new(SemanticRole::Row, ""))
            .styled(
                Style::default()
                    .axis(Axis::Row)
                    .size(Dimension::Percent(1.0), Dimension::Auto)
                    .min_width(Dimension::Px(0.0)),
            );
            let mut cell = Some((first, element));
            while let Some((block, mut element)) = cell.take() {
                let data = block.content.cell.expect("table cell");
                element.semantic = Some(Semantic::new(
                    if data.header {
                        SemanticRole::ColumnHeader
                    } else {
                        SemanticRole::Cell
                    },
                    "",
                ));
                element.style.width = Dimension::Percent(1.0 / data.columns.max(1) as f32);
                element.style.min_width = Dimension::Px(0.0);
                element.style.min_height = Dimension::Px(theme.typography.font_size * 1.5 + 4.0);
                element.style.flex_grow = 1.0;
                element.style.flex_basis = Some(Dimension::Px(0.0));
                element.style.visual.border_color = Some(theme.palette.border);
                element.style.visual.border_width = 1.0;
                if data.header {
                    element.style.visual.background = Some(theme.palette.surface);
                }
                row.children.push(element);
                if cells
                    .peek()
                    .is_some_and(|(next, _)| next.content.cell.is_some_and(|c| c.column > 0))
                {
                    cell = cells.next();
                }
            }
            table.children.push(row);
            if cells
                .peek()
                .is_some_and(|(next, _)| next.content.cell.is_some_and(|c| !c.header))
            {
                current = cells.next();
            }
        }
        result.push(table);
    }
    result
}

impl MarkdownView {
    pub(super) fn row_starts(&self) -> impl Iterator<Item = usize> + '_ {
        self.blocks.iter().enumerate().filter_map(|(index, block)| {
            block
                .content
                .cell
                .is_none_or(|cell| cell.column == 0)
                .then_some(index)
        })
    }

    pub(super) fn block_range(&self, range: Range<usize>) -> Range<usize> {
        let start = self
            .row_starts()
            .nth(range.start)
            .unwrap_or(self.blocks.len());
        let end = self
            .row_starts()
            .nth(range.end.max(range.start))
            .unwrap_or(self.blocks.len());
        start..end
    }

    pub(super) fn blocks_in(&self, range: Range<usize>) -> impl Iterator<Item = &Block> {
        self.blocks[self.block_range(range)].iter()
    }

    pub(super) fn row_index(&self, block: MarkdownBlockId) -> Result<usize, MarkdownAnchorError> {
        let index = self.block_index(block)?;
        Ok(self
            .row_starts()
            .take_while(|&start| start <= index)
            .count()
            .saturating_sub(1))
    }

    pub(super) fn separator_after(&self, index: usize) -> &'static str {
        match (
            self.blocks.get(index).and_then(|b| b.content.cell),
            self.blocks.get(index + 1).and_then(|b| b.content.cell),
        ) {
            (Some(_), Some(next)) if next.column > 0 => "\t",
            (Some(_), Some(next)) if !next.header => "\n",
            _ => "\n\n",
        }
    }
}

pub(super) fn find_mut(element: &mut Element, id: vesta_gpui::ElementId) -> Option<&mut Element> {
    if element.id == id {
        return Some(element);
    }
    element
        .children
        .iter_mut()
        .find_map(|child| find_mut(child, id))
}
