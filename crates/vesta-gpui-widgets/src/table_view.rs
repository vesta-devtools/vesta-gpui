//! Data-neutral table rows with stable identity and host-owned actions.

use std::collections::HashSet;

use crate::{VirtualListRange, Widget, WidgetContent, WidgetIdentity, WidgetTheme};
use vesta_gpui::{
    Align, Axis, Dimension, Element, ElementId, Insets, Point, Semantic, SemanticRole, Style,
};

#[derive(Clone, Debug, PartialEq)]
pub struct TableColumn<A> {
    pub header: WidgetIdentity,
    pub title: String,
    pub width: f32,
    pub sort_action: Option<A>,
    pub resize: Option<TableColumnResize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TableColumnResize {
    pub handle: WidgetIdentity,
    pub min_width: f32,
    pub max_width: f32,
    pub keyboard_step: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TableResizeKey {
    Decrease,
    Increase,
    PageDecrease,
    PageIncrease,
    Minimum,
    Maximum,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableCell<A> {
    pub identity: WidgetIdentity,
    pub value: String,
    pub selected: bool,
    pub select_action: Option<A>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TableRow<A> {
    pub identity: WidgetIdentity,
    pub cells: Vec<TableCell<A>>,
    pub selected: bool,
    pub select_action: A,
}

/// Header content keeps its own child actions. The shell owns sorting and resizing.
#[derive(Clone, Debug, PartialEq)]
pub struct TableColumnContent<A> {
    pub header: WidgetIdentity,
    pub content: WidgetContent<A>,
    pub width: f32,
    pub sort_action: Option<A>,
    pub resize: Option<TableColumnResize>,
}

/// A cell's semantic label is explicit even when its content is an image or editor.
#[derive(Clone, Debug, PartialEq)]
pub struct TableCellContent<A> {
    pub identity: WidgetIdentity,
    pub content: WidgetContent<A>,
    pub selected: bool,
    pub select_action: Option<A>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TableRowContent<A> {
    pub identity: WidgetIdentity,
    pub cells: Vec<TableCellContent<A>>,
    pub selected: bool,
    pub select_action: A,
}

// Text compatibility and custom content use one shell without inventing child IDs.
enum TableBody<A> {
    Text(String),
    Content(Box<WidgetContent<A>>),
}

impl<A> TableBody<A> {
    fn label(&self) -> &str {
        match self {
            Self::Text(text) => text,
            Self::Content(content) => &content.label,
        }
    }

    fn project(self, identity: WidgetIdentity) -> Widget<A> {
        match self {
            Self::Text(text) => {
                Widget::from_element(Element::text(identity.element, identity.key, text))
            }
            Self::Content(content) => {
                Widget::from_element(Element::container(identity.element, identity.key))
                    .child(content.widget)
            }
        }
    }
}

struct ColumnProjection<A> {
    header: WidgetIdentity,
    body: TableBody<A>,
    width: f32,
    sort_action: Option<A>,
    resize: Option<TableColumnResize>,
}

struct CellProjection<A> {
    identity: WidgetIdentity,
    body: TableBody<A>,
    selected: bool,
    select_action: Option<A>,
}

struct RowProjection<A> {
    identity: WidgetIdentity,
    cells: Vec<CellProjection<A>>,
    selected: bool,
    select_action: A,
}

impl<A> From<TableColumn<A>> for ColumnProjection<A> {
    fn from(column: TableColumn<A>) -> Self {
        Self {
            header: column.header,
            body: TableBody::Text(column.title),
            width: column.width,
            sort_action: column.sort_action,
            resize: column.resize,
        }
    }
}

impl<A> From<TableColumnContent<A>> for ColumnProjection<A> {
    fn from(column: TableColumnContent<A>) -> Self {
        Self {
            header: column.header,
            body: TableBody::Content(Box::new(column.content)),
            width: column.width,
            sort_action: column.sort_action,
            resize: column.resize,
        }
    }
}

impl<A> From<TableRow<A>> for RowProjection<A> {
    fn from(row: TableRow<A>) -> Self {
        Self {
            identity: row.identity,
            cells: row
                .cells
                .into_iter()
                .map(|cell| CellProjection {
                    identity: cell.identity,
                    body: TableBody::Text(cell.value),
                    selected: cell.selected,
                    select_action: cell.select_action,
                })
                .collect(),
            selected: row.selected,
            select_action: row.select_action,
        }
    }
}

impl<A> From<TableRowContent<A>> for RowProjection<A> {
    fn from(row: TableRowContent<A>) -> Self {
        Self {
            identity: row.identity,
            cells: row
                .cells
                .into_iter()
                .map(|cell| CellProjection {
                    identity: cell.identity,
                    body: TableBody::Content(Box::new(cell.content)),
                    selected: cell.selected,
                    select_action: cell.select_action,
                })
                .collect(),
            selected: row.selected,
            select_action: row.select_action,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableWindowIds {
    pub root: WidgetIdentity,
    pub header: WidgetIdentity,
    pub viewport: WidgetIdentity,
    pub content: WidgetIdentity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TableViewError {
    EmptyColumns,
    InvalidColumnWidth,
    InvalidResizeBounds,
    CellCount,
    InvalidViewport,
    DuplicateElement(ElementId),
}

#[must_use]
pub fn table_column_width_at(
    start_width: f32,
    pointer_delta: f32,
    resize: &TableColumnResize,
) -> f32 {
    (start_width + pointer_delta).clamp(resize.min_width, resize.max_width)
}

#[must_use]
pub fn table_column_width_for_key(
    width: f32,
    key: TableResizeKey,
    resize: &TableColumnResize,
) -> f32 {
    let step = match key {
        TableResizeKey::Decrease => -resize.keyboard_step,
        TableResizeKey::Increase => resize.keyboard_step,
        TableResizeKey::PageDecrease => -resize.keyboard_step * 4.0,
        TableResizeKey::PageIncrease => resize.keyboard_step * 4.0,
        TableResizeKey::Minimum => return resize.min_width,
        TableResizeKey::Maximum => return resize.max_width,
    };
    table_column_width_at(width, step, resize)
}

/// Project only visible rows. Hosts own sorting, selection, scrolling, and any virtualization.
pub fn table_view<A: Clone>(
    root: WidgetIdentity,
    header: WidgetIdentity,
    columns: Vec<TableColumn<A>>,
    rows: Vec<TableRow<A>>,
    theme: WidgetTheme,
) -> Result<Widget<A>, TableViewError> {
    table_view_projected(
        root,
        header,
        columns.into_iter().map(Into::into).collect(),
        rows.into_iter().map(Into::into).collect(),
        theme,
    )
}

/// Project arbitrary header and cell Widgets with the same table behavior as text rows.
pub fn table_view_with_content<A: Clone>(
    root: WidgetIdentity,
    header: WidgetIdentity,
    columns: Vec<TableColumnContent<A>>,
    rows: Vec<TableRowContent<A>>,
    theme: WidgetTheme,
) -> Result<Widget<A>, TableViewError> {
    table_view_projected(
        root,
        header,
        columns.into_iter().map(Into::into).collect(),
        rows.into_iter().map(Into::into).collect(),
        theme,
    )
}

fn table_view_projected<A: Clone>(
    root: WidgetIdentity,
    header: WidgetIdentity,
    columns: Vec<ColumnProjection<A>>,
    rows: Vec<RowProjection<A>>,
    theme: WidgetTheme,
) -> Result<Widget<A>, TableViewError> {
    if columns.is_empty() {
        return Err(TableViewError::EmptyColumns);
    }
    if columns
        .iter()
        .any(|column| !column.width.is_finite() || column.width <= 0.0)
    {
        return Err(TableViewError::InvalidColumnWidth);
    }
    if columns
        .iter()
        .filter_map(|column| column.resize.as_ref())
        .any(|resize| {
            !resize.min_width.is_finite()
                || !resize.max_width.is_finite()
                || !resize.keyboard_step.is_finite()
                || resize.min_width <= 0.0
                || resize.max_width < resize.min_width
                || resize.keyboard_step <= 0.0
        })
    {
        return Err(TableViewError::InvalidResizeBounds);
    }
    if rows.iter().any(|row| row.cells.len() != columns.len()) {
        return Err(TableViewError::CellCount);
    }
    let mut identities = HashSet::new();
    for id in [root.element, header.element]
        .into_iter()
        .chain(columns.iter().map(|column| column.header.element))
        .chain(
            columns
                .iter()
                .filter_map(|column| column.resize.as_ref().map(|resize| resize.handle.element)),
        )
        .chain(rows.iter().flat_map(|row| {
            std::iter::once(row.identity.element)
                .chain(row.cells.iter().map(|cell| cell.identity.element))
        }))
    {
        if !identities.insert(id) {
            return Err(TableViewError::DuplicateElement(id));
        }
    }

    let mut actions = Vec::new();
    let mut header_element = Element::container(header.element, header.key)
        .styled(Style::default().axis(Axis::Row).align(Align::Center));
    let widths = columns
        .iter()
        .map(|column| column.width)
        .collect::<Vec<_>>();
    for column in columns {
        let handle_width = if column.resize.is_some() { 6.0 } else { 0.0 };
        let label = column.body.label().to_owned();
        let mut widget = column.body.project(column.header);
        widget.element = widget
            .element
            .semantic(Semantic::new(SemanticRole::ColumnHeader, &label))
            .styled(
                Style::default()
                    .size(
                        Dimension::Px((column.width - handle_width).max(1.0)),
                        Dimension::Px(theme.metrics.control_height),
                    )
                    .flex(0.0, 0.0)
                    .padding(Insets::symmetric(7.0, 0.0))
                    .text(theme.typography.font_size, theme.palette.text)
                    .background(theme.palette.surface)
                    .clipped(),
            );
        if let Some(action) = column.sort_action {
            actions.push((widget.element.id, action));
            widget.element.style.focusable = true;
        } else {
            widget.element.style.pointer_events = false;
        }
        actions.extend(widget.actions);
        header_element = header_element.child(widget.element);
        if let Some(resize) = &column.resize {
            header_element = header_element.child(
                Element::container(resize.handle.element, resize.handle.key.clone())
                    .semantic(
                        Semantic::new(SemanticRole::Splitter, format!("Resize {label} column"))
                            .numeric(
                                f64::from(column.width),
                                f64::from(resize.min_width),
                                f64::from(resize.max_width),
                                Some(f64::from(resize.keyboard_step)),
                            ),
                    )
                    .styled(
                        Style::default()
                            .size(
                                Dimension::Px(handle_width),
                                Dimension::Px(theme.metrics.control_height),
                            )
                            .flex(0.0, 0.0)
                            .background(theme.palette.border)
                            .focusable(true),
                    ),
            );
        }
    }

    let mut root_element = Element::container(root.element, root.key)
        .semantic(Semantic::new(SemanticRole::Table, "Table"))
        .styled(
            Style::default()
                .axis(Axis::Column)
                .background(theme.palette.panel)
                .border(1.0, theme.palette.border),
        )
        .child(header_element);
    for row in rows {
        let row_label = row
            .cells
            .iter()
            .map(|cell| cell.body.label())
            .collect::<Vec<_>>()
            .join(", ");
        let mut row_element = Element::container(row.identity.element, row.identity.key)
            .semantic(Semantic::new(SemanticRole::Row, row_label).selected(row.selected))
            .styled(
                Style::default()
                    .axis(Axis::Row)
                    .align(Align::Center)
                    .focusable(true)
                    .background(if row.selected {
                        theme.palette.selection
                    } else {
                        theme.palette.panel
                    }),
            );
        actions.push((row_element.id, row.select_action));
        for (cell, width) in row.cells.into_iter().zip(&widths) {
            let mut style = Style::default()
                .size(
                    Dimension::Px(*width),
                    Dimension::Px(theme.metrics.control_height),
                )
                .flex(0.0, 0.0)
                .padding(Insets::symmetric(7.0, 0.0))
                .text(theme.typography.font_size, theme.palette.text)
                .clipped();
            if cell.selected {
                style = style.background(theme.palette.selection);
            }
            if cell.select_action.is_some() {
                style.focusable = true;
            } else {
                style.pointer_events = false;
            }
            let cell_id = cell.identity.element;
            let label = cell.body.label().to_owned();
            let mut content = cell.body.project(cell.identity);
            content.element = content
                .element
                .semantic(Semantic::new(SemanticRole::Cell, label).selected(cell.selected))
                .styled(style);
            row_element = row_element.child(content.element);
            actions.extend(content.actions);
            if let Some(action) = cell.select_action {
                actions.push((cell_id, action));
            }
        }
        root_element = root_element.child(row_element);
    }
    // Child content participates in the same identity namespace as table shells.
    let mut identities = HashSet::new();
    let mut pending = vec![&root_element];
    while let Some(element) = pending.pop() {
        if !identities.insert(element.id) {
            return Err(TableViewError::DuplicateElement(element.id));
        }
        pending.extend(element.children.iter().rev());
    }
    Ok(Widget {
        element: root_element,
        actions,
    })
}

/// Keep the table header visible while projecting only the host's bounded row range.
/// The host maps each sorted visual index to a stable row identity.
pub fn table_view_windowed<A: Clone>(
    ids: TableWindowIds,
    columns: Vec<TableColumn<A>>,
    range: VirtualListRange,
    retained_index: Option<usize>,
    mut row_at: impl FnMut(usize) -> TableRow<A>,
    theme: WidgetTheme,
) -> Result<Widget<A>, TableViewError> {
    table_view_windowed_projected(
        ids,
        columns.into_iter().map(Into::into).collect(),
        range,
        None,
        retained_index,
        |index| row_at(index).into(),
        theme,
    )
}

/// Build only the bounded visible range and one retained focus row, including child content.
pub fn table_view_windowed_with_content<A: Clone>(
    ids: TableWindowIds,
    columns: Vec<TableColumnContent<A>>,
    range: VirtualListRange,
    retained_index: Option<usize>,
    mut row_at: impl FnMut(usize) -> TableRowContent<A>,
    theme: WidgetTheme,
) -> Result<Widget<A>, TableViewError> {
    table_view_windowed_projected(
        ids,
        columns.into_iter().map(Into::into).collect(),
        range,
        None,
        retained_index,
        |index| row_at(index).into(),
        theme,
    )
}

/// Project one host-owned horizontal offset through both header and body.
/// The vertical offset remains authoritative in `range`.
pub fn table_view_windowed_scrolled<A: Clone>(
    ids: TableWindowIds,
    columns: Vec<TableColumn<A>>,
    range: VirtualListRange,
    horizontal_offset: f32,
    retained_index: Option<usize>,
    mut row_at: impl FnMut(usize) -> TableRow<A>,
    theme: WidgetTheme,
) -> Result<Widget<A>, TableViewError> {
    table_view_windowed_projected(
        ids,
        columns.into_iter().map(Into::into).collect(),
        range,
        Some(horizontal_offset),
        retained_index,
        |index| row_at(index).into(),
        theme,
    )
}

/// Arbitrary content uses the same horizontal projection and bounded row work.
pub fn table_view_windowed_scrolled_with_content<A: Clone>(
    ids: TableWindowIds,
    columns: Vec<TableColumnContent<A>>,
    range: VirtualListRange,
    horizontal_offset: f32,
    retained_index: Option<usize>,
    mut row_at: impl FnMut(usize) -> TableRowContent<A>,
    theme: WidgetTheme,
) -> Result<Widget<A>, TableViewError> {
    table_view_windowed_projected(
        ids,
        columns.into_iter().map(Into::into).collect(),
        range,
        Some(horizontal_offset),
        retained_index,
        |index| row_at(index).into(),
        theme,
    )
}

fn table_view_windowed_projected<A: Clone>(
    ids: TableWindowIds,
    columns: Vec<ColumnProjection<A>>,
    range: VirtualListRange,
    horizontal_offset: Option<f32>,
    retained_index: Option<usize>,
    mut row_at: impl FnMut(usize) -> RowProjection<A>,
    theme: WidgetTheme,
) -> Result<Widget<A>, TableViewError> {
    if !range.row_height.is_finite()
        || range.row_height < 1.0
        || !range.viewport_height.is_finite()
        || range.viewport_height <= 0.0
        || !range.content_height.is_finite()
        || !(0.0..=16_777_216.0).contains(&range.content_height)
        || !range.offset.is_finite()
        || range.first > range.end
        || range.end > range.count
        || range.end - range.first > 320
        || (f64::from(range.content_height) - range.count as f64 * f64::from(range.row_height))
            .abs()
            > f64::from(range.row_height)
        || range.offset < 0.0
        || range.offset > (range.content_height - range.viewport_height).max(0.0)
    {
        return Err(TableViewError::InvalidViewport);
    }
    if ids.viewport.element == ids.content.element {
        return Err(TableViewError::DuplicateElement(ids.viewport.element));
    }
    let mut indices = (range.first..range.end).collect::<Vec<_>>();
    if let Some(index) = retained_index
        .filter(|index| *index < range.count && (*index < range.first || *index >= range.end))
    {
        indices.push(index);
    }
    let width = columns.iter().map(|column| column.width).sum::<f32>();
    if !width.is_finite() {
        return Err(TableViewError::InvalidColumnWidth);
    }
    if horizontal_offset.is_some_and(|offset| !offset.is_finite() || offset < 0.0 || offset > width)
    {
        return Err(TableViewError::InvalidViewport);
    }
    let rows = indices.iter().copied().map(&mut row_at).collect();
    let mut table = table_view_projected(ids.root, ids.header, columns, rows, theme)?;
    let outer_width = width
        + if range.content_height > range.viewport_height {
            vesta_gpui::SCROLLBAR_GUTTER
        } else {
            0.0
        };
    table.element.style.width = Dimension::Px(outer_width);
    table.element.style.max_width = Dimension::Percent(1.0);
    for id in [ids.viewport.element, ids.content.element] {
        if table.element.find(id).is_some() {
            return Err(TableViewError::DuplicateElement(id));
        }
    }
    let mut children = std::mem::take(&mut table.element.children);
    let mut header = children.remove(0);
    if let Some(offset) = horizontal_offset {
        // Core applies inherited flow to both scroll projections. The header
        // has no independent retained state or visible scrollbar.
        header.kind = vesta_gpui::ElementKind::ScrollBox(Point::new(offset, 0.0));
        header.style.height = Dimension::Px(theme.metrics.control_height);
        header.style.flex_shrink = 0.0;
        header.style.scrollbar.gutter = vesta_gpui::ScrollbarGutter::Overlay;
        header.style.scrollbar.visibility = vesta_gpui::ScrollbarVisibility::OnHover;
        let transparent = vesta_gpui::Color::srgba(0.0, 0.0, 0.0, 0.0);
        header.style.scrollbar.track = transparent;
        header.style.scrollbar.thumb = transparent;
        header.style.scrollbar.thumb_hovered = transparent;
    }
    let mut content = Element::container(ids.content.element, ids.content.key)
        .styled(Style::default().size(Dimension::Px(width), Dimension::Px(range.content_height)));
    for (index, mut row) in indices.into_iter().zip(children) {
        row.style = row
            .style
            .absolute(0.0, index as f32 * range.row_height)
            .size(Dimension::Percent(1.0), Dimension::Px(range.row_height));
        content = content.child(row);
    }
    let viewport = Element::scroll_box(
        ids.viewport.element,
        ids.viewport.key,
        Point::new(horizontal_offset.unwrap_or_default(), range.offset),
    )
    .styled(
        Style::default()
            .size(
                Dimension::Px(outer_width),
                Dimension::Px(range.viewport_height),
            )
            .max_width(Dimension::Percent(1.0)),
    )
    .child(content);
    table.element.children = vec![header, viewport];
    Ok(table)
}

#[cfg(test)]
mod tests;
