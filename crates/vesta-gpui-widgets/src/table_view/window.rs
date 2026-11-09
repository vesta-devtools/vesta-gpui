//! Bounded column projection; source records, identities and scroll policy stay with the Host.
use std::collections::HashSet;

use super::*;
use vesta_gpui::{FlowDirection, ScrollAttachment, ScrollbarGutter};

const MAX_EXTENT: f64 = 16_777_216.0;
const MAX_COLUMNS: usize = 320;

/// Immutable geometry in the Host's current visual column order.
/// Leading/trailing frozen columns are contiguous prefixes/suffixes in that order.
#[derive(Clone, Debug, PartialEq)]
pub struct TableColumnMetrics {
    edges: Vec<f64>,
    leading: usize,
    trailing: usize,
}

impl TableColumnMetrics {
    pub fn new(widths: &[f32], leading: usize, trailing: usize) -> Result<Self, TableViewError> {
        if widths.is_empty() {
            return Err(TableViewError::EmptyColumns);
        }
        if widths.len() > MAX_EXTENT as usize {
            return Err(TableViewError::InvalidColumnWidth);
        }
        if leading > widths.len() || trailing > widths.len() - leading || leading + trailing > 64 {
            return Err(TableViewError::InvalidViewport);
        }
        let mut edges = Vec::with_capacity(widths.len() + 1);
        edges.push(0.0);
        let mut total = 0.0;
        for &width in widths {
            if !width.is_finite() || width < 1.0 {
                return Err(TableViewError::InvalidColumnWidth);
            }
            total += f64::from(width);
            if total > MAX_EXTENT {
                return Err(TableViewError::InvalidColumnWidth);
            }
            edges.push(total);
        }
        Ok(Self {
            edges,
            leading,
            trailing,
        })
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.edges.len() - 1
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        false // Construction requires a column.
    }

    #[must_use]
    pub fn width(&self, index: usize) -> Option<f32> {
        self.edges
            .get(index..=index.checked_add(1)?)
            .map(|edges| (edges[1] - edges[0]) as f32)
    }

    /// `width` is the usable table width. The Host chooses/owns scrollbar gutters.
    /// Offset increases from logical start in either direction; finite offsets clamp.
    /// At most 256 visible scrolling columns, 32 overscan per side and 320 total
    /// projected columns are admitted, including at most 64 frozen columns.
    pub fn window(
        &self,
        width: f32,
        offset: f32,
        overscan: usize,
        flow: FlowDirection,
    ) -> Result<TableColumnWindow<'_>, TableViewError> {
        let start = self.edges[self.leading];
        let end = self.edges[self.len() - self.trailing];
        let trailing_width = self.edges[self.len()] - end;
        let viewport = f64::from(width) - start - trailing_width;
        if !width.is_finite()
            || width <= 0.0
            || f64::from(width) > MAX_EXTENT
            || !offset.is_finite()
            || viewport < 0.0
            || (start < end && viewport == 0.0)
        {
            return Err(TableViewError::InvalidViewport);
        }
        let maximum = (end - start - viewport).max(0.0);
        let mut offset = f64::from(offset).clamp(0.0, maximum) as f32;
        if f64::from(offset) > maximum {
            offset = offset.next_down().max(0.0);
        }
        let left = start + f64::from(offset);
        let right = (left + viewport).min(end);
        let first = self
            .edges
            .partition_point(|edge| *edge <= left)
            .saturating_sub(1)
            .clamp(self.leading, self.len() - self.trailing);
        let last = self
            .edges
            .partition_point(|edge| *edge < right)
            .clamp(first, self.len() - self.trailing);
        if last - first > 256 {
            return Err(TableViewError::InvalidViewport);
        }
        let overscan = overscan.min(32);
        let scrolling = first.saturating_sub(overscan).max(self.leading)
            ..last
                .saturating_add(overscan)
                .min(self.len() - self.trailing);
        if self.leading + scrolling.len() + self.trailing > MAX_COLUMNS {
            return Err(TableViewError::InvalidViewport);
        }
        let indices = (0..self.leading)
            .chain(scrolling.clone())
            .chain(self.len() - self.trailing..self.len())
            .collect();
        Ok(TableColumnWindow {
            metrics: self,
            width,
            offset,
            maximum: maximum as f32,
            flow,
            indices,
        })
    }
}

