//! Host-owned modal stack and shared dialog projection.

use crate::{
    ControlState, PopupCloseReason, PopupDismissal, Widget, WidgetIdentity, WidgetTheme, button,
};
use vesta_gpui::{Align, Axis, Color, Dimension, Element, Semantic, SemanticRole, Size, Style};
use vesta_gpui::{InputTarget, UiWindowId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModalStackError {
    Duplicate,
    Full,
}

impl std::fmt::Display for ModalStackError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Duplicate => "modal is already in the stack",
            Self::Full => "modal stack capacity reached",
        })
    }
}
impl std::error::Error for ModalStackError {}

#[derive(Clone, Debug)]
pub struct ModalStack<Id> {
    entries: Vec<Id>,
    owners: Vec<Option<(UiWindowId, Option<InputTarget>, ModalDismissPolicy)>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModalDismissPolicy {
    pub explicit: bool,
    pub outside_press: bool,
}
impl Default for ModalDismissPolicy {
    fn default() -> Self {
        Self {
            explicit: true,
            outside_press: false,
        }
    }
}

impl<Id> Default for ModalStack<Id> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            owners: Vec::new(),
        }
    }
}

impl<Id: Copy + Eq> ModalStack<Id> {
    pub const CAPACITY: usize = 4;

    pub fn open(&mut self, id: Id) -> Result<(), ModalStackError> {
        if self.entries.contains(&id) {
            return Err(ModalStackError::Duplicate);
        }
        if self.entries.len() >= Self::CAPACITY {
            return Err(ModalStackError::Full);
        }
        self.entries.push(id);
        self.owners.push(None);
        Ok(())
    }

    pub fn open_in(
        &mut self,
        window: UiWindowId,
        id: Id,
        opener: Option<InputTarget>,
        policy: ModalDismissPolicy,
    ) -> Result<(), ModalStackError> {
        self.open(id)?;
        if let Some(owner) = self.owners.last_mut() {
            *owner = Some((
                window,
                opener.filter(|target| target.window == window),
                policy,
            ));
        }
        Ok(())
    }

    /// Only the top dialog and its source window may close. Teardown is always permitted.
    pub fn dismiss(
        &mut self,
        source: UiWindowId,
        id: Id,
        reason: PopupCloseReason,
    ) -> Option<PopupDismissal> {
        if self.top() != Some(id) {
            return None;
        }
        let owner = self.owners.last().copied().flatten();
        if let Some((window, _, policy)) = owner
            && (window != source
                || reason == PopupCloseReason::OutsidePress && !policy.outside_press
                || reason == PopupCloseReason::Explicit && !policy.explicit)
        {
            return None;
        }
        let opener = owner.and_then(|(_, opener, _)| opener);
        self.close_top(id);
        Some(PopupDismissal {
            source_window: source,
            reason,
            opener,
        })
    }

    pub fn close_top(&mut self, id: Id) -> bool {
        if self.top() == Some(id) {
            self.entries.pop();
            self.owners.pop();
            true
        } else {
            false
        }
    }

