//! One controlled location path in each acceptance page; focus stays window-local.

use super::*;

#[derive(Clone, Copy)]
pub(crate) enum Host {
    Collections,
    Layout,
}

impl Host {
    fn base(self) -> u32 {
        match self {
            Self::Collections => BREADCRUMB_ROOT,
            Self::Layout => BREADCRUMB_ROOT + 100,
        }
    }
}

pub(crate) fn samples(
    input: &UiBuildState<'_>,
    host: Host,
    actions: &mut Vec<(ElementId, DemoAction)>,
) -> [Element; 2] {
    let scope = input.scopes.gallery;
    let identity = |slot, part: &str| {
        WidgetIdentity::new(
            scope.element(host.base() + slot),
            match host {
                Host::Collections => format!("gallery.breadcrumb.{part}"),
                Host::Layout => format!("gallery.layout.breadcrumb.{part}"),
            },
        )
    };
    let depth = usize::from(input.demo.breadcrumb_depth);
    let segments = ["Vesta", "Project", "Assets", "Cube"]
        .into_iter()
        .enumerate()
        .take(depth + 1)
        .map(|(index, label)| BreadcrumbSegment {
            root: identity(10 + index as u32 * 3, &format!("segment.{index}")),
            separator: identity(11 + index as u32 * 3, &format!("segment.{index}.separator")),
            text: identity(12 + index as u32 * 3, &format!("segment.{index}.label")),
            label: label.into(),
            action: (index < depth).then_some(DemoAction::BreadcrumbAncestor(index as u8)),
        })
        .collect();
    let mut path = widget_element(
        breadcrumb(
            BreadcrumbIds {
                root: identity(0, "root"),
                row: identity(1, "row"),
                overflow: identity(2, "overflow"),
                overflow_label: identity(3, "overflow.label"),
                hidden: identity(4, "hidden"),
            },
            segments,
            ((input.viewport_size.width - 100.0) / 3.0).clamp(80.0, 400.0),
            input.demo.breadcrumb_expanded,
            DemoAction::ToggleBreadcrumbOverflow,
            input.theme,
        ),
        actions,
    );
    path.semantic = Some(Semantic::new(SemanticRole::Group, "Location path"));
    let next = widget_element(
        button(
            identity(5, "next"),
            identity(6, "next.label"),
            "Open child",
            DemoAction::BreadcrumbNext,
            if depth < 3 {
                ControlState::Idle
            } else {
                ControlState::Disabled
            },
            input.theme,
        ),
        actions,
    );
    [path, next]
}

impl App {
    fn focused_breadcrumb_host(&self, window: UiWindowId) -> Option<Host> {
        let scope = self.gallery_scopes(window)?;
        let route = self.window_actions(window)?;
        let focus = self.window_focus(window)?;
        [Host::Collections, Host::Layout].into_iter().find(|host| {
            focus == scope.element(host.base() + 5)
                || route.is_descendant_of(focus, scope.element(host.base()))
        })
    }

    pub(crate) fn focus_breadcrumb_navigation(&mut self, window: UiWindowId) {
        let Some(scope) = self.gallery_scopes(window) else {
            return;
        };
        let host = self.focused_breadcrumb_host(window).or_else(|| {
            let route = self.window_actions(window)?;
            [Host::Collections, Host::Layout]
                .into_iter()
                .find(|host| route.path_to(scope.element(host.base())).is_some())
        });
        if let Some(host) = host {
            let slot = if self.demo.breadcrumb_depth < 3 {
                5
            } else {
                10
            };
            self.set_window_focus(window, Some(scope.element(host.base() + slot)));
        }
    }

    pub(crate) fn dismiss_breadcrumb_overflow(&mut self, window: UiWindowId) -> bool {
        if !self.demo.breadcrumb_expanded {
            return false;
        }
        let Some(host) = self.focused_breadcrumb_host(window) else {
            return false;
        };
        let Some(scope) = self.gallery_scopes(window) else {
            return false;
        };
        let opener = scope.element(host.base() + 2);
        if self
            .window_actions(window)
            .is_none_or(|route| route.path_to(opener).is_none())
        {
            return false;
        }
        self.demo.breadcrumb_expanded = false;
        self.set_window_focus(window, Some(opener));
        true
    }
}
