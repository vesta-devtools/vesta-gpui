//! Platform adapter from winit events to backend-neutral `vesta-gpui` editing state.
//!
//! The adapter does not own an event loop, widgets, application state, or a clipboard
//! implementation. Hosts keep those responsibilities and inject the narrow services used here.

#![deny(unsafe_code)]

#[allow(unsafe_code)]
mod monitor;
pub use monitor::popup_monitor_bounds;

mod document_link;
mod ui_capture;
mod ui_context;
mod ui_input;
mod ui_input_command;
mod ui_input_observation;
mod ui_json;
mod ui_stdio;
mod ui_trace;
mod ui_wait;
pub use ui_capture::{UiCaptureGeometry, UiCaptureRequest};
pub use ui_context::{UiAutomationContext, UiContextLease, UiScopedResponse};
pub use ui_input::{UiInputEvent, UiInputEventSource, UiInputOrigin, UiKeyEvent, UiKeyEventSource};
pub use ui_input_command::{
    UiInputRequest, UiInputStep, UiWindowCommand, apply_ui_window_command, ui_input_schema,
    ui_input_state, ui_window_state,
};
pub use ui_input_observation::UiInputObservation;
pub use ui_json::{ui_changes_json, ui_snapshot_json};
pub use ui_stdio::{start_ui_stdio, start_ui_stdio_with_limit};
pub use ui_trace::{UiTraceError, UiTraceExport, UiTraceRecording};
mod focus_visibility;
mod ime_request;
mod text_pointer;
pub use document_link::{DocumentLink, DocumentLinkError, resolve_document_link};
#[cfg(not(target_arch = "wasm32"))]
pub use document_link::{WebLinkOpenError, open_web_link};
pub use focus_visibility::{is_user_key_press, observe_focus_visibility};
pub use text_pointer::TextPointerAction;
mod window_chrome;
mod window_text;
pub use window_chrome::window_resize_direction;
pub use window_text::{TextBridgeRegistrationError, WindowTextInputBridges};

pub use accesskit_winit::{
    Adapter as AccessibilityAdapter, Event as AccessibilityEvent,
    WindowEvent as AccessibilityWindowEvent,
};

use std::{error::Error, fmt};

use UiInputEvent as WindowEvent;
use vesta_gpui::{
    KeyChord, KeyModifiers, KeyStroke, Point, PointerButton, TextEditKind, TextInputAction,
    TextInputContext, TextInputError, TextInputState, TextInputViewport,
};
use winit::{
    dpi::PhysicalPosition,
    event::{ElementState, Ime, Modifiers, MouseButton, MouseScrollDelta},
    keyboard::{Key, KeyCode, ModifiersState, NamedKey, PhysicalKey},
    window::Window,
};

pub trait Clipboard {
    type Error;

    fn read_text(&mut self) -> Result<String, Self::Error>;
    fn write_text(&mut self, text: &str) -> Result<(), Self::Error>;
}

