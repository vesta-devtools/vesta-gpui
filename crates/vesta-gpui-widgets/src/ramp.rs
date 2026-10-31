//! Stable point projection for a host-owned scalar/color ramp.

use std::collections::HashSet;

/// Translate a selected group from its starting snapshot, preserving spacing at the edges.
/// Produces keyed UI edits; the host owns document commands and undo history.
pub fn ramp_group_translation<K: Clone + Eq>(
    initial: &[(K, f32)],
    selected: &[K],
    live: &[K],
    requested: f32,
) -> Option<Vec<(K, f32)>> {
    let points = initial
        .iter()
        .filter(|(key, _)| selected.contains(key))
        .collect::<Vec<_>>();
    if points.is_empty()
        || !requested.is_finite()
        || selected
            .iter()
            .any(|key| !initial.iter().any(|(candidate, _)| candidate == key))
        || points.iter().any(|(key, position)| {
            !live.contains(key) || !position.is_finite() || !(0.0..=1.0).contains(position)
        })
    {
        return None;
    }
    let min = points
        .iter()
        .map(|(_, position)| *position)
        .fold(1.0, f32::min);
    let max = points
        .iter()
        .map(|(_, position)| *position)
        .fold(0.0, f32::max);
    let delta = requested.clamp(-min, 1.0 - max);
    let delta = if delta.abs() <= f32::EPSILON {
        0.0
    } else {
        delta
    };
    Some(
        points
            .into_iter()
            .map(|(key, position)| (key.clone(), (*position + delta).clamp(0.0, 1.0)))
            .collect(),
    )
}

use crate::{Widget, WidgetIdentity, WidgetTheme};
use vesta_gpui::{
    Align, Color, Dimension, Element, ElementId, ElementKey, ElementScope, Insets, Justify, Rect,
    Semantic, SemanticRole, Style,
};

/// Logical width of one ramp control-point handle.
pub const RAMP_POINT_WIDTH: f32 = 18.0;
/// Logical height of one ramp control-point handle.
pub const RAMP_POINT_HEIGHT: f32 = 30.0;
/// Vertical inset that keeps a handle inside the track.
pub const RAMP_POINT_TOP: f32 = 3.0;
/// Logical height of the ramp track.
pub const RAMP_TRACK_HEIGHT: f32 = 36.0;
/// Track border width, which is also the handle's containing-block inset.
pub const RAMP_TRACK_BORDER: f32 = 1.0;
/// Number of flat segments that approximate the gradient band under the handles.
pub const RAMP_BAND_SEGMENTS: u32 = 24;

/// How a ramp fills the space between two control points.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RampInterpolation {
    /// Hold the left point's color until the next point.
    Constant,
    /// Component-wise sRGB blend between the two neighbours.
    #[default]
    Linear,
    /// Smoothstep blend between the two neighbours.
    Smooth,
}

impl RampInterpolation {
    /// Stable display label; hosts project it into their own controls.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Constant => "Constant",
            Self::Linear => "Linear",
            Self::Smooth => "Smooth",
        }
    }

    /// Next mode in the cycle a host control walks through.
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Constant => Self::Linear,
            Self::Linear => Self::Smooth,
            Self::Smooth => Self::Constant,
        }
    }
}

/// Authoring inputs of one ramp that are not per-point identity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RampSpec {
    pub interpolation: RampInterpolation,
    pub width: f32,
}

impl RampSpec {
    #[must_use]
    pub const fn new(interpolation: RampInterpolation, width: f32) -> Self {
        Self {
            interpolation,
            width,
        }
    }
}

/// Component-wise blend in sRGB space; alpha blends the same way.
fn ramp_blend(from: Color, to: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    Color::srgba(
        from.red + (to.red - from.red) * t,
        from.green + (to.green - from.green) * t,
        from.blue + (to.blue - from.blue) * t,
        from.alpha + (to.alpha - from.alpha) * t,
    )
}

