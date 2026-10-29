//! Native-only platform observation precedes shared logical dispatch.
use super::*;
impl App {
    pub(super) fn native_platform_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        id: WindowId,
        event: NativeWindowEvent,
    ) {
        let window = if self.window.as_ref().is_some_and(|window| window.id() == id) {
            Some(STANDALONE_WINDOW)
        } else {
            self.native_windows.get(&id).map(|host| host.ui_window)
        };
        if let Some(window) = window {
            self.lifecycle_profile.native_event(window, &event);
        }
        if self.window.as_ref().is_some_and(|window| window.id() == id) {
            if let (Some(adapter), Some(window)) = (&mut self.accessibility, &self.window) {
                adapter.process_event(window, &event);
            }
        } else if let Some(host) = self.native_windows.get_mut(&id) {
            host.accessibility.process_event(&host.window, &event);
        }
        let input = WindowEvent::from(&event);
        if let Some(container) = window.and_then(|owner| self.window_container(owner)) {
            self.observe_workspace_geometry(container, &input);
        }
        if self.ui_query.enabled && input.changes_ui() && window.is_some() {
            self.ui_query.invalidate_shared();
            self.request_all_redraw();
        }
        let observation = native_capture::Event::from(&input);
        let control_key = self.observe_native_control_key(id, &input);
        self.dispatch_window_event(event_loop, id, input);
        if let Some(observation) = control_key {
            self.observe_native_control_return(observation);
        }
        self.observe_native_capture(event_loop, id, observation);
    }
}
