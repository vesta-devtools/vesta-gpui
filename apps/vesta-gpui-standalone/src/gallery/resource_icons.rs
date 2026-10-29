//! Existing resource pixels and host state through the public compact Button shell.

use super::*;
use vesta_gpui_widgets::{
    IconButtonContent, IconButtonPresentation, icon_button_with_presentation,
};

pub(super) enum Host {
    Controls,
    Layout,
}

pub(super) fn samples(
    scope: ElementScope,
    host: Host,
    selected: bool,
    theme: WidgetTheme,
    actions: &mut Vec<(ElementId, DemoAction)>,
) -> Vec<Element> {
    let (start, key) = match host {
        Host::Controls => (27_200_000, "controls"),
        Host::Layout => (27_200_100, "layout"),
    };
    [
        (
            "Pin sample with resource icon",
            Some("Space"),
            CompactButtonKind::Icon,
            ControlState::Idle,
        ),
        (
            "Toggle pinned resource",
            Some("Enter"),
            CompactButtonKind::Toolbar,
            ControlState::Idle,
        ),
        (
            "Unavailable resource action",
            None,
            CompactButtonKind::Toolbar,
            ControlState::Disabled,
        ),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (name, shortcut, kind, state))| {
        let base = start + index as u32 * 2;
        widget_element(
            icon_button_with_presentation(
                WidgetIdentity::new(
                    scope.element(base),
                    format!("resource-icon.{key}.{index}.root"),
                ),
                WidgetIdentity::new(
                    scope.element(base + 1),
                    format!("resource-icon.{key}.{index}.image"),
                ),
                IconButtonSpec {
                    icon: "+",
                    name,
                    shortcut,
                    kind,
                    state,
                },
                DemoAction::ToggleCompactButton,
                IconButtonPresentation {
                    content: IconButtonContent::Image(if selected {
                        "demo://image-tall"
                    } else {
                        "demo://image-wide"
                    }),
                    selected: Some(selected),
                },
                theme,
            ),
            actions,
        )
    })
    .collect()
}