/// Color of the ramp at `position`, sampled from sorted `(position, color)` pairs.
///
/// Positions before the first point and after the last one clamp to that endpoint, so the
/// band never extrapolates. Returns `None` for an empty ramp.
#[must_use]
pub fn ramp_color_at(
    points: &[(f32, Color)],
    interpolation: RampInterpolation,
    position: f32,
) -> Option<Color> {
    let (first_position, first_color) = *points.first()?;
    let (last_position, last_color) = *points.last()?;
    if position <= first_position {
        return Some(first_color);
    }
    if position >= last_position {
        return Some(last_color);
    }
    let window = points
        .windows(2)
        .find(|pair| pair[0].0 <= position && position <= pair[1].0)?;
    let (left_position, left_color) = window[0];
    let (right_position, right_color) = window[1];
    match interpolation {
        RampInterpolation::Constant => Some(left_color),
        RampInterpolation::Linear | RampInterpolation::Smooth => {
            let span = right_position - left_position;
            if span <= f32::EPSILON {
                return Some(right_color);
            }
            let mut t = (position - left_position) / span;
            if interpolation == RampInterpolation::Smooth {
                t = t * t * (3.0 - 2.0 * t);
            }
            Some(ramp_blend(left_color, right_color, t))
        }
    }
}

/// Horizontal distance a handle travels inside a track of `width` logical pixels.
///
/// Handles are positioned inside the track's padding box, so both borders are removed
/// before the handle width. A host that repeats this arithmetic drifts from the
/// projection; use the helpers below instead.
#[must_use]
pub fn ramp_travel(width: f32) -> f32 {
    if !width.is_finite() {
        return 0.0;
    }
    (width - 2.0 * RAMP_TRACK_BORDER - RAMP_POINT_WIDTH).max(0.0)
}

/// Left edge of the handle for `position`, relative to the track's padding box.
#[must_use]
pub fn ramp_point_offset(position: f32, width: f32) -> f32 {
    position.clamp(0.0, 1.0) * ramp_travel(width)
}

/// Left edge of the handle for `position` inside a laid-out track rect.
#[must_use]
pub fn ramp_handle_x(track: Rect, position: f32) -> f32 {
    track.x + RAMP_TRACK_BORDER + ramp_point_offset(position, track.width)
}

/// Position whose handle has its left edge at `offset`, clamped to `0..=1`.
#[must_use]
pub fn ramp_position_from_offset(offset: f32, width: f32) -> f32 {
    let travel = ramp_travel(width);
    if travel <= 0.0 || !offset.is_finite() {
        return 0.0;
    }
    (offset / travel).clamp(0.0, 1.0)
}

/// Position for a pointer inside a laid-out `track` that keeps its grab point.
///
/// `grab_offset` is `pointer_x - ramp_handle_x(track, position)` captured when the
/// pointer went down, so the handle does not jump to the pointer on the first move.
#[must_use]
pub fn ramp_position_for_pointer(track: Rect, pointer_x: f32, grab_offset: f32) -> f32 {
    ramp_position_from_offset(
        pointer_x - track.x - RAMP_TRACK_BORDER - grab_offset,
        track.width,
    )
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RampIds {
    pub root: WidgetIdentity,
    pub track: WidgetIdentity,
    /// Scope for the gradient band; the widget uses `band.element(0..RAMP_BAND_SEGMENTS)`.
    pub band: ElementScope,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RampPoint<A> {
    pub id: u32,
    pub root: WidgetIdentity,
    pub label: WidgetIdentity,
    pub position: f32,
    pub color: Color,
    pub selected: bool,
    pub select_action: A,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RampError {
    InvalidWidth,
    InvalidPosition,
    UnorderedPoints,
    DuplicatePoint(u32),
    DuplicateElement(ElementId),
}

impl std::fmt::Display for RampError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidWidth => f.write_str("invalid ramp width"),
            Self::InvalidPosition => f.write_str("invalid ramp point position"),
            Self::UnorderedPoints => f.write_str("ramp points are not ordered"),
            Self::DuplicatePoint(id) => write!(f, "duplicate ramp point {id}"),
            Self::DuplicateElement(id) => write!(f, "duplicate ramp element {id:?}"),
        }
    }
}
impl std::error::Error for RampError {}