/// Validated borrowed column window. Recompute after width/order/scroll changes.
#[derive(Clone, Debug)]
pub struct TableColumnWindow<'a> {
    metrics: &'a TableColumnMetrics,
    width: f32,
    offset: f32,
    maximum: f32,
    flow: FlowDirection,
    indices: Vec<usize>,
}

impl TableColumnWindow<'_> {
    #[must_use]
    pub fn indices(&self) -> &[usize] {
        &self.indices
    }

    #[must_use]
    pub const fn offset(&self) -> f32 {
        self.offset
    }

    #[must_use]
    pub const fn max_offset(&self) -> f32 {
        self.maximum
    }

    /// Desired logical offset for the Host's existing scroll state. Frozen
    /// columns require no movement. Oversized cells align to logical start.
    #[must_use]
    pub fn offset_to_reveal(&self, index: usize) -> Option<f32> {
        self.metrics.width(index)?;
        if self.band(index) != 1 {
            return Some(self.offset);
        }
        let origin = self.metrics.edges[self.metrics.leading];
        let start = self.metrics.edges[index] - origin;
        let end = self.metrics.edges[index + 1] - origin;
        let width = f64::from(self.band_bounds(1).1);
        let offset = f64::from(self.offset);
        let desired = if start < offset || end - start > width {
            start
        } else if end > offset + width {
            end - width
        } else {
            offset
        };
        Some(desired.clamp(0.0, f64::from(self.maximum)) as f32)
    }

    /// Physical table-local bounds, before frozen-band clipping. Useful for Host
    /// reveal/input policy; this never creates another layout or hit authority.
    #[must_use]
    pub fn column_bounds(&self, index: usize) -> Option<(f32, f32)> {
        let width = self.metrics.width(index)?;
        let band = self.band(index);
        let (band_x, band_width) = self.band_bounds(band);
        let x = self.local_start(index, band);
        Some((band_x + self.physical_x(x, width, band_width), width))
    }

    fn band(&self, index: usize) -> usize {
        if index < self.metrics.leading {
            0
        } else if index >= self.metrics.len() - self.metrics.trailing {
            2
        } else {
            1
        }
    }

    fn physical_x(&self, start: f32, width: f32, parent_width: f32) -> f32 {
        match self.flow {
            FlowDirection::LeftToRight => start,
            FlowDirection::RightToLeft => parent_width - start - width,
        }
    }

    fn band_bounds(&self, band: usize) -> (f32, f32) {
        let leading = self.metrics.edges[self.metrics.leading];
        let trailing = self.metrics.edges[self.metrics.len()]
            - self.metrics.edges[self.metrics.len() - self.metrics.trailing];
        let viewport = f64::from(self.width);
        let (start, width) = match band {
            0 => (0.0, leading),
            1 => (leading, viewport - leading - trailing),
            _ => (viewport - trailing, trailing),
        };
        let x = match self.flow {
            FlowDirection::LeftToRight => start,
            FlowDirection::RightToLeft => viewport - start - width,
        };
        (x as f32, width as f32)
    }

    fn local_start(&self, index: usize, band: usize) -> f32 {
        let origin = match band {
            0 => 0.0,
            1 => self.metrics.edges[self.metrics.leading] + f64::from(self.offset),
            _ => self.metrics.edges[self.metrics.len() - self.metrics.trailing],
        };
        (self.metrics.edges[index] - origin) as f32
    }
}

