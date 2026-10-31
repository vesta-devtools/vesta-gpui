//! Immutable, window-owned geometry for caller-defined text field identities.

use crate::{ElementId, InputTarget, TextInputViewport, UiWindowId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextInputViewError {
    DuplicateElement(ElementId),
    DuplicateField { first: ElementId, second: ElementId },
}

impl std::fmt::Display for TextInputViewError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateElement(element) => {
                write!(formatter, "duplicate text input element {}", element.get())
            }
            Self::DuplicateField { first, second } => write!(
                formatter,
                "text input field is mapped by both {} and {}",
                first.get(),
                second.get()
            ),
        }
    }
}
impl std::error::Error for TextInputViewError {}

/// Rebuilt from one window's current immutable text layout geometry. A host may
/// project paint snapshots before submission or use completed renderer output;
/// it must use the same layout for its input/hit maps. Content, drafts, validation
/// and platform IME objects remain host-owned; absent fields have no stale geometry.
#[derive(Clone, Debug)]
pub struct TextInputViews<K> {
    window: Option<UiWindowId>,
    entries: Vec<(K, ElementId, TextInputViewport)>,
}

impl<K> Default for TextInputViews<K> {
    fn default() -> Self {
        Self {
            window: None,
            entries: Vec::new(),
        }
    }
}

impl<K: Copy + Eq> TextInputViews<K> {
    pub fn from_layout(
        window: UiWindowId,
        geometry: &[(ElementId, TextInputViewport)],
        mut identify: impl FnMut(ElementId) -> Option<K>,
    ) -> Result<Self, TextInputViewError> {
        let mut entries: Vec<(K, ElementId, TextInputViewport)> = Vec::new();
        for &(element, viewport) in geometry {
            let Some(field) = identify(element) else {
                continue;
            };
            if entries.iter().any(|(_, existing, _)| *existing == element) {
                return Err(TextInputViewError::DuplicateElement(element));
            }
            if let Some((_, first, _)) = entries.iter().find(|(key, _, _)| *key == field) {
                return Err(TextInputViewError::DuplicateField {
                    first: *first,
                    second: element,
                });
            }
            entries.push((field, element, viewport));
        }
        Ok(Self {
            window: Some(window),
            entries,
        })
    }

    #[must_use]
    pub fn get(&self, field: K) -> Option<TextInputViewport> {
        self.entries
            .iter()
            .find_map(|(key, _, view)| (*key == field).then_some(*view))
    }

    /// Reject another window, even if its field or element identity happens to match.
    #[must_use]
    pub fn field_at(&self, target: InputTarget) -> Option<K> {
        (self.window == Some(target.window))
            .then(|| {
                self.entries
                    .iter()
                    .find_map(|(field, element, _)| (*element == target.element).then_some(*field))
            })
            .flatten()
    }
}
