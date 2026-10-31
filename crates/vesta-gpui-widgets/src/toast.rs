//! Bounded transient notifications driven by a host clock.

use std::{collections::VecDeque, time::Instant};

use crate::{ControlState, StatusTone, Widget, WidgetIdentity, WidgetTheme, button};
use vesta_gpui::{Align, Axis, Dimension, Element, Insets, Semantic, SemanticRole, Style};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ToastId(pub u32);

#[derive(Clone, Debug, PartialEq)]
pub struct ToastNotice<A> {
    pub id: ToastId,
    pub message: String,
    pub tone: StatusTone,
    pub action_label: Option<String>,
    pub action: Option<A>,
    pub expires_at: Instant,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToastQueueError {
    Duplicate(ToastId),
    Full,
    InvalidTimeout,
}

impl std::fmt::Display for ToastQueueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Duplicate(id) => write!(f, "duplicate notification identity {id:?}"),
            Self::Full => f.write_str("notification queue capacity reached"),
            Self::InvalidTimeout => f.write_str("notification deadline must be in the future"),
        }
    }
}
impl std::error::Error for ToastQueueError {}

#[derive(Clone, Debug)]
pub struct ToastQueue<A> {
    entries: VecDeque<ToastNotice<A>>,
}

impl<A> Default for ToastQueue<A> {
    fn default() -> Self {
        Self {
            entries: VecDeque::new(),
        }
    }
}

impl<A> ToastQueue<A> {
    pub const CAPACITY: usize = 4;

    pub fn push(&mut self, notice: ToastNotice<A>, now: Instant) -> Result<(), ToastQueueError> {
        self.expire(now);
        if self.entries.iter().any(|entry| entry.id == notice.id) {
            return Err(ToastQueueError::Duplicate(notice.id));
        }
        if self.entries.len() >= Self::CAPACITY {
            return Err(ToastQueueError::Full);
        }
        if notice.expires_at <= now {
            return Err(ToastQueueError::InvalidTimeout);
        }
        self.entries.push_back(notice);
        Ok(())
    }

    pub fn expire(&mut self, now: Instant) -> bool {
        let before = self.entries.len();
        self.entries.retain(|notice| notice.expires_at > now);
        self.entries.len() != before
    }

    pub fn dismiss(&mut self, id: ToastId) -> bool {
        let before = self.entries.len();
        self.entries.retain(|notice| notice.id != id);
        self.entries.len() != before
    }

    #[must_use]
    pub fn current(&self) -> Option<&ToastNotice<A>> {
        self.entries.front()
    }

    #[must_use]
    pub fn next_deadline(&self) -> Option<Instant> {
        self.entries.iter().map(|notice| notice.expires_at).min()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToastIds {
    pub root: WidgetIdentity,
    pub text: WidgetIdentity,
    pub action: WidgetIdentity,
    pub action_label: WidgetIdentity,
    pub dismiss: WidgetIdentity,
    pub dismiss_label: WidgetIdentity,
}

/// Project the current notice. The host applies its action and dismiss command.
#[must_use]
pub fn toast_widget<A: Clone>(
    ids: ToastIds,
    notice: &ToastNotice<A>,
    dismiss_action: A,
    theme: WidgetTheme,
) -> Widget<A> {
    let color = match notice.tone {
        StatusTone::Information => theme.palette.accent,
        StatusTone::Success => theme.palette.success,
        StatusTone::Warning => theme.palette.warning,
        StatusTone::Error => theme.palette.danger,
    };
    let mut widget = Widget::from_element(
        Element::container(ids.root.element, ids.root.key)
            .semantic(Semantic::new(SemanticRole::Status, &notice.message))
            .styled(
                Style::default()
                    .size(Dimension::Percent(1.0), Dimension::Auto)
                    .min_height(Dimension::Px(theme.metrics.control_height + 12.0))
                    .max_width(Dimension::Px(360.0))
                    .axis(Axis::Row)
                    .align(Align::Center)
                    .gap(6.0)
                    .padding(Insets::all(8.0))
                    .background(theme.palette.field)
                    .border(1.0, color),
            )
            .child(
                Element::text(ids.text.element, ids.text.key, &notice.message).styled(
                    Style::default()
                        .size(Dimension::Auto, Dimension::Auto)
                        .min_width(Dimension::Px(0.0))
                        .flex(1.0, 1.0)
                        .text_wrap(true)
                        .text(theme.typography.font_size, theme.palette.text)
                        .pointer_events(false),
                ),
            ),
    );
    if let (Some(label), Some(action)) = (&notice.action_label, &notice.action) {
        widget = widget.child(button(
            ids.action,
            ids.action_label,
            label,
            action.clone(),
            ControlState::Idle,
            theme,
        ));
    }
    let mut dismiss = button(
        ids.dismiss,
        ids.dismiss_label,
        "×",
        dismiss_action,
        ControlState::Idle,
        theme,
    );
    dismiss.element.semantic = Some(Semantic::new(SemanticRole::Button, "Dismiss notification"));
    dismiss.element.style.width = Dimension::Px(theme.metrics.control_height);
    dismiss.element.style.padding = Insets::all(0.0);
    widget.child(dismiss)
}