/// Normalize a winit logical key for a backend-neutral shortcut map.
/// The host calls this only after the current text input or popup declines the event.
pub fn shortcut_chord(key: Key<&str>, modifiers: ModifiersState) -> Option<KeyChord> {
    let modifiers = KeyModifiers {
        control: modifiers.control_key(),
        alt: modifiers.alt_key(),
        shift: modifiers.shift_key(),
        super_key: modifiers.super_key(),
    };
    let stroke = match key {
        Key::Character(text) => {
            let mut chars = text.chars();
            let character = chars.next()?;
            if chars.next().is_some() || character.is_control() {
                return None;
            }
            return Some(KeyChord::character(character, modifiers));
        }
        Key::Named(NamedKey::Escape) => KeyStroke::Escape,
        Key::Named(NamedKey::Enter) => KeyStroke::Enter,
        Key::Named(NamedKey::Space) => KeyStroke::Space,
        Key::Named(NamedKey::Tab) => KeyStroke::Tab,
        Key::Named(NamedKey::ArrowLeft) => KeyStroke::ArrowLeft,
        Key::Named(NamedKey::ArrowRight) => KeyStroke::ArrowRight,
        Key::Named(NamedKey::ArrowUp) => KeyStroke::ArrowUp,
        Key::Named(NamedKey::ArrowDown) => KeyStroke::ArrowDown,
        Key::Named(NamedKey::Home) => KeyStroke::Home,
        Key::Named(NamedKey::End) => KeyStroke::End,
        Key::Named(NamedKey::PageUp) => KeyStroke::PageUp,
        Key::Named(NamedKey::PageDown) => KeyStroke::PageDown,
        _ => return None,
    };
    Some(KeyChord::new(stroke, modifiers))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextInputResponse {
    Ignored,
    Handled { changed: bool },
    Submit,
    Cancel,
    TraverseFocus { backwards: bool },
}

impl TextInputResponse {
    #[must_use]
    pub const fn is_handled(self) -> bool {
        !matches!(self, Self::Ignored)
    }

    #[must_use]
    pub const fn changed(self) -> bool {
        matches!(self, Self::Handled { changed: true })
    }
}

#[derive(Debug)]
pub enum TextInputBridgeError<E> {
    Edit(TextInputError),
    Clipboard(E),
}

impl<E: fmt::Display> fmt::Display for TextInputBridgeError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Edit(error) => write!(formatter, "text edit failed: {error}"),
            Self::Clipboard(error) => write!(formatter, "clipboard access failed: {error}"),
        }
    }
}

impl<E: Error + 'static> Error for TextInputBridgeError<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Edit(error) => Some(error),
            Self::Clipboard(error) => Some(error),
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct TextInputBridge {
    modifiers: ModifiersState,
    alt_graph: bool,
    ime_enabled: bool,
    ime_request: ime_request::ImeRequestState,
    text_pointer: Option<text_pointer::TextPointerPress>,
}

impl TextInputBridge {
    #[must_use]
    pub const fn modifiers(&self) -> ModifiersState {
        self.modifiers
    }

    pub fn reset(&mut self) {
        self.modifiers = ModifiersState::empty();
        self.alt_graph = false;
        self.clear_ime_session();
    }

    /// End the current field's IME session without discarding keyboard modifiers.
    /// A later field accepts composition only after its own `Ime::Enabled` event.
    pub fn clear_ime_session(&mut self) {
        self.ime_enabled = false;
        self.ime_request.invalidate();
        self.text_pointer = None;
    }

    /// Publish this window's requested IME state only when it changes. Session end/reset,
    /// platform lifecycle, caret and DPI changes invalidate the relevant request state.
    /// Received Ime::Enabled is independent and never substitutes for requested allowed.
    pub fn sync_ime(&mut self, window: &Window, enabled: bool, view: Option<TextInputViewport>) {
        self.ime_request.sync(window, enabled, view);
    }

    pub fn handle_window_event<C: Clipboard>(
        &mut self,
        event: &impl UiInputEventSource,
        input: &mut TextInputState,
        context: &mut TextInputContext,
        clipboard: &mut C,
        read_only: bool,
    ) -> Result<TextInputResponse, TextInputBridgeError<C::Error>> {
        self.observe_text_pointer_event(event, true);
        let event = event.ui_input_event();
        match event.as_ref() {
            WindowEvent::ModifiersChanged(modifiers) => {
                self.update_modifiers(modifiers);
                Ok(TextInputResponse::Ignored)
            }
            WindowEvent::KeyboardInput {
                event,
                is_synthetic: false,
                ..
            } => self.handle_key_event(event, input, context, clipboard, read_only),
            WindowEvent::Ime(ime) => self.handle_ime(ime, input, context, read_only),
            WindowEvent::Text(text) => {
                self.text_pointer = None;
                let changed = if read_only || input.is_read_only() || input.is_composing() {
                    false
                } else {
                    input
                        .commit_ime(context, text)
                        .map_err(TextInputBridgeError::Edit)?
                };
                Ok(TextInputResponse::Handled { changed })
            }
            WindowEvent::Focused(false) => {
                let changed = input.is_composing();
                input.cancel_compose(context);
                input.end_group();
                self.reset();
                Ok(TextInputResponse::Handled { changed })
            }
            _ => Ok(TextInputResponse::Ignored),
        }
    }

