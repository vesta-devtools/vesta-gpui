use vesta_gpui::{
    ActionRoute, Axis, Dimension, Element, ElementId, InputTarget, Insets, KeyChord, KeyStroke,
    Rect, Semantic, SemanticRole, Size, Style, UiWindowId,
};

use crate::{
    ControlState, PopupCloseReason, PopupDirection, PopupDismissal, Widget, WidgetIdentity,
    WidgetTheme, button, place_popup,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PopupKeyAction {
    Close,
    TraverseFocus { backwards: bool },
}

/// Only dismissal keys belong to a general popover; its content keeps other keys.
#[must_use]
pub fn popup_key_action(chord: KeyChord) -> Option<PopupKeyAction> {
    if chord.modifiers.control || chord.modifiers.alt || chord.modifiers.super_key {
        return None;
    }
    match chord.key {
        KeyStroke::Escape => Some(PopupKeyAction::Close),
        KeyStroke::Tab => Some(PopupKeyAction::TraverseFocus {
            backwards: chord.modifiers.shift,
        }),
        _ => None,
    }
}

/// An outside press dismisses only the popup owned by the event's source window.
/// Both the opener's full subtree and the popup's content count as inside. The host
/// continues routing the press to its original target after closing the popup.
#[must_use]
pub fn popup_dismiss_for_hit<A>(
    opener: InputTarget,
    popup_root: ElementId,
    source_window: UiWindowId,
    hit: Option<ElementId>,
    actions: &ActionRoute<A>,
) -> bool {
    opener.window == source_window
        && !hit.is_some_and(|element| {
            actions.is_descendant_of(element, opener.element)
                || actions.is_descendant_of(element, popup_root)
        })
}

/// Retained visibility and opener identity for one non-modal popover.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PopoverState {
    opener: Option<InputTarget>,
}

impl PopoverState {
    #[must_use]
    pub fn is_open(self) -> bool {
        self.opener.is_some()
    }

    /// The stable opener and owning window of the current popover.
    #[must_use]
    pub fn opener(self) -> Option<InputTarget> {
        self.opener
    }

    pub fn open_for(&mut self, opener: InputTarget) {
        self.opener = Some(opener);
    }

    /// Returns the opener so the host can restore it only if still focusable.
    pub fn close(&mut self) -> Option<InputTarget> {
        self.opener.take()
    }

    /// Removed/disabled openers end only this window's popup without returning focus.
    pub fn reconcile<A>(
        &mut self,
        source: UiWindowId,
        scope: &vesta_gpui::FocusScope<'_, A>,
    ) -> Option<PopupDismissal> {
        let opener = self.opener.filter(|opener| opener.window == source)?;
        if scope.contains(opener) {
            return None;
        }
        self.dismiss(source, PopupCloseReason::OwnerRemoved)
    }

    /// Foreign requests and repeated closes leave the session untouched.
    pub fn dismiss(
        &mut self,
        source_window: UiWindowId,
        reason: PopupCloseReason,
    ) -> Option<PopupDismissal> {
        let opener = self
            .opener
            .filter(|opener| opener.window == source_window)?;
        self.opener = None;
        Some(PopupDismissal {
            source_window,
            reason,
            opener: Some(opener),
        })
    }
}

/// A button whose expanded state follows the host's retained popover state.
#[must_use]
pub fn popover_trigger<A>(
    root: WidgetIdentity,
    label: WidgetIdentity,
    text: impl Into<String>,
    open: bool,
    action: A,
    state: ControlState,
    theme: WidgetTheme,
) -> Widget<A> {
    let text = text.into();
    let mut trigger = button(root, label, &text, action, state, theme);
    trigger.element.semantic = Some(Semantic::new(SemanticRole::Button, &text).expanded(open));
    trigger
}

/// Projects an anchored popover without owning its content or actions.
/// The root has a stable identity; the content keeps its original typed
/// action identities.
#[must_use]
pub fn popover<A>(
    root: WidgetIdentity,
    content: Widget<A>,
    anchor: Rect,
    bounds: Rect,
    size: Size,
    theme: WidgetTheme,
) -> Widget<A> {
    let position = place_popup(anchor, size, bounds, PopupDirection::Below);
    let element = Element::container(root.element, root.key)
        .semantic(Semantic::new(SemanticRole::Group, "Popover"))
        .styled(
            Style::default()
                .absolute(position.x, position.y)
                .size(Dimension::Px(size.width), Dimension::Px(size.height))
                .axis(Axis::Column)
                .padding(Insets::all(8.0))
                .background(theme.palette.overlay)
                .border(1.0, theme.palette.focus)
                .radius(theme.metrics.radius)
                .z_index(82)
                .pointer_events(true),
        )
        .child(content.element);
    Widget {
        element,
        actions: content.actions,
    }
}