/// All wrapper identities are explicit Host identities in the same Element tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableBandIds {
    pub leading: WidgetIdentity,
    pub scrolling: WidgetIdentity,
    pub trailing: WidgetIdentity,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WideTableIds {
    pub table: TableWindowIds,
    pub header_bands: TableBandIds,
}

/// One focused row/cell/header may survive outside either visible range.
/// `row: None` retains a header; `column: None` retains only a row shell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TableRetainedFocus {
    pub row: Option<usize>,
    pub column: Option<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TableWindowRow<A> {
    pub bands: TableBandIds,
    /// Stable accessible row name, independent of which cells are realized.
    pub label: String,
    /// Cells follow exactly the column-index slice passed to the row factory.
    pub row: TableRowContent<A>,
}

fn bands(ids: TableBandIds, window: &TableColumnWindow<'_>, height: f32) -> [Element; 3] {
    let mut elements = [ids.leading, ids.scrolling, ids.trailing].map(|identity| {
        Element::container(identity.element, identity.key)
            .styled(Style::default().size(Dimension::Px(0.0), Dimension::Px(height)))
    });
    // Absolute coordinates are physical. RTL still controls arbitrary child Widgets.
    for (band, element) in elements.iter_mut().enumerate() {
        let (x, width) = window.band_bounds(band);
        element.style = element.style.clone().absolute(x, 0.0);
        element.style.width = Dimension::Px(width);
    }
    elements
}

fn include(indices: &mut Vec<usize>, index: Option<usize>) {
    if let Some(index) = index
        && let Err(position) = indices.binary_search(&index)
    {
        indices.insert(position, index);
    }
}