    pub fn update_modifiers(&mut self, modifiers: &Modifiers) {
        self.modifiers = modifiers.state();
    }

    pub fn handle_ime<E>(
        &mut self,
        ime: &Ime,
        input: &mut TextInputState,
        context: &mut TextInputContext,
        read_only: bool,
    ) -> Result<TextInputResponse, TextInputBridgeError<E>> {
        if matches!(ime, Ime::Commit(_) | Ime::Disabled)
            || matches!(ime, Ime::Preedit(value, _) if !value.is_empty())
        {
            self.text_pointer = None;
        }
        let read_only = read_only || input.is_read_only();
        match ime {
            Ime::Enabled => {
                self.ime_request.invalidate_cursor();
                self.ime_enabled = true;
                return Ok(TextInputResponse::Ignored);
            }
            Ime::Disabled if !self.ime_enabled => {
                self.ime_request.invalidate();
                return Ok(TextInputResponse::Handled { changed: false });
            }
            Ime::Disabled => {
                self.ime_request.invalidate();
                self.ime_enabled = false;
            }
            Ime::Preedit(_, _) | Ime::Commit(_) if !self.ime_enabled => {
                return Ok(TextInputResponse::Handled { changed: false });
            }
            Ime::Preedit(_, _) | Ime::Commit(_) => {}
        }
        if read_only {
            return Ok(match ime {
                Ime::Preedit(_, _) | Ime::Commit(_) | Ime::Disabled => {
                    TextInputResponse::Handled { changed: false }
                }
                Ime::Enabled => unreachable!(),
            });
        }
        let changed = match ime {
            Ime::Preedit(value, cursor) => {
                input
                    .preedit(context, value, *cursor)
                    .map_err(TextInputBridgeError::Edit)?;
                true
            }
            Ime::Commit(value) => input
                .commit_ime(context, value)
                .map_err(TextInputBridgeError::Edit)?,
            Ime::Disabled => {
                let changed = input.is_composing();
                input
                    .preedit(context, "", None)
                    .map_err(TextInputBridgeError::Edit)?;
                changed
            }
            Ime::Enabled => unreachable!(),
        };
        Ok(TextInputResponse::Handled { changed })
    }

