//! Notification focus returns are window-qualified and validated against the current tree.
use crate::ToastId;
use std::{collections::HashMap, time::Instant};
use vesta_gpui::{ElementId, FocusScope, InputTarget, UiWindowId};

#[derive(Default)]
pub struct NotificationFocusState {
    snapshots: HashMap<ToastId, (Instant, HashMap<UiWindowId, InputTarget>)>,
    inactive_openers: HashMap<UiWindowId, InputTarget>,
}
impl NotificationFocusState {
    /// Observe activation before clearing live ownership on blur. This retains
    /// only notification return history, never active focus or pointer capture.
    pub fn observe_window_activation(
        &mut self,
        window: UiWindowId,
        active: bool,
        focused: Option<InputTarget>,
    ) {
        self.inactive_openers.remove(&window);
        if !active && let Some(target) = focused.filter(|target| target.window == window) {
            self.inactive_openers.insert(window, target);
        }
    }

    /// A closed window cannot receive any existing or future notification return.
    pub fn forget_window(&mut self, window: UiWindowId) {
        self.inactive_openers.remove(&window);
        for (_, targets) in self.snapshots.values_mut() {
            targets.remove(&window);
        }
    }

    pub fn remove(&mut self, id: &ToastId) {
        self.snapshots.remove(id);
    }
    pub fn is_empty(&self) -> bool {
        self.snapshots.is_empty()
    }
    pub fn len(&self) -> usize {
        self.snapshots.len()
    }
    pub fn record(
        &mut self,
        id: ToastId,
        expires: Instant,
        targets: impl IntoIterator<Item = InputTarget>,
    ) {
        let mut openers = self.inactive_openers.clone();
        openers.extend(targets.into_iter().map(|target| (target.window, target)));
        self.snapshots.insert(id, (expires, openers));
    }
    pub fn opener(&self, id: ToastId, window: UiWindowId) -> Option<InputTarget> {
        self.snapshots.get(&id)?.1.get(&window).copied()
    }
    pub fn return_target<A>(
        &self,
        id: ToastId,
        window: UiWindowId,
        focused: Option<InputTarget>,
        root: ElementId,
        scope: &FocusScope<'_, A>,
    ) -> Option<InputTarget> {
        if !scope
            .path_to(focused?)
            .is_some_and(|path| path.contains(&root))
        {
            return None;
        }
        scope.restore_target(self.opener(id, window)?)
    }
    pub fn prune(&mut self, now: Instant) {
        self.snapshots.retain(|_, (expires, _)| *expires > now);
    }
}
