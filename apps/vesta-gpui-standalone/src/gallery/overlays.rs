//! Gallery-owned transient overlays and current-tree action membership.
use super::*;

pub(super) fn compose(
    input: &UiBuildState<'_>,
    mut root: Element,
    ids: &mut Ids,
    actions: &mut Vec<(ElementId, DemoAction)>,
    combo_open: bool,
    popover_open: bool,
) -> Element {
    let scope = input.scopes.gallery;
    let theme = input.theme;
    let demo = input.demo;
    let bounds = Rect::new(
        8.0,
        CHROME_HEIGHT + 8.0,
        (input.viewport_size.width - 16.0).max(0.0),
        (input.viewport_size.height - CHROME_HEIGHT - CHROME_STATUS_HEIGHT - 16.0).max(0.0),
    );
    if combo_open {
        root = root.child(widget_element(
            build_combo_popup(
                scope,
                ids,
                theme,
                demo.combo_selected,
                input
                    .combo_anchor
                    .unwrap_or(Rect::new(24.0, 170.0, 220.0, 32.0)),
                bounds,
            ),
            actions,
        ));
    }
    if popover_open {
        root = root.child(widget_element(
            build_popover(
                input.scopes.popover,
                theme,
                demo.number,
                input
                    .popover_anchor
                    .unwrap_or(Rect::new(24.0, 240.0, 120.0, 32.0)),
                bounds,
            ),
            actions,
        ));
    }
    if let Some(notice) = demo.toast_queue.current() {
        let base = TOAST_ROOT + notice.id.0 * 6;
        let identity = |slot, part| {
            WidgetIdentity::new(
                scope.element(base + slot),
                format!("gallery.toast.{}.{part}", notice.id.0),
            )
        };
        let mut toast = toast_widget(
            ToastIds {
                root: identity(0, "root"),
                text: identity(1, "text"),
                action: identity(2, "action"),
                action_label: identity(3, "action.label"),
                dismiss: identity(4, "dismiss"),
                dismiss_label: identity(5, "dismiss.label"),
            },
            notice,
            DemoAction::DismissToast(notice.id),
            theme,
        );
        toast.element.style.position = PositionMode::Absolute;
        toast.element.style.width =
            Dimension::Px((input.viewport_size.width - 16.0).clamp(0.0, 360.0));
        toast.element.style.right = Some(8.0);
        toast.element.style.bottom = Some(
            8.0 + if input.workspace_container == ContainerRef::Main {
                CHROME_STATUS_HEIGHT
            } else {
                0.0
            },
        );
        toast.element.style.z_index = 140;
        root = root.child(widget_element(toast, actions));
    }
    // Inactive pages are constructed by this fixture too. Retain the same action order and
    // membership using one tree walk, instead of walking the root once per candidate action.
    // ActionRoute remains responsible for identity validity, enabled state and modal routing.
    let mut live = HashSet::new();
    collect_ids(&root, &mut live);
    actions.retain(|(id, _)| live.contains(id));
    root
}

fn collect_ids(element: &Element, live: &mut HashSet<ElementId>) {
    live.insert(element.id);
    for child in &element.children {
        collect_ids(child, live);
    }
}