    pub fn handle_key_event<C: Clipboard>(
        &mut self,
        event: &impl UiKeyEventSource,
        input: &mut TextInputState,
        context: &mut TextInputContext,
        clipboard: &mut C,
        read_only: bool,
    ) -> Result<TextInputResponse, TextInputBridgeError<C::Error>> {
        let event = event.ui_key_event();
        self.handle_key_parts(
            event.logical_key.as_ref(),
            event.physical_key,
            event.text.as_deref(),
            event.state,
            input,
            context,
            clipboard,
            read_only,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn handle_key_parts<C: Clipboard>(
        &mut self,
        logical_key: Key<&str>,
        physical_key: PhysicalKey,
        text: Option<&str>,
        state: ElementState,
        input: &mut TextInputState,
        context: &mut TextInputContext,
        clipboard: &mut C,
        read_only: bool,
    ) -> Result<TextInputResponse, TextInputBridgeError<C::Error>> {
        let read_only = read_only || input.is_read_only();
        let alt_graph_key = logical_key == Key::Named(NamedKey::AltGraph)
            || physical_key == PhysicalKey::Code(KeyCode::AltRight);
        if alt_graph_key {
            self.alt_graph = state == ElementState::Pressed;
        }
        if state != ElementState::Pressed {
            return Ok(TextInputResponse::Ignored);
        }
        self.text_pointer = None;

        // While an IME owns the composition, text and navigation arrive through
        // Preedit/Commit. A concurrent key event must not insert duplicate text,
        // submit the field, or move focus away from the active composition.
        if input.is_composing() {
            if logical_key == Key::Named(NamedKey::Escape) {
                input.cancel_compose(context);
                return Ok(TextInputResponse::Handled { changed: true });
            }
            return Ok(TextInputResponse::Handled { changed: false });
        }

        let primary = self.primary_modifier() && !self.alt_graph;
        let shift = self.modifiers.shift_key();
        let action = match logical_key {
            Key::Named(NamedKey::ArrowLeft) => Some(TextInputAction::Left {
                extend: shift,
                word: primary,
            }),
            Key::Named(NamedKey::ArrowRight) => Some(TextInputAction::Right {
                extend: shift,
                word: primary,
            }),
            Key::Named(NamedKey::ArrowUp) if input.is_multiline() => {
                Some(TextInputAction::Up { extend: shift })
            }
            Key::Named(NamedKey::ArrowDown) if input.is_multiline() => {
                Some(TextInputAction::Down { extend: shift })
            }
            Key::Named(NamedKey::Home) => Some(if primary {
                TextInputAction::TextStart { extend: shift }
            } else {
                TextInputAction::Start { extend: shift }
            }),
            Key::Named(NamedKey::End) => Some(if primary {
                TextInputAction::TextEnd { extend: shift }
            } else {
                TextInputAction::End { extend: shift }
            }),
            Key::Named(NamedKey::Backspace) => Some(TextInputAction::Backspace { word: primary }),
            Key::Named(NamedKey::Delete) => Some(TextInputAction::Delete { word: primary }),
            _ => match (physical_key, primary) {
                (PhysicalKey::Code(KeyCode::KeyA), true) => Some(TextInputAction::SelectAll),
                (PhysicalKey::Code(KeyCode::KeyZ), true) => Some(if shift {
                    TextInputAction::Redo
                } else {
                    TextInputAction::Undo
                }),
                (PhysicalKey::Code(KeyCode::KeyY), true) => Some(TextInputAction::Redo),
                _ => None,
            },
        };
        if let Some(action) = action {
            let modifying = matches!(
                action,
                TextInputAction::Backspace { .. }
                    | TextInputAction::Delete { .. }
                    | TextInputAction::Undo
                    | TextInputAction::Redo
            );
            let changed = if read_only && modifying {
                false
            } else {
                input.act(context, action)
            };
            return Ok(TextInputResponse::Handled { changed });
        }

        match logical_key {
            Key::Named(NamedKey::Enter) => {
                if input.is_multiline() && !primary {
                    if read_only {
                        return Ok(TextInputResponse::Handled { changed: false });
                    }
                    let changed = input
                        .insert(context, "\n", TextEditKind::Typing)
                        .map_err(TextInputBridgeError::Edit)?;
                    return Ok(TextInputResponse::Handled { changed });
                }
                return Ok(TextInputResponse::Submit);
            }
            Key::Named(NamedKey::Escape) => return Ok(TextInputResponse::Cancel),
            Key::Named(NamedKey::Tab)
                if !primary && !self.modifiers.alt_key() && !self.alt_graph =>
            {
                return Ok(TextInputResponse::TraverseFocus { backwards: shift });
            }
            _ => {}
        }

        if primary
            && matches!(
                physical_key,
                PhysicalKey::Code(KeyCode::KeyC) | PhysicalKey::Code(KeyCode::KeyX)
            )
        {
            if input.is_password() {
                return Ok(TextInputResponse::Handled { changed: false });
            }
            let cut = physical_key == PhysicalKey::Code(KeyCode::KeyX);
            if let Some(selected) = input.selected_text().map(str::to_owned) {
                clipboard
                    .write_text(&selected)
                    .map_err(TextInputBridgeError::Clipboard)?;
                let changed = if cut && !read_only {
                    input
                        .insert(context, "", TextEditKind::Paste)
                        .map_err(TextInputBridgeError::Edit)?
                } else {
                    false
                };
                return Ok(TextInputResponse::Handled { changed });
            }
            return Ok(TextInputResponse::Handled { changed: false });
        }
        if primary && physical_key == PhysicalKey::Code(KeyCode::KeyV) {
            if read_only {
                return Ok(TextInputResponse::Handled { changed: false });
            }
            let pasted = clipboard
                .read_text()
                .map_err(TextInputBridgeError::Clipboard)?;
            let changed = input
                .insert(context, &pasted, TextEditKind::Paste)
                .map_err(TextInputBridgeError::Edit)?;
            return Ok(TextInputResponse::Handled { changed });
        }

        if primary || (self.modifiers.alt_key() && !self.alt_graph) || self.modifiers.super_key() {
            return Ok(TextInputResponse::Ignored);
        }
        let text = text.or(match logical_key {
            Key::Character(value) => Some(value),
            _ => None,
        });
        let Some(text) = text.filter(|value| !value.chars().any(char::is_control)) else {
            return Ok(TextInputResponse::Ignored);
        };
        if read_only {
            return Ok(TextInputResponse::Handled { changed: false });
        }
        let changed = input
            .insert(context, text, TextEditKind::Typing)
            .map_err(TextInputBridgeError::Edit)?;
        Ok(TextInputResponse::Handled { changed })
    }

    fn primary_modifier(self) -> bool {
        if cfg!(target_os = "macos") {
            self.modifiers.super_key()
        } else {
            self.modifiers.control_key()
        }
    }
}

pub fn sync_ime(window: &Window, enabled: bool, view: Option<TextInputViewport>) {
    window.set_ime_allowed(enabled);
    if !enabled {
        return;
    }
    if let Some(cursor) = ime_request::CursorArea::from_view(view, window.scale_factor()) {
        cursor.publish(window);
    }
}

#[must_use]
pub fn logical_point(position: PhysicalPosition<f64>, scale_factor: f64) -> Point {
    let scale = if scale_factor.is_finite() && scale_factor > 0.0 {
        scale_factor
    } else {
        1.0
    };
    Point::new((position.x / scale) as f32, (position.y / scale) as f32)
}

#[must_use]
pub const fn pointer_button(button: MouseButton) -> Option<PointerButton> {
    match button {
        MouseButton::Left => Some(PointerButton::Primary),
        MouseButton::Right => Some(PointerButton::Secondary),
        MouseButton::Middle => Some(PointerButton::Middle),
        MouseButton::Back | MouseButton::Forward | MouseButton::Other(_) => None,
    }
}

#[must_use]
pub fn wheel_delta(delta: MouseScrollDelta, scale_factor: f64, line_pixels: f32) -> Point {
    match delta {
        MouseScrollDelta::LineDelta(x, y) => Point::new(x * line_pixels, y * line_pixels),
        MouseScrollDelta::PixelDelta(position) => {
            let point = logical_point(position, scale_factor);
            Point::new(point.x, point.y)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct MemoryClipboard(String);

    impl Clipboard for MemoryClipboard {
        type Error = std::convert::Infallible;

        fn read_text(&mut self) -> Result<String, Self::Error> {
            Ok(self.0.clone())
        }

        fn write_text(&mut self, text: &str) -> Result<(), Self::Error> {
            self.0 = text.to_owned();
            Ok(())
        }
    }

    fn press(text: &str) -> (Key<&str>, PhysicalKey, Option<&str>) {
        (
            Key::Character(text),
            PhysicalKey::Code(KeyCode::KeyH),
            Some(text),
        )
    }

    #[test]
    fn shortcut_projection_preserves_modifiers_and_ignores_ime_text() {
        assert_eq!(
            shortcut_chord(Key::Character("T"), ModifiersState::SHIFT),
            Some(KeyChord::character(
                't',
                KeyModifiers {
                    shift: true,
                    ..KeyModifiers::default()
                }
            ))
        );
        assert_eq!(
            shortcut_chord(Key::Character("ni"), ModifiersState::empty()),
            None
        );
    }

    #[test]
    fn key_text_and_ime_commit_share_the_core_editor() {
        let mut bridge = TextInputBridge::default();
        let mut input = TextInputState::default();
        let mut context = TextInputContext::default();
        let mut clipboard = MemoryClipboard::default();
        let (logical, physical, text) = press("hello");
        let typed = bridge
            .handle_key_parts(
                logical,
                physical,
                text,
                ElementState::Pressed,
                &mut input,
                &mut context,
                &mut clipboard,
                false,
            )
            .expect("typing succeeds");
        assert_eq!(typed, TextInputResponse::Handled { changed: true });
        bridge
            .handle_ime::<std::convert::Infallible>(&Ime::Enabled, &mut input, &mut context, false)
            .expect("IME starts");
        bridge
            .handle_ime::<std::convert::Infallible>(
                &Ime::Preedit("世".to_owned(), Some((3, 3))),
                &mut input,
                &mut context,
                false,
            )
            .expect("preedit succeeds");
        bridge
            .handle_ime::<std::convert::Infallible>(
                &Ime::Commit("世界".to_owned()),
                &mut input,
                &mut context,
                false,
            )
            .expect("commit succeeds");
        assert_eq!(input.text(), "hello世界");

        bridge.clear_ime_session();
        let mut next_field = TextInputState::default();
        assert_eq!(
            bridge
                .handle_ime::<std::convert::Infallible>(
                    &Ime::Commit("旧字段".to_owned()),
                    &mut next_field,
                    &mut context,
                    false,
                )
                .expect("stale commit is ignored"),
            TextInputResponse::Handled { changed: false }
        );
        assert_eq!(next_field.text(), "");
        bridge
            .handle_ime::<std::convert::Infallible>(
                &Ime::Enabled,
                &mut next_field,
                &mut context,
                false,
            )
            .expect("next field IME starts");
        bridge
            .handle_ime::<std::convert::Infallible>(
                &Ime::Commit("新字段".to_owned()),
                &mut next_field,
                &mut context,
                false,
            )
            .expect("next field commit succeeds");
        assert_eq!(next_field.text(), "新字段");
        bridge.clear_ime_session();
        let mut command_field = TextInputState::default();
        bridge
            .handle_window_event(
                &WindowEvent::Text("命令🙂".into()),
                &mut command_field,
                &mut context,
                &mut clipboard,
                false,
            )
            .expect("logical text does not need an OS IME session");
        assert_eq!(command_field.text(), "命令🙂");
        bridge
            .handle_ime::<std::convert::Infallible>(
                &Ime::Commit("stale native".into()),
                &mut command_field,
                &mut context,
                false,
            )
            .expect("native gate remains closed");
        assert_eq!(command_field.text(), "命令🙂");
    }

    #[test]
    fn composing_ime_owns_keys_until_commit_or_cancel() {
        let mut bridge = TextInputBridge::default();
        let mut input = TextInputState::default();
        let mut context = TextInputContext::default();
        let mut clipboard = MemoryClipboard::default();
        bridge
            .handle_ime::<std::convert::Infallible>(&Ime::Enabled, &mut input, &mut context, false)
            .expect("IME starts");
        bridge
            .handle_ime::<std::convert::Infallible>(
                &Ime::Preedit("中".to_owned(), None),
                &mut input,
                &mut context,
                false,
            )
            .expect("preedit succeeds");

        for (key, physical, text) in [
            (
                Key::Character("z"),
                PhysicalKey::Code(KeyCode::KeyZ),
                Some("z"),
            ),
            (
                Key::Named(NamedKey::Enter),
                PhysicalKey::Code(KeyCode::Enter),
                None,
            ),
            (
                Key::Named(NamedKey::Tab),
                PhysicalKey::Code(KeyCode::Tab),
                None,
            ),
        ] {
            let response = bridge
                .handle_key_parts(
                    key,
                    physical,
                    text,
                    ElementState::Pressed,
                    &mut input,
                    &mut context,
                    &mut clipboard,
                    false,
                )
                .expect("composing key is absorbed");
            assert_eq!(response, TextInputResponse::Handled { changed: false });
        }
        assert!(input.is_composing());
        bridge
            .handle_ime::<std::convert::Infallible>(
                &Ime::Commit("中".to_owned()),
                &mut input,
                &mut context,
                false,
            )
            .expect("commit succeeds");
        assert_eq!(input.text(), "中");

        bridge
            .handle_ime::<std::convert::Infallible>(
                &Ime::Preedit("文".to_owned(), None),
                &mut input,
                &mut context,
                false,
            )
            .expect("second preedit succeeds");
        assert_eq!(
            bridge
                .handle_key_parts(
                    Key::Named(NamedKey::Escape),
                    PhysicalKey::Code(KeyCode::Escape),
                    None,
                    ElementState::Pressed,
                    &mut input,
                    &mut context,
                    &mut clipboard,
                    false,
                )
                .expect("escape cancels composition"),
            TextInputResponse::Handled { changed: true }
        );
        assert!(!input.is_composing());
        assert_eq!(input.text(), "中");
    }

    #[test]
    fn primary_paste_uses_injected_clipboard_without_owning_it() {
        let mut bridge = TextInputBridge {
            modifiers: if cfg!(target_os = "macos") {
                ModifiersState::SUPER
            } else {
                ModifiersState::CONTROL
            },
            alt_graph: false,
            ..TextInputBridge::default()
        };
        // A window transfer must not turn another window's modifier/IME state
        // into the current bridge or overwrite a live owner on duplicate registration.
        let first = vesta_gpui::UiWindowId(1);
        let second = vesta_gpui::UiWindowId(2);
        let mut windows = WindowTextInputBridges::default();
        windows.register(first).expect("main owner");
        windows.register(second).expect("floating owner");
        *windows.get_mut(first).expect("main bridge") = bridge;
        windows.get_mut(first).expect("main bridge").ime_enabled = true;
        windows.get_mut(second).expect("floating bridge").reset();
        assert_eq!(
            windows.get(first).expect("main bridge").modifiers(),
            bridge.modifiers()
        );
        assert!(windows.get(first).expect("main bridge").ime_enabled);
        assert_eq!(
            windows.register(first),
            Err(TextBridgeRegistrationError::DuplicateWindow(first))
        );
        assert!(
            windows
                .get(first)
                .expect("duplicate preserves active IME")
                .ime_enabled
        );
        assert!(windows.remove(first));
        assert!(
            windows.get_mut(first).is_none(),
            "late events cannot recreate a closed owner"
        );
        windows.register(first).expect("fresh owner after teardown");
        assert!(!windows.get(first).expect("fresh bridge").ime_enabled);
        assert!(
            windows
                .get(first)
                .expect("fresh bridge")
                .modifiers()
                .is_empty()
        );
        let mut input = TextInputState::default();
        let mut context = TextInputContext::default();
        let font = include_bytes!("../../vesta-gpui/tests/fixtures/MaterialIcons-Regular.ttf");
        assert!(context.register_font(std::sync::Arc::new(font.to_vec())));
        let mut clipboard = MemoryClipboard("pasted text".to_owned());

        bridge
            .handle_key_parts(
                Key::Character("v"),
                PhysicalKey::Code(KeyCode::KeyV),
                Some("v"),
                ElementState::Pressed,
                &mut input,
                &mut context,
                &mut clipboard,
                false,
            )
            .expect("paste succeeds");
        assert_eq!(input.text(), "pasted text");
        for (key, code, expected) in [
            ("a", KeyCode::KeyA, "pasted text"),
            ("c", KeyCode::KeyC, "pasted text"),
            ("x", KeyCode::KeyX, ""),
            ("z", KeyCode::KeyZ, "pasted text"),
            ("y", KeyCode::KeyY, ""),
            ("v", KeyCode::KeyV, "pasted text"),
        ] {
            bridge
                .handle_key_parts(
                    Key::Character(key),
                    PhysicalKey::Code(code),
                    Some(key),
                    ElementState::Pressed,
                    &mut input,
                    &mut context,
                    &mut clipboard,
                    false,
                )
                .expect("clipboard or history shortcut succeeds");
            assert_eq!(input.text(), expected, "after Ctrl+{key}");
            if key == "a" {
                assert_eq!(input.selected_text(), Some("pasted text"));
            }
            assert_eq!(clipboard.0, "pasted text");
        }
        let mut input = TextInputState::new_password(13.0, "Material Icons", 256);
        input
            .reset(&mut context, "private-value")
            .expect("password");
        clipboard.0 = "Clipboard 中文 😀".to_owned();
        for (key, code, expected) in [
            ("a", KeyCode::KeyA, "private-value"),
            ("c", KeyCode::KeyC, "private-value"),
            ("x", KeyCode::KeyX, "private-value"),
            ("v", KeyCode::KeyV, "Clipboard 中文 😀"),
            ("z", KeyCode::KeyZ, "private-value"),
            ("y", KeyCode::KeyY, "Clipboard 中文 😀"),
        ] {
            bridge
                .handle_key_parts(
                    Key::Character(key),
                    PhysicalKey::Code(code),
                    Some(key),
                    ElementState::Pressed,
                    &mut input,
                    &mut context,
                    &mut clipboard,
                    false,
                )
                .expect("password shortcut");
            assert_eq!(input.text(), expected, "password Ctrl+{key}");
            assert_eq!(
                clipboard.0, "Clipboard 中文 😀",
                "password cannot write the clipboard"
            );
        }

        // A caller passing false must not make an owner-created selectable label
        // writable. Clipboard stays injected and Paste must not even read it.
        struct CopyOnlyClipboard {
            text: String,
            writes: usize,
        }
        impl Clipboard for CopyOnlyClipboard {
            type Error = &'static str;
            fn read_text(&mut self) -> Result<String, Self::Error> {
                Err("read-only paste read clipboard")
            }
            fn write_text(&mut self, text: &str) -> Result<(), Self::Error> {
                self.text = text.into();
                self.writes += 1;
                Ok(())
            }
        }
        let source = "e\u{301}👨‍👩‍👧‍👦\t中文\r\nمرحبا";
        let mut input = TextInputState::new_read_only(13.0, "Material Icons", Some(140.0));
        input.reset(&mut context, source).expect("label source");
        let mut copied = CopyOnlyClipboard {
            text: String::new(),
            writes: 0,
        };
        for (key, code) in [
            ("a", KeyCode::KeyA),
            ("c", KeyCode::KeyC),
            ("x", KeyCode::KeyX),
            ("v", KeyCode::KeyV),
            ("z", KeyCode::KeyZ),
            ("y", KeyCode::KeyY),
        ] {
            bridge
                .handle_key_parts(
                    Key::Character(key),
                    PhysicalKey::Code(code),
                    Some(key),
                    ElementState::Pressed,
                    &mut input,
                    &mut context,
                    &mut copied,
                    false,
                )
                .expect("read-only shortcut");
            assert_eq!(input.text(), source);
        }
        assert_eq!(copied.text, source);
        assert_eq!(
            copied.writes, 2,
            "Copy and existing read-only Cut copy without deleting"
        );
        bridge.modifiers = ModifiersState::empty();
        bridge
            .handle_key_parts(
                Key::Named(NamedKey::Delete),
                PhysicalKey::Code(KeyCode::Delete),
                None,
                ElementState::Pressed,
                &mut input,
                &mut context,
                &mut copied,
                false,
            )
            .expect("delete consumed");
        bridge
            .handle_key_parts(
                Key::Character("x"),
                PhysicalKey::Code(KeyCode::KeyX),
                Some("x"),
                ElementState::Pressed,
                &mut input,
                &mut context,
                &mut copied,
                false,
            )
            .expect("typing consumed");
        bridge
            .handle_ime::<&str>(&Ime::Enabled, &mut input, &mut context, false)
            .expect("platform lifecycle");
        for ime in [
            Ime::Preedit("候选".into(), None),
            Ime::Commit("提交".into()),
            Ime::Disabled,
        ] {
            assert_eq!(
                bridge
                    .handle_ime::<&str>(&ime, &mut input, &mut context, false)
                    .expect("read-only IME"),
                TextInputResponse::Handled { changed: false }
            );
        }
        assert_eq!(input.text(), source);
        assert_eq!(input.selected_text(), Some(source));
        assert!(!input.is_composing());
    }
}