/// Project bounded rows AND columns with frozen logical start/end bands.
/// Factories see source indices, never synthesized IDs. Only the retained row
/// builds an offscreen focused cell; retaining a header does not build that cell
/// in every row. Sorting, resizing, scroll/reveal, selection and clipboard are Host policy.
pub fn table_view_columns<A: Clone>(
    ids: WideTableIds,
    window: &TableColumnWindow<'_>,
    range: VirtualListRange,
    retained: Option<TableRetainedFocus>,
    mut column_at: impl FnMut(usize) -> TableColumnContent<A>,
    mut row_at: impl FnMut(usize, &[usize]) -> TableWindowRow<A>,
    theme: WidgetTheme,
) -> Result<Widget<A>, TableViewError> {
    // Raw row DTOs use the same admission rules as the existing row-window API.
    validate_row_window(range)?;
    if retained.is_some_and(|focus| {
        focus.row.is_some_and(|row| row >= range.count)
            || focus
                .column
                .is_some_and(|column| column >= window.metrics.len())
    }) {
        return Err(TableViewError::InvalidViewport);
    }
    let mut actions = Vec::new();
    let mut header_indices = window.indices.clone();
    include(
        &mut header_indices,
        retained
            .filter(|focus| focus.row.is_none())
            .and_then(|focus| focus.column),
    );
    let mut header_bands = bands(ids.header_bands, window, theme.metrics.control_height);
    for index in header_indices {
        let column = column_at(index);
        let width = window.metrics.width(index).expect("admitted column");
        if column.width != width {
            return Err(TableViewError::InvalidColumnWidth);
        }
        if column.resize.is_some() && width < 7.0 {
            return Err(TableViewError::InvalidColumnWidth);
        }
        validate_resize(column.resize.as_ref())?;
        let band = window.band(index);
        let (_, band_width) = window.band_bounds(band);
        let mut start = window.local_start(index, band);
        for mut element in project_column(column.into(), theme, &mut actions) {
            let Dimension::Px(width) = element.style.width else {
                unreachable!()
            };
            element.style = element
                .style
                .absolute(window.physical_x(start, width, band_width), 0.0);
            header_bands[band].children.push(element);
            start += width;
        }
    }
    let header = Element::container(ids.table.header.element, ids.table.header.key)
        .styled(Style::default().absolute(0.0, 0.0).size(
            Dimension::Px(window.width),
            Dimension::Px(theme.metrics.control_height),
        ))
        .children(header_bands);
    let mut content = Element::container(ids.table.content.element, ids.table.content.key).styled(
        Style::default()
            .absolute(0.0, 0.0)
            .scroll_attachment(ScrollAttachment::ViewportX)
            .size(
                Dimension::Px(
                    (window.metrics.edges[window.metrics.len()] as f32).max(window.width),
                ),
                Dimension::Px(range.content_height),
            ),
    );
    let mut rows = (range.first..range.end).collect::<Vec<_>>();
    include(&mut rows, retained.and_then(|focus| focus.row));
    for index in rows {
        let mut indices = window.indices.clone();
        include(
            &mut indices,
            retained
                .filter(|focus| focus.row == Some(index))
                .and_then(|focus| focus.column),
        );
        let TableWindowRow {
            bands: band_ids,
            label,
            row,
        } = row_at(index, &indices);
        if row.cells.len() != indices.len() {
            return Err(TableViewError::CellCount);
        }
        let mut row_element = Element::container(row.identity.element, row.identity.key)
            .semantic(Semantic::new(SemanticRole::Row, label).selected(row.selected))
            // The content wrapper attaches horizontally to this same ScrollBox.
            // The scrolling band projects its offset; frozen bands stay fixed.
            .styled(
                Style::default()
                    .absolute(0.0, index as f32 * range.row_height)
                    .size(Dimension::Px(window.width), Dimension::Px(range.row_height))
                    .focusable(true)
                    .background(if row.selected {
                        theme.palette.selection
                    } else {
                        theme.palette.panel
                    }),
            );
        actions.push((row_element.id, row.select_action));
        let mut row_bands = bands(band_ids, window, range.row_height);
        for (column, cell) in indices.into_iter().zip(row.cells) {
            let band = window.band(column);
            let width = window.metrics.width(column).expect("admitted column");
            let (_, band_width) = window.band_bounds(band);
            let mut element = project_cell(
                CellProjection {
                    identity: cell.identity,
                    body: TableBody::Content(Box::new(cell.content)),
                    selected: cell.selected,
                    select_action: cell.select_action,
                },
                width,
                range.row_height,
                theme,
                &mut actions,
            );
            element.style = element.style.absolute(
                window.physical_x(window.local_start(column, band), width, band_width),
                0.0,
            );
            row_bands[band].children.push(element);
        }
        row_element.children.extend(row_bands);
        content.children.push(row_element);
    }
    let mut style = Style::default()
        .absolute(0.0, theme.metrics.control_height)
        .size(
            Dimension::Px(window.width),
            Dimension::Px(range.viewport_height),
        );
    style.scrollbar.gutter = ScrollbarGutter::Overlay;
    let viewport = Element::scroll_box(
        ids.table.viewport.element,
        ids.table.viewport.key,
        Point::new(window.offset, range.offset),
    )
    .styled(style)
    .child(content);
    let element = Element::container(ids.table.root.element, ids.table.root.key)
        .semantic(Semantic::new(SemanticRole::Table, "Table"))
        .styled(
            Style::default()
                .size(
                    Dimension::Px(window.width),
                    Dimension::Px(theme.metrics.control_height + range.viewport_height),
                )
                .flow_direction(window.flow)
                .background(theme.palette.panel)
                .border(1.0, theme.palette.border),
        )
        .child(header)
        .child(viewport);
    let mut seen = HashSet::new();
    let mut pending = vec![&element];
    while let Some(element) = pending.pop() {
        if !seen.insert(element.id) {
            return Err(TableViewError::DuplicateElement(element.id));
        }
        pending.extend(&element.children);
    }
    Ok(Widget { element, actions })
}

#[cfg(test)]
mod tests;
