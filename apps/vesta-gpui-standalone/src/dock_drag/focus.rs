//! One accepted Dock action transfers focus through the destination's next route.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DockFocusTransfer {
    item: ItemId,
    destination: ContainerRef,
    native: bool,
}

impl DockFocusTransfer {
    pub(crate) fn after_drop(
        item: ItemId,
        source: ContainerRef,
        model: &vesta_workspace::WorkspaceModel,
    ) -> Option<Self> {
        let destination = model.container_of_item(item)?;
        Some(Self {
            item,
            destination,
            native: source != destination,
        })
    }

    pub(crate) fn destination(self) -> ContainerRef {
        self.destination
    }

    pub(crate) fn is_current(self, model: &vesta_workspace::WorkspaceModel) -> bool {
        model.container_of_item(self.item) == Some(self.destination)
            && model.active_item().is_some_and(|item| item.id == self.item)
    }

    /// Consume only at the current destination; shared focus admission remains final.
    pub(crate) fn request(
        pending: &mut Option<Self>,
        model: &vesta_workspace::WorkspaceModel,
        container: ContainerRef,
        window: UiWindowId,
        actions: &ActionRoute<DemoAction>,
        root: Option<ElementId>,
        input: &mut InputOwnership,
    ) -> bool {
        let Some(transfer) = *pending else {
            return false;
        };
        if !transfer.is_current(model) {
            *pending = None;
            return false;
        }
        if transfer.destination != container {
            return false;
        }
        *pending = None;
        let Some(element) = actions.focus_order().iter().copied().find(|element| {
            actions.action_at(*element)
                == Some(&DemoAction::Workspace(WorkspaceViewAction::Tab(
                    transfer.item,
                )))
        }) else {
            return false;
        };
        let target = InputTarget { window, element };
        if !FocusScope::new(window, actions, root).accepts(target) {
            return false;
        }
        input.observe_focus_input(vesta_gpui::FocusInput::Pointer);
        input.request_focus(target);
        transfer.native
    }
}
