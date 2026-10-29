//! Main and floating windows consume the shared deliberate-focus tooltip policy.
use crate::*;
use vesta_gpui::LayoutError;

/// Tooltip discovery uses the base route; later consumers need the final tree.
pub(crate) fn route_after_tooltip(
    root: &Element,
    base_children: usize,
    route: ActionRoute<DemoAction>,
    actions: Vec<(ElementId, DemoAction)>,
) -> Result<ActionRoute<DemoAction>, vesta_gpui::ActionRouteError> {
    if root.children.len() == base_children {
        Ok(route)
    } else {
        // TooltipController appends a passive root child. Keep the complete
        // presented ancestry with the original typed actions and focus order.
        ActionRoute::build(root, actions)
    }
}

impl App {
    pub(crate) fn layout_main_tooltips(
        &mut self,
        root: &mut Element,
        route: &ActionRoute<DemoAction>,
        viewport: Viewport,
        active: bool,
        focus_root: Option<ElementId>,
    ) -> Result<LayoutTree, LayoutError> {
        self.tooltip.layout(
            root,
            TooltipFrame {
                scope: self.scopes.tooltip,
                viewport,
                route,
                focus: self
                    .input_ownership
                    .visible_focus()
                    .map(|focus| focus.element),
                focus_root: self
                    .demo
                    .menu
                    .is_open()
                    .then_some(self.scopes.menu.element(1))
                    .or(focus_root),
                cursor: self.cursor,
                active,
                theme: self.theme,
                now: Instant::now(),
            },
            &mut self.text_context,
        )
    }

    pub(crate) fn layout_floating_tooltips(
        &mut self,
        host: &mut NativeFloatingHost,
        root: &mut Element,
        route: &ActionRoute<DemoAction>,
        viewport: Viewport,
        active: bool,
        focus_root: Option<ElementId>,
    ) -> Result<LayoutTree, LayoutError> {
        host.tooltip.layout(
            root,
            TooltipFrame {
                scope: host.scopes.tooltip,
                viewport,
                route,
                focus: host.input.visible_focus().map(|focus| focus.element),
                focus_root: host
                    .pane_menu
                    .as_ref()
                    .map(|_| host.scopes.menu.element(1))
                    .or(focus_root),
                cursor: host.cursor,
                active,
                theme: self.theme,
                now: Instant::now(),
            },
            &mut self.text_context,
        )
    }
}
