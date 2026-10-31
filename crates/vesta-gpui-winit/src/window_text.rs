//! Registered window ownership for platform modifier and IME bridge state.

use crate::TextInputBridge;
use std::collections::{HashMap, hash_map::Entry};
use vesta_gpui::UiWindowId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextBridgeRegistrationError {
    DuplicateWindow(UiWindowId),
}

impl std::fmt::Display for TextBridgeRegistrationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateWindow(id) => {
                write!(f, "text input window is already registered: {id:?}")
            }
        }
    }
}
impl std::error::Error for TextBridgeRegistrationError {}

/// A host registers a window before dispatch and removes it after ending its
/// author-independent text session. Unknown/closed windows never create state
/// implicitly; registering an existing identity cannot overwrite active IME.
#[derive(Default)]
pub struct WindowTextInputBridges {
    windows: HashMap<UiWindowId, TextInputBridge>,
}

impl WindowTextInputBridges {
    pub fn register(&mut self, window: UiWindowId) -> Result<(), TextBridgeRegistrationError> {
        match self.windows.entry(window) {
            Entry::Vacant(entry) => {
                entry.insert(TextInputBridge::default());
                Ok(())
            }
            Entry::Occupied(_) => Err(TextBridgeRegistrationError::DuplicateWindow(window)),
        }
    }

    #[must_use]
    pub fn get(&self, window: UiWindowId) -> Option<&TextInputBridge> {
        self.windows.get(&window)
    }

    pub fn get_mut(&mut self, window: UiWindowId) -> Option<&mut TextInputBridge> {
        self.windows.get_mut(&window)
    }

    pub fn remove(&mut self, window: UiWindowId) -> bool {
        self.windows.remove(&window).is_some()
    }
}