    #[must_use]
    pub fn top(&self) -> Option<Id> {
        self.entries.last().copied()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModalIds {
    pub backdrop: WidgetIdentity,
    pub panel: WidgetIdentity,
    pub title: WidgetIdentity,
    pub body: WidgetIdentity,
    pub footer: WidgetIdentity,
    pub confirm: WidgetIdentity,
    pub confirm_label: WidgetIdentity,
    pub cancel: WidgetIdentity,
    pub cancel_label: WidgetIdentity,
    pub secondary: WidgetIdentity,
    pub secondary_label: WidgetIdentity,
}

pub struct ModalDialogSpec<'a, A> {
    pub title: &'a str,
    pub body: &'a str,
    pub confirm: A,
    pub cancel: A,
    pub secondary: Option<(&'a str, A)>,
}

pub fn modal_dialog<A>(
    ids: ModalIds,
    viewport: Size,
    spec: ModalDialogSpec<'_, A>,
    theme: WidgetTheme,
) -> Widget<A> {
    modal_dialog_with_labels(ids, viewport, spec, "Confirm", "Cancel", theme)
}

/// Shared modal with Host-localized button captions and matching semantics.
/// The original constructor retains its default labels and presentation.
pub fn modal_dialog_with_labels<A>(
    ids: ModalIds,
    viewport: Size,
    spec: ModalDialogSpec<'_, A>,
    confirm_label: &str,
    cancel_label: &str,
    theme: WidgetTheme,
) -> Widget<A> {
    let ModalDialogSpec {
        title,
        body,
        confirm: confirm_action,
        cancel: cancel_action,
        secondary,
    } = spec;
    let width = (viewport.width - 32.0).clamp(160.0, 420.0);
    let height = if secondary.is_some() { 204.0 } else { 180.0 };
    let mut footer = Widget::from_element(
        Element::container(ids.footer.element, ids.footer.key).styled(
            Style::default()
                .axis(Axis::Row)
                .align(Align::Center)
                .gap(8.0),
        ),
    );
    if let Some((label, action)) = secondary {
        footer = footer.child(button(
            ids.secondary,
            ids.secondary_label,
            label,
            action,
            ControlState::Idle,
            theme,
        ));
    }
    footer = footer
        .child(button(
            ids.cancel,
            ids.cancel_label,
            cancel_label,
            cancel_action,
            ControlState::Idle,
            theme,
        ))
        .child(button(
            ids.confirm,
            ids.confirm_label,
            confirm_label,
            confirm_action,
            ControlState::Idle,
            theme,
        ));
    let panel = Widget::from_element(
        Element::container(ids.panel.element, ids.panel.key)
            .semantic(Semantic::new(SemanticRole::Dialog, title))
            .styled(
                Style::default()
                    .absolute(
                        ((viewport.width - width) / 2.0).max(0.0),
                        ((viewport.height - height) / 2.0).max(0.0),
                    )
                    .size(Dimension::Px(width), Dimension::Px(height))
                    .axis(Axis::Column)
                    .gap(12.0)
                    .padding(vesta_gpui::Insets::all(16.0))
                    .background(theme.palette.menu_surface)
                    .border(1.0, theme.palette.border),
            )
            .child(
                Element::text(ids.title.element, ids.title.key, title).styled(
                    Style::default()
                        .text(theme.typography.heading_font_size, theme.palette.text)
                        .pointer_events(false),
                ),
            )
            .child(
                Element::text(ids.body.element, ids.body.key, body).styled(
                    Style::default()
                        .size(Dimension::Percent(1.0), Dimension::Auto)
                        .flex(1.0, 1.0)
                        .text(theme.typography.font_size, theme.palette.text_muted)
                        .text_wrap(true)
                        .pointer_events(false),
                ),
            ),
    )
    .child(footer);
    Widget::from_element(
        Element::container(ids.backdrop.element, ids.backdrop.key).styled(
            Style::default()
                .absolute(0.0, 0.0)
                .size(
                    Dimension::Px(viewport.width),
                    Dimension::Px(viewport.height),
                )
                .background(Color::srgba(0.0, 0.0, 0.0, 0.58))
                .z_index(200),
        ),
    )
    .child(panel)
}

#[cfg(test)]
mod tests {
    use super::*;
    use vesta_gpui::ElementId;

    fn id(value: u64) -> WidgetIdentity {
        WidgetIdentity::new(ElementId::new(value), format!("modal.{value}"))
    }

    #[test]
    fn modal_stack_enforces_lifo_and_dialog_keeps_typed_actions() {
        let mut stack = ModalStack::default();
        assert_eq!(stack.open(1_u8), Ok(()));
        assert_eq!(stack.open(2), Ok(()));
        assert_eq!(stack.open(2), Err(ModalStackError::Duplicate));
        assert!(!stack.close_top(1));
        assert_eq!(stack.top(), Some(2));
        assert!(stack.close_top(2));
        assert_eq!(stack.top(), Some(1));
        let ids = ModalIds {
            backdrop: id(1),
            panel: id(2),
            title: id(3),
            body: id(4),
            footer: id(5),
            confirm: id(6),
            confirm_label: id(7),
            cancel: id(8),
            cancel_label: id(9),
            secondary: id(10),
            secondary_label: id(11),
        };
        let widget = modal_dialog(
            ids.clone(),
            Size::new(800.0, 600.0),
            ModalDialogSpec {
                title: "Confirm",
                body: "Body",
                confirm: 20_u8,
                cancel: 21,
                secondary: Some(("More", 22)),
            },
            WidgetTheme::dark(),
        );
        assert_eq!(widget.action_at(ElementId::new(6)), Some(&20));
        assert_eq!(widget.action_at(ElementId::new(8)), Some(&21));
        assert_eq!(widget.action_at(ElementId::new(10)), Some(&22));
        let localized = modal_dialog_with_labels(
            ids,
            Size::new(800.0, 600.0),
            ModalDialogSpec {
                title: "Close",
                body: "Body",
                confirm: 20_u8,
                cancel: 21,
                secondary: Some(("Discard", 22)),
            },
            "保存并关闭",
            "取消",
            WidgetTheme::light(),
        );
        fn find(element: &Element, id: ElementId) -> Option<&Element> {
            if element.id == id {
                return Some(element);
            }
            element.children.iter().find_map(|child| find(child, id))
        }
        for (control, label, caption, action) in [
            (6, 7, "保存并关闭", 20),
            (8, 9, "取消", 21),
            (10, 11, "Discard", 22),
        ] {
            let control = ElementId::new(control);
            assert_eq!(localized.action_at(control), Some(&action));
            assert_eq!(
                find(&localized.element, control)
                    .expect("control")
                    .semantic
                    .as_ref()
                    .expect("button semantics")
                    .label,
                caption
            );
            assert_eq!(
                find(&localized.element, ElementId::new(label))
                    .expect("caption")
                    .kind,
                vesta_gpui::ElementKind::Text(caption.into())
            );
        }
    }
}
