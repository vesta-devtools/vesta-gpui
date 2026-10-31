//! The single finite-input admission and Style-to-Taffy conversion boundary.
use super::LayoutError;
use crate::{
    Align, Axis, Dimension, Element, ElementKind, FlexWrap, GridCell, GridTrack, Insets, Justify,
    PositionMode,
};
use taffy::{
    AlignContent, AlignItems, Dimension as TaffyDimension, Display, FlexDirection,
    GridPlacement as TaffyGridPlacement, JustifyContent, LengthPercentage, LengthPercentageAuto,
    Line as TaffyLine, Overflow, Point as TaffyPoint, Position as TaffyPosition, Rect as TaffyRect,
    Size as TaffySize, Style as TaffyStyle,
    style_helpers::{auto, fr, length, line, span},
};

pub(super) fn node_style(
    element: &Element,
    in_scrollbox: bool,
    parent_axis: Axis,
    parent_is_grid: bool,
) -> Result<TaffyStyle, LayoutError> {
    if element.style.align_content == Align::Baseline {
        return Err(LayoutError::InvalidBaselineAlignment(element.id));
    }
    if element
        .style
        .aspect_ratio
        .is_some_and(|ratio| !ratio.is_finite() || ratio <= 0.0)
    {
        return Err(LayoutError::InvalidAspectRatio(element.id));
    }
    if !finite_layout_inputs(element) {
        return Err(LayoutError::NonFiniteStyle(element.id));
    }
    let is_scrollbox = matches!(element.kind, crate::ElementKind::ScrollBox(_));
    let mut style = to_taffy_style(&element.style);
    // Only the immediate scroll content keeps its natural main-axis size.
    // Descendant rows still need to shrink around disclosure, padding and controls.
    if in_scrollbox {
        style.flex_shrink = 0.0;
    }
    // ScrollBox 是滚动视口：taffy 中 scroll container 的 min-content
    // contribution 等于其 flex_basis（flexbox.rs），因此 auto 主轴长度
    // 的 basis 必须为 0——否则内容会把 ScrollBox 撑成内容高度。
    // 已声明的主轴尺寸或显式 flex_basis 必须保留；否则固定宽度的
    // 横向嵌套 ScrollBox 会被错误压成零宽。
    // Hidden and Scroll both give a zero automatic minimum. Reserved axes are
    // resolved from overflow by the shared scroll layout pass.
    if is_scrollbox {
        style.overflow = TaffyPoint {
            x: Overflow::Scroll,
            y: Overflow::Scroll,
        };
        style.scrollbar_width = 0.0;
        if element.style.scrollbar.gutter == crate::ScrollbarGutter::Reserve {
            style.overflow = TaffyPoint {
                x: Overflow::Hidden,
                y: Overflow::Hidden,
            };
            style.scrollbar_width = crate::SCROLLBAR_GUTTER;
        }
        let main_dimension = match parent_axis {
            Axis::Row => element.style.width,
            Axis::Column => element.style.height,
        };
        if !parent_is_grid
            && matches!(main_dimension, Dimension::Auto)
            && element.style.flex_basis.is_none()
        {
            style.flex_basis = TaffyDimension::length(0.0);
        }
    }
    Ok(style)
}

fn finite_layout_inputs(element: &Element) -> bool {
    let style = &element.style;
    let dimension = |value| match value {
        Dimension::Auto => true,
        Dimension::Px(value) | Dimension::Percent(value) => value.is_finite(),
    };
    [
        style.width,
        style.height,
        style.min_width,
        style.min_height,
        style.max_width,
        style.max_height,
        style.flex_basis.unwrap_or(Dimension::Auto),
    ]
    .into_iter()
    .all(dimension)
        && [style.left, style.top, style.right, style.bottom]
            .into_iter()
            .flatten()
            .all(f32::is_finite)
        && [
            style.flex_grow,
            style.flex_shrink,
            style.gap,
            style.padding.left,
            style.padding.right,
            style.padding.top,
            style.padding.bottom,
            style.margin.left,
            style.margin.right,
            style.margin.top,
            style.margin.bottom,
        ]
        .into_iter()
        .all(f32::is_finite)
        && style
            .grid_columns
            .as_deref()
            .unwrap_or_default()
            .iter()
            .chain(&style.grid_rows)
            .all(|track| match track {
                GridTrack::Auto => true,
                GridTrack::Px(value) | GridTrack::Fr(value) => value.is_finite(),
            })
        && match element.kind {
            ElementKind::ScrollBox(offset) => offset.x.is_finite() && offset.y.is_finite(),
            _ => true,
        }
}

