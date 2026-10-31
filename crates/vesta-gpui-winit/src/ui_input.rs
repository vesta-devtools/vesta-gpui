//! Owned logical input shared by native adapters and explicit command dispatch.
//! This is application input, never an operating-system event injector.
use std::{borrow::Cow, path::PathBuf};
use winit::{
    dpi::{PhysicalPosition, PhysicalSize},
    event::{
        DeviceId, ElementState, Ime, KeyEvent, Modifiers, MouseButton, MouseScrollDelta, Touch,
        TouchPhase, WindowEvent,
    },
    keyboard::{Key, KeyLocation, PhysicalKey},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiInputOrigin {
    Native,
    Command,
}

/// The public portion of a platform key. No platform-private fields or unsafe constructors.
#[derive(Clone, Debug)]
pub struct UiKeyEvent {
    pub physical_key: PhysicalKey,
    pub logical_key: Key,
    pub text: Option<String>,
    pub location: KeyLocation,
    pub state: ElementState,
    pub repeat: bool,
}
impl From<&KeyEvent> for UiKeyEvent {
    fn from(key: &KeyEvent) -> Self {
        Self {
            physical_key: key.physical_key,
            logical_key: key.logical_key.clone(),
            text: key.text.as_ref().map(ToString::to_string),
            location: key.location,
            state: key.state,
            repeat: key.repeat,
        }
    }
}

/// Host-visible input/lifecycle data. Unhandled platform notifications remain ignored.
#[derive(Clone, Debug)]
pub enum UiInputEvent {
    KeyboardInput {
        device_id: DeviceId,
        event: UiKeyEvent,
        is_synthetic: bool,
    },
    CursorMoved {
        device_id: DeviceId,
        position: PhysicalPosition<f64>,
    },
    CursorLeft {
        device_id: DeviceId,
    },
    MouseInput {
        device_id: DeviceId,
        state: ElementState,
        button: MouseButton,
    },
    MouseWheel {
        device_id: DeviceId,
        delta: MouseScrollDelta,
        phase: TouchPhase,
    },
    ModifiersChanged(Modifiers),
    Ime(Ime),
    /// Committed logical text from the bounded command API, independent of OS IME.
    Text(String),
    Touch(Touch),
    Focused(bool),
    Resized(PhysicalSize<u32>),
    ScaleFactorChanged {
        scale_factor: f64,
    },
    Moved(PhysicalPosition<i32>),
    Occluded(bool),
    DroppedFile(PathBuf),
    CloseRequested,
    Destroyed,
    RedrawRequested,
    Other,
}
impl From<&WindowEvent> for UiInputEvent {
    fn from(event: &WindowEvent) -> Self {
        match event {
            WindowEvent::KeyboardInput {
                device_id,
                event,
                is_synthetic,
            } => Self::KeyboardInput {
                device_id: *device_id,
                event: event.into(),
                is_synthetic: *is_synthetic,
            },
            WindowEvent::CursorMoved {
                device_id,
                position,
            } => Self::CursorMoved {
                device_id: *device_id,
                position: *position,
            },
            WindowEvent::CursorLeft { device_id } => Self::CursorLeft {
                device_id: *device_id,
            },
            WindowEvent::MouseInput {
                device_id,
                state,
                button,
            } => Self::MouseInput {
                device_id: *device_id,
                state: *state,
                button: *button,
            },
            WindowEvent::MouseWheel {
                device_id,
                delta,
                phase,
            } => Self::MouseWheel {
                device_id: *device_id,
                delta: *delta,
                phase: *phase,
            },
            WindowEvent::ModifiersChanged(value) => Self::ModifiersChanged(*value),
            WindowEvent::Ime(value) => Self::Ime(value.clone()),
            WindowEvent::Touch(value) => Self::Touch(*value),
            WindowEvent::Focused(value) => Self::Focused(*value),
            WindowEvent::Resized(value) => Self::Resized(*value),
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => Self::ScaleFactorChanged {
                scale_factor: *scale_factor,
            },
            WindowEvent::Moved(value) => Self::Moved(*value),
            WindowEvent::Occluded(value) => Self::Occluded(*value),
            WindowEvent::DroppedFile(value) => Self::DroppedFile(value.clone()),
            WindowEvent::CloseRequested => Self::CloseRequested,
            WindowEvent::Destroyed => Self::Destroyed,
            WindowEvent::RedrawRequested => Self::RedrawRequested,
            _ => Self::Other,
        }
    }
}

impl UiInputEvent {
    /// Hint for invalidating ordinary input/geometry observations. A false value
    /// never permits skipping lifecycle dispatch: Hosts still reconcile visibility,
    /// capture and renderer availability when those policies change.
    pub fn changes_ui(&self) -> bool {
        !matches!(
            self,
            Self::RedrawRequested | Self::Other | Self::Occluded(_)
        )
    }
}

/// Native callers retain their existing API; normalized callers borrow without cloning.
pub trait UiInputEventSource {
    fn ui_input_event(&self) -> Cow<'_, UiInputEvent>;
}
impl UiInputEventSource for WindowEvent {
    fn ui_input_event(&self) -> Cow<'_, UiInputEvent> {
        Cow::Owned(self.into())
    }
}
impl UiInputEventSource for UiInputEvent {
    fn ui_input_event(&self) -> Cow<'_, UiInputEvent> {
        Cow::Borrowed(self)
    }
}
pub trait UiKeyEventSource {
    fn ui_key_event(&self) -> Cow<'_, UiKeyEvent>;
}
impl UiKeyEventSource for KeyEvent {
    fn ui_key_event(&self) -> Cow<'_, UiKeyEvent> {
        Cow::Owned(self.into())
    }
}
impl UiKeyEventSource for UiKeyEvent {
    fn ui_key_event(&self) -> Cow<'_, UiKeyEvent> {
        Cow::Borrowed(self)
    }
}
