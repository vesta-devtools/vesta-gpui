//! Existing toolbar actions in both acceptance pages, with window-owned focus.

use super::*;

#[derive(Clone, Copy)]
pub(crate) enum Host {
    Controls,
    Layout,
}

impl Host {
    fn base(self) -> u32 {
        match self {
            Self::Controls => TOOLBAR_ROOT,
            Self::Layout => TOOLBAR_ROOT + 100,
        }
    }
}

pub(crate) fn sample(
    input: &UiBuildState<'_>,
    host: Host,
    actions: &mut Vec<(ElementId, DemoAction)>,
) -> Element {
    let scope = input.scopes.gallery;
    let base = host.base();
    let identity = |slot, part: &str| {
        WidgetIdentity::new(
            scope.element(base + slot),
            match host {
                Host::Controls => format!("gallery.toolbar.{part}"),
                Host::Layout => format!("gallery.layout.toolbar.{part}"),
            },
        )
    };
    let items = [
        (
            "−",
            "Decrease value",
            None,
            0,
            ControlState::Idle,
            DemoAction::StepNumber(-1),
        ),
        (
            "◐",
            "Switch theme",
            Some("T"),
            0,
            ControlState::Idle,
            DemoAction::ToggleTheme,
        ),
        (
            "P",
            "Toggle pinned",
            None,
            1,
            ControlState::Idle,
            DemoAction::ToggleCompactButton,
        ),
        (
            "−",
            "Unavailable toolbar action",
            None,
            1,
            ControlState::Disabled,
            DemoAction::StepNumber(-99),
        ),
    ]
    .into_iter()
    .enumerate()
    .map(
        |(i, (icon, name, shortcut, group, state, action))| ToolbarItem {
            root: identity(10 + i as u32 * 4, &format!("item.{i}")),
            glyph: identity(11 + i as u32 * 4, &format!("item.{i}.glyph")),
            label: identity(12 + i as u32 * 4, &format!("item.{i}.label")),
            separator: identity(13 + i as u32 * 4, &format!("item.{i}.separator")),
            icon: icon.into(),
            name: name.into(),
            shortcut: shortcut.map(str::to_owned),
            group,
            state,
            action,
        },
    )
    .collect();
    let widget = toolbar(
        ToolbarIds {
            root: identity(0, "root"),
            row: identity(1, "row"),
            overflow: identity(2, "overflow"),
            overflow_label: identity(3, "overflow.label"),
            hidden: identity(4, "hidden"),
        },
        items,
        ((input.viewport_size.width - 100.0) / 4.0).clamp(70.0, 150.0),
        input.demo.toolbar_overflow_open,
        DemoAction::ToggleToolbarOverflow,
        input.theme,
    );
    let mut element = widget_element(widget, actions);
    element.semantic = Some(Semantic::new(SemanticRole::Group, "Ordered toolbar"));
    element
}

impl App {
    fn toolbar_opener(&self, window: UiWindowId, hidden_only: bool) -> Option<ElementId> {
        let scope = self.gallery_scopes(window)?;
        let focus = self.window_focus(window)?;
        let route = self.window_actions(window)?;
        [Host::Controls, Host::Layout].into_iter().find_map(|host| {
            route
                .is_descendant_of(
                    focus,
                    scope.element(host.base() + if hidden_only { 4 } else { 0 }),
                )
                .then_some(scope.element(host.base() + 2))
        })
    }

    pub(crate) fn close_toolbar_overflow(&mut self, window: UiWindowId) {
        if self.demo.toolbar_overflow_open {
            let opener = self.toolbar_opener(window, true);
            self.demo.toolbar_overflow_open = false;
            if let Some(opener) = opener {
                self.set_window_focus(window, Some(opener));
            }
        }
    }

    pub(crate) fn dismiss_toolbar_overflow(&mut self, window: UiWindowId) -> bool {
        if self.demo.toolbar_overflow_open && self.toolbar_opener(window, false).is_some() {
            self.close_toolbar_overflow(window);
            true
        } else {
            false
        }
    }
}
