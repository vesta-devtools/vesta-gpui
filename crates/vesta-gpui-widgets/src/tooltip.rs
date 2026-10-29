use std::time::{Duration, Instant};

use vesta_gpui::{
    Align, Dimension, Element, ElementId, Insets, Rect, Semantic, SemanticRole, Size, Style,
};

use crate::{PopupDirection, WidgetIdentity, WidgetTheme, place_popup};

pub(crate) const TOOLTIP_PADDING: Insets = Insets::symmetric(6.0, 4.0);

/// Tracks one pointer tooltip. Hosts pass only owners with tooltip metadata;
/// keyboard focus reveals such an owner's tooltip without a delay.
#[derive(Clone, Copy, Debug, Default)]
pub struct TooltipState {
    hovered: Option<(ElementId, Instant)>,
    wake_consumed: bool,
}

impl TooltipState {
    pub fn hover(&mut self, owner: Option<ElementId>, now: Instant) {
        if self.hovered.map(|(current, _)| current) != owner {
            self.hovered = owner.map(|owner| (owner, now));
            self.wake_consumed = false;
        }
    }

    #[must_use]
    pub fn visible_owner(
        &self,
        focused: Option<ElementId>,
        now: Instant,
        delay: Duration,
    ) -> Option<ElementId> {
        focused.or_else(|| {
            self.hovered
                .filter(|(_, since)| now.saturating_duration_since(*since) >= delay)
                .map(|(owner, _)| owner)
        })
    }

    #[must_use]
    pub fn deadline(
        &self,
        focused: Option<ElementId>,
        now: Instant,
        delay: Duration,
    ) -> Option<Instant> {
        if focused.is_some() || self.wake_consumed {
            return None;
        }
        self.hovered
            .and_then(|(_, since)| since.checked_add(delay))
            .filter(|deadline| *deadline > now)
    }

    pub fn reveal_due(
        &mut self,
        focused: Option<ElementId>,
        now: Instant,
        delay: Duration,
    ) -> bool {
        if focused.is_some() || self.wake_consumed {
            return false;
        }
        if self
            .hovered
            .is_some_and(|(_, since)| now.saturating_duration_since(since) >= delay)
        {
            self.wake_consumed = true;
            return true;
        }
        false
    }
}

/// Projects a non-interactive tooltip. The host owns timing and passes a visible anchor.
#[must_use]
pub fn tooltip_popup(
    root: WidgetIdentity,
    label: WidgetIdentity,
    text: &str,
    anchor: Rect,
    bounds: Rect,
    size: Size,
    theme: WidgetTheme,
) -> Element {
    let position = place_popup(anchor, size, bounds, PopupDirection::Below);
    Element::container(root.element, root.key)
        .styled(
            Style::default()
                .absolute(position.x, position.y)
                .size(Dimension::Px(size.width), Dimension::Px(size.height))
                .align(Align::Center)
                .padding(TOOLTIP_PADDING)
                .clipped()
                .background(theme.palette.tooltip_surface)
                .border(1.0, theme.palette.tooltip_border)
                .radius(0.0)
                .z_index(90)
                .pointer_events(false),
        )
        .child(
            Element::text(label.element, label.key, text)
                .styled(
                    Style::default()
                        .size(Dimension::Percent(1.0), Dimension::Auto)
                        .text(theme.typography.small_font_size, theme.palette.tooltip_text)
                        .text_wrap(true)
                        .pointer_events(false),
                )
                .semantic(Semantic::new(SemanticRole::Label, text)),
        )
}
