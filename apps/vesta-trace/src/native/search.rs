//! Shared text editing/IME bridge; native host owns focus and query debounce.
use super::*;
use vesta_gpui::{PaintCommand, TextInputState, TextInputViewport};
use vesta_gpui_winit::{Clipboard, TextInputBridge, UiInputEventSource};

#[derive(Default)]
struct SystemClipboard(Option<arboard::Clipboard>);
impl SystemClipboard {
    fn get(&mut self) -> std::io::Result<&mut arboard::Clipboard> {
        if self.0.is_none() {
            self.0 = Some(arboard::Clipboard::new().map_err(std::io::Error::other)?);
        }
        self.0
            .as_mut()
            .ok_or_else(|| std::io::Error::other("clipboard initialization failed"))
    }
}
impl Clipboard for SystemClipboard {
    type Error = std::io::Error;
    fn read_text(&mut self) -> std::io::Result<String> {
        self.get()?.get_text().map_err(std::io::Error::other)
    }
    fn write_text(&mut self, text: &str) -> std::io::Result<()> {
        self.get()?
            .set_text(text.to_owned())
            .map_err(std::io::Error::other)
    }
}
pub(super) struct Search {
    pub input: TextInputState,
    pub bridge: TextInputBridge,
    clipboard: SystemClipboard,
    pub view: Option<TextInputViewport>,
    scroll: Point,
    pub due: Option<Instant>,
}
impl Search {
    pub fn new(context: &mut TextInputContext, text: &str) -> Result<Self> {
        let mut input = TextInputState::new(14.0, "Segoe UI, Microsoft YaHei");
        input.reset(context, text)?;
        Ok(Self {
            input,
            bridge: Default::default(),
            clipboard: Default::default(),
            view: None,
            scroll: Default::default(),
            due: None,
        })
    }
    pub fn snapshot(&self) -> vesta_gpui::TextInputSnapshot {
        self.input
            .snapshot()
            .with_viewport_scroll(self.scroll, true)
    }
    pub fn clear_geometry(&mut self) {
        self.view = None;
        self.bridge.clear_text_pointer();
    }
    pub fn end_session(&mut self, context: &mut TextInputContext) {
        self.input.cancel_compose(context);
        self.input.end_group();
        self.bridge.clear_ime_session();
    }
    pub fn refresh_view(&mut self) {
        if let Some(old) = self.view {
            let view = self.snapshot().viewport(old.bounds, old.clip, self.scroll);
            self.scroll = Point::new(view.scroll_x, view.scroll_y);
            self.view = Some(view);
        }
    }
    pub fn install(&mut self, paint: &PaintList) {
        self.view = paint.commands().iter().find_map(|command| match command {
            PaintCommand::TextInput {
                element,
                bounds,
                clip,
                snapshot,
                ..
            } if *element == view::SEARCH => Some(snapshot.viewport(*bounds, *clip, self.scroll)),
            _ => None,
        });
        if let Some(view) = self.view {
            self.scroll = Point::new(view.scroll_x, view.scroll_y);
        }
    }
    pub fn edit(
        &mut self,
        event: &impl UiInputEventSource,
        context: &mut TextInputContext,
    ) -> std::result::Result<bool, TraceError> {
        let response = self
            .bridge
            .handle_window_event(event, &mut self.input, context, &mut self.clipboard, false)
            .map_err(|e| TraceError::new("trace_search_input", e.to_string()))?;
        if response.is_handled() {
            self.refresh_view();
            if !self.input.is_composing() {
                self.due = Some(Instant::now() + Duration::from_millis(180));
            }
        }
        Ok(response.is_handled())
    }
}
impl App {
    pub(super) fn focus_changed(&mut self, old: Option<vesta_gpui::InputTarget>) {
        if old.is_some_and(|t| t.element == view::SEARCH) && !self.search_focused() {
            self.search.end_session(&mut self.context);
        }
        if let Some(window) = &self.window {
            self.search.bridge.sync_ime(
                window,
                self.search_focused() && self.active && !self.occluded,
                self.search.view,
            );
        }
        self.redraw();
    }
    pub(super) fn search_focused(&self) -> bool {
        self.ownership
            .focus()
            .is_some_and(|t| t.element == view::SEARCH)
    }
    pub(super) fn set_focus(&mut self, target: Option<vesta_gpui::InputTarget>) {
        let old = self.ownership.focus();
        self.ownership.set_focus(target);
        self.focus_changed(old);
    }
    pub(super) fn apply_search(&mut self) -> std::result::Result<(), TraceError> {
        self.search.due = None;
        if self.search.input.is_composing() {
            return Ok(());
        }
        if self.importing() {
            self.redraw();
            return Ok(());
        }
        let text = self.search.input.text().to_owned();
        if text.len() > 256 {
            self.status = "Search exceeds 256 UTF-8 bytes; previous filter retained".into();
            self.redraw();
            return Ok(());
        }
        if text != self.filter {
            self.cancel_drag();
            self.begin_change();
            self.filter = text;
            self.timer = None;
            self.reset_pages();
            self.reanalyze()?;
        }
        Ok(())
    }
}