pub fn ramp_editor<A>(
    ids: RampIds,
    points: Vec<RampPoint<A>>,
    spec: RampSpec,
    theme: WidgetTheme,
) -> Result<Widget<A>, RampError> {
    let width = spec.width;
    if !width.is_finite() || width < 48.0 {
        return Err(RampError::InvalidWidth);
    }
    let mut point_ids = HashSet::new();
    let mut element_ids = HashSet::new();
    for id in [ids.root.element, ids.track.element].into_iter().chain(
        points
            .iter()
            .flat_map(|point| [point.root.element, point.label.element]),
    ) {
        if !element_ids.insert(id) {
            return Err(RampError::DuplicateElement(id));
        }
    }
    for point in &points {
        if !point.position.is_finite() || !(0.0..=1.0).contains(&point.position) {
            return Err(RampError::InvalidPosition);
        }
        if !point_ids.insert(point.id) {
            return Err(RampError::DuplicatePoint(point.id));
        }
    }
    if points
        .windows(2)
        .any(|pair| pair[0].position > pair[1].position)
    {
        return Err(RampError::UnorderedPoints);
    }
    let mut track = Element::container(ids.track.element, ids.track.key).styled(
        Style::default()
            .size(Dimension::Px(width), Dimension::Px(RAMP_TRACK_HEIGHT))
            .background(theme.palette.field)
            .border(RAMP_TRACK_BORDER, theme.palette.border),
    );
    // The gradient band is a fixed number of flat segments: the renderer has no gradient
    // primitive, and keeping the sampling here means hosts never repeat the interpolation.
    let samples = points
        .iter()
        .map(|point| (point.position, point.color))
        .collect::<Vec<_>>();
    let travel = ramp_travel(width);
    let segment_width = travel / RAMP_BAND_SEGMENTS as f32;
    for segment in 0..RAMP_BAND_SEGMENTS {
        let position = (segment as f32 + 0.5) / RAMP_BAND_SEGMENTS as f32;
        let Some(color) = ramp_color_at(&samples, spec.interpolation, position) else {
            break;
        };
        track = track.child(
            Element::container(
                ids.band.element(segment),
                ElementKey::new(format!("ramp.band.{segment}")),
            )
            .styled(
                Style::default()
                    .absolute(
                        RAMP_TRACK_BORDER + segment as f32 * segment_width,
                        RAMP_TRACK_BORDER,
                    )
                    // Half a pixel of overlap keeps neighbouring segments seam-free.
                    .size(
                        Dimension::Px(segment_width + 0.5),
                        Dimension::Px(RAMP_TRACK_HEIGHT - 2.0 * RAMP_TRACK_BORDER),
                    )
                    .background(color)
                    .pointer_events(false),
            ),
        );
    }
    let mut actions = Vec::with_capacity(points.len());
    for point in points {
        let label = format!("Point {} at {:.0}%", point.id, point.position * 100.0);
        let x = RAMP_TRACK_BORDER + ramp_point_offset(point.position, width);
        // Translucent data has no single backdrop: keep a small opaque glyph backing
        // rather than changing the swatch or guessing the gradient underneath it.
        let backing = (point.color.alpha < 1.0).then_some(Color {
            alpha: 1.0,
            ..theme.palette.field
        });
        let mut mark_style = Style::default()
            .size(Dimension::Percent(1.0), Dimension::Percent(1.0))
            .text(
                theme.typography.small_font_size,
                crate::theme::control_mark_on_fill(backing.unwrap_or(point.color)),
            )
            .pointer_events(false);
        if let Some(backing) = backing {
            mark_style = mark_style
                .size(Dimension::Auto, Dimension::Auto)
                .padding(Insets::all(1.0))
                .background(backing);
        }
        let element = Element::container(point.root.element, point.root.key)
            .semantic(
                Semantic::new(SemanticRole::Slider, &label)
                    .numeric(f64::from(point.position), 0.0, 1.0, Some(0.05))
                    .selected(point.selected),
            )
            .styled(
                Style::default()
                    .absolute(x, RAMP_POINT_TOP)
                    .size(
                        Dimension::Px(RAMP_POINT_WIDTH),
                        Dimension::Px(RAMP_POINT_HEIGHT),
                    )
                    .align(Align::Center)
                    .justify(Justify::Center)
                    .focusable(true)
                    .background(point.color)
                    .border(
                        if point.selected { 2.0 } else { 1.0 },
                        if point.selected {
                            theme.palette.focus
                        } else {
                            theme.palette.border
                        },
                    ),
            )
            .child(Element::text(point.label.element, point.label.key, "◆").styled(mark_style));
        actions.push((element.id, point.select_action));
        track = track.child(element);
    }
    Ok(Widget {
        element: Element::container(ids.root.element, ids.root.key)
            .semantic(Semantic::new(SemanticRole::Group, "Ramp editor"))
            .child(track),
        actions,
    })
}
