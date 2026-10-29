//! Styled text runs with independently addressable links.

use crate::{Widget, WidgetIdentity, WidgetTheme};
use vesta_gpui::{Align, Axis, Dimension, Element, Semantic, SemanticRole, Style};

#[must_use]
pub fn rich_text_paragraph(
    identity: WidgetIdentity,
    accessible_name: &str,
    snapshot: vesta_gpui::TextInputSnapshot,
    theme: WidgetTheme,
) -> Element {
    let height = snapshot.layout_height() + 4.0;
    Element::text_input(identity.element, identity.key, snapshot, false)
        .text_source(vesta_gpui::TextSource::Literal)
        .semantic(Semantic::new(SemanticRole::Label, accessible_name))
        .styled(
            Style::default()
                .size(Dimension::Percent(1.0), Dimension::Px(height.max(4.0)))
                .min_width(Dimension::Px(0.0))
                .text(theme.typography.font_size, theme.palette.text)
                .pointer_events(false),
        )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RichTextTone {
    Normal,
    Muted,
    Emphasis,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RichTextRun<A> {
    pub identity: WidgetIdentity,
    pub text: String,
    pub tone: RichTextTone,
    pub action: Option<A>,
    pub enabled: bool,
}

/// Compose styled runs. A run with an action becomes a keyboard-operable link.
#[must_use]
pub fn rich_text<A>(
    root: WidgetIdentity,
    accessible_name: &str,
    runs: Vec<RichTextRun<A>>,
    theme: WidgetTheme,
) -> Widget<A> {
    let mut widget = Widget::from_element(
        Element::container(root.element, root.key)
            .semantic(Semantic::new(SemanticRole::Group, accessible_name))
            .styled(
                Style::default()
                    .size(Dimension::Auto, Dimension::Auto)
                    .axis(Axis::Row)
                    .align(Align::Center),
            ),
    );
    for run in runs {
        let is_link = run.action.is_some();
        let color = if is_link {
            if run.enabled {
                theme.palette.accent
            } else {
                theme.palette.text_disabled
            }
        } else {
            match run.tone {
                RichTextTone::Normal => theme.palette.text,
                RichTextTone::Muted => theme.palette.text_muted,
                RichTextTone::Emphasis => theme.palette.accent,
            }
        };
        let mut span = Element::text(run.identity.element, run.identity.key, &run.text)
            .semantic(Semantic::new(
                if is_link {
                    SemanticRole::Link
                } else {
                    SemanticRole::Label
                },
                &run.text,
            ))
            .styled(
                Style::default()
                    .text(theme.typography.font_size, color)
                    .pointer_events(is_link && run.enabled)
                    .focusable(is_link && run.enabled)
                    .action_boundary(is_link && !run.enabled),
            );
        if is_link {
            span.tooltip = Some(run.text.clone());
        }
        let mut part = Widget::from_element(span);
        if run.enabled
            && let Some(action) = run.action
        {
            part = part.on_click(action);
        }
        widget = widget.child(part);
    }
    widget
}

#[cfg(test)]
mod tests {
    use super::*;
    use vesta_gpui::ElementId;

    #[test]
    fn link_runs_keep_typed_actions_and_disabled_links_leave_the_focus_order() {
        let identity = |value, key| WidgetIdentity::new(ElementId::new(value), key);
        let widget = rich_text(
            identity(1, "root"),
            "Help",
            vec![
                RichTextRun {
                    identity: identity(2, "plain"),
                    text: "Open ".into(),
                    tone: RichTextTone::Normal,
                    action: None,
                    enabled: true,
                },
                RichTextRun {
                    identity: identity(3, "active"),
                    text: "Viewport".into(),
                    tone: RichTextTone::Emphasis,
                    action: Some(7_u8),
                    enabled: true,
                },
                RichTextRun {
                    identity: identity(4, "disabled"),
                    text: " Later".into(),
                    tone: RichTextTone::Muted,
                    action: Some(8_u8),
                    enabled: false,
                },
            ],
            WidgetTheme::dark(),
        );
        assert_eq!(widget.action_at(ElementId::new(3)), Some(&7));
        assert!(widget.action_at(ElementId::new(4)).is_none());
        let active = widget.element.find(ElementId::new(3)).expect("active link");
        let disabled = widget
            .element
            .find(ElementId::new(4))
            .expect("disabled link");
        assert_eq!(
            active.semantic.as_ref().expect("link semantic").role,
            SemanticRole::Link
        );
        assert!(active.style.focusable);
        assert!(!disabled.style.focusable);
        let paragraph = rich_text_paragraph(
            identity(5, "paragraph"),
            "Read only",
            vesta_gpui::TextInputSnapshot::default(),
            WidgetTheme::dark(),
        );
        assert_eq!(
            paragraph.semantic.as_ref().expect("paragraph").role,
            SemanticRole::Label
        );
        assert!(!paragraph.style.focusable && !paragraph.style.pointer_events);
    }
}
