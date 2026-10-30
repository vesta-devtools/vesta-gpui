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

impl App {
    pub(super) fn request_redraw(&self) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    pub(super) fn request_all_redraw(&self) {
        self.request_redraw();
        for host in self.native_windows.values() {
            host.window.request_redraw();
        }
    }

    /// Closes the palette and modal dialogs when their owning window disappears, so an overlay
    /// can never stay open in a window that no longer exists (ADR-0254).
    pub(super) fn close_owned_overlays(&mut self) {
        if self.overlay_owner.is_none() {
            return;
        }
        while let Some(id) = self.demo.dialog_stack.top() {
            self.close_demo_dialog(id, false);
        }
        self.close_command_palette();
        self.overlay_owner = None;
    }

    pub(super) fn native_window_title(&self, id: FloatingWindowId) -> Option<String> {
        self.demo
            .workspace
            .floating_window(id)
            .map(|window| window.title.clone())
    }

    pub(super) fn native_accessibility_action(
        &mut self,
        window_id: WindowId,
        request: accesskit::ActionRequest,
    ) {
        if let Some(window) = self
            .native_windows
            .get(&window_id)
            .map(|host| host.ui_window)
        {
            self.window_accessibility_action(window, request);
        }
    }
}

#[derive(Default)]
pub(super) struct HostClipboard(Option<arboard::Clipboard>);

impl Clipboard for HostClipboard {
    type Error = String;

    fn read_text(&mut self) -> Result<String, Self::Error> {
        self.clipboard()?
            .get_text()
            .map_err(|error| error.to_string())
    }

    fn write_text(&mut self, text: &str) -> Result<(), Self::Error> {
        self.clipboard()?
            .set_text(text.to_owned())
            .map_err(|error| error.to_string())
    }
}

impl HostClipboard {
    fn clipboard(&mut self) -> Result<&mut arboard::Clipboard, String> {
        if self.0.is_none() {
            self.0 = Some(arboard::Clipboard::new().map_err(|error| error.to_string())?);
        }
        match self.0.as_mut() {
            Some(clipboard) => Ok(clipboard),
            None => Err("clipboard initialization returned no instance".to_owned()),
        }
    }
}

pub(super) fn chrome_resize_direction(cursor: Point, size: Size) -> Option<ResizeDirection> {
    if cursor.x < 0.0 || cursor.y < 0.0 || cursor.x > size.width || cursor.y > size.height {
        return None;
    }
    let west = cursor.x <= RESIZE_EDGE;
    let east = cursor.x >= size.width - RESIZE_EDGE;
    let north = cursor.y <= RESIZE_EDGE;
    let south = cursor.y >= size.height - RESIZE_EDGE;
    match (west, east, north, south) {
        (true, _, true, _) => Some(ResizeDirection::NorthWest),
        (_, true, true, _) => Some(ResizeDirection::NorthEast),
        (true, _, _, true) => Some(ResizeDirection::SouthWest),
        (_, true, _, true) => Some(ResizeDirection::SouthEast),
        (true, _, _, _) => Some(ResizeDirection::West),
        (_, true, _, _) => Some(ResizeDirection::East),
        (_, _, true, _) => Some(ResizeDirection::North),
        (_, _, _, true) => Some(ResizeDirection::South),
        _ => None,
    }
}