fn to_taffy_style(style: &crate::Style) -> TaffyStyle {
    TaffyStyle {
        aspect_ratio: style.aspect_ratio,
        display: if style.grid_columns.is_some() {
            Display::Grid
        } else {
            Display::Flex
        },
        position: match style.position {
            PositionMode::Relative => TaffyPosition::Relative,
            PositionMode::Absolute => TaffyPosition::Absolute,
        },
        inset: TaffyRect {
            left: style
                .left
                .map_or_else(LengthPercentageAuto::auto, LengthPercentageAuto::length),
            right: style
                .right
                .map_or_else(LengthPercentageAuto::auto, LengthPercentageAuto::length),
            top: style
                .top
                .map_or_else(LengthPercentageAuto::auto, LengthPercentageAuto::length),
            bottom: style
                .bottom
                .map_or_else(LengthPercentageAuto::auto, LengthPercentageAuto::length),
        },
        size: TaffySize {
            width: to_dimension(style.width),
            height: to_dimension(style.height),
        },
        min_size: TaffySize {
            width: to_dimension(style.min_width),
            height: to_dimension(style.min_height),
        },
        max_size: TaffySize {
            width: to_dimension(style.max_width),
            height: to_dimension(style.max_height),
        },
        flex_basis: style
            .flex_basis
            .map_or(TaffyDimension::auto(), to_dimension),
        flex_grow: style.flex_grow.max(0.0),
        flex_shrink: style.flex_shrink.max(0.0),
        flex_direction: match style.axis {
            Axis::Row => FlexDirection::Row,
            Axis::Column => FlexDirection::Column,
        },
        flex_wrap: match style.flex_wrap {
            FlexWrap::NoWrap => taffy::FlexWrap::NoWrap,
            FlexWrap::Wrap => taffy::FlexWrap::Wrap,
            FlexWrap::WrapReverse => taffy::FlexWrap::WrapReverse,
        },
        grid_template_columns: style
            .grid_columns
            .as_ref()
            .map(|tracks| tracks.iter().copied().map(to_grid_track).collect())
            .unwrap_or_default(),
        grid_template_rows: style.grid_rows.iter().copied().map(to_grid_track).collect(),
        grid_column: to_grid_cell(style.grid_column),
        grid_row: to_grid_cell(style.grid_row),
        align_items: Some(match style.align {
            Align::Start => AlignItems::START,
            Align::Center => AlignItems::CENTER,
            Align::End => AlignItems::END,
            Align::Stretch => AlignItems::STRETCH,
            Align::Baseline => AlignItems::BASELINE,
        }),
        align_self: style.align_self.map(|align| match align {
            Align::Start => AlignItems::START,
            Align::Center => AlignItems::CENTER,
            Align::End => AlignItems::END,
            Align::Stretch => AlignItems::STRETCH,
            Align::Baseline => AlignItems::BASELINE,
        }),
        align_content: Some(match style.align_content {
            Align::Start => AlignContent::START,
            Align::Center => AlignContent::CENTER,
            Align::End => AlignContent::END,
            Align::Stretch => AlignContent::STRETCH,
            // node_style rejects baseline line alignment before this projection.
            Align::Baseline => AlignContent::START,
        }),
        justify_content: Some(match style.justify {
            Justify::Start => JustifyContent::START,
            Justify::Center => JustifyContent::CENTER,
            Justify::End => JustifyContent::END,
            Justify::SpaceBetween => JustifyContent::SPACE_BETWEEN,
        }),
        gap: TaffySize {
            width: LengthPercentage::length(style.gap.max(0.0)),
            height: LengthPercentage::length(style.gap.max(0.0)),
        },
        padding: to_padding(style.padding),
        margin: TaffyRect {
            left: LengthPercentageAuto::length(style.margin.left),
            right: LengthPercentageAuto::length(style.margin.right),
            top: LengthPercentageAuto::length(style.margin.top),
            bottom: LengthPercentageAuto::length(style.margin.bottom),
        },
        ..TaffyStyle::default()
    }
}

fn to_grid_track(track: GridTrack) -> taffy::GridTemplateComponent<String> {
    match track {
        GridTrack::Auto => auto(),
        GridTrack::Px(value) => length(value.max(0.0)),
        GridTrack::Fr(value) => fr(value.max(0.0)),
    }
}

fn to_grid_cell(cell: GridCell) -> TaffyLine<TaffyGridPlacement> {
    TaffyLine {
        start: cell
            .start
            .filter(|start| *start > 0)
            .map_or(TaffyGridPlacement::Auto, |start| {
                line(start.min(i16::MAX as u16) as i16)
            }),
        end: if cell.span > 1 {
            span(cell.span)
        } else {
            TaffyGridPlacement::Auto
        },
    }
}

fn to_dimension(dimension: Dimension) -> TaffyDimension {
    match dimension {
        Dimension::Auto => TaffyDimension::auto(),
        Dimension::Px(value) => TaffyDimension::length(value.max(0.0)),
        Dimension::Percent(value) => TaffyDimension::percent(value.max(0.0)),
    }
}

fn to_padding(insets: Insets) -> TaffyRect<LengthPercentage> {
    TaffyRect {
        left: LengthPercentage::length(insets.left.max(0.0)),
        right: LengthPercentage::length(insets.right.max(0.0)),
        top: LengthPercentage::length(insets.top.max(0.0)),
        bottom: LengthPercentage::length(insets.bottom.max(0.0)),
    }
}
