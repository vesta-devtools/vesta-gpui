use std::collections::HashMap;

/// A platform-independent key usable by the host's shortcut map.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeyStroke {
    Character(char),
    Escape,
    Enter,
    Space,
    Tab,
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    ArrowDown,
    Home,
    End,
    PageUp,
    PageDown,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct KeyModifiers {
    pub control: bool,
    pub alt: bool,
    pub shift: bool,
    pub super_key: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct KeyChord {
    pub key: KeyStroke,
    pub modifiers: KeyModifiers,
}

impl KeyChord {
    #[must_use]
    pub const fn new(key: KeyStroke, modifiers: KeyModifiers) -> Self {
        Self { key, modifiers }
    }

    #[must_use]
    pub fn character(key: char, modifiers: KeyModifiers) -> Self {
        Self::new(KeyStroke::Character(key.to_ascii_lowercase()), modifiers)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeymapError {
    Duplicate(KeyChord),
}

impl std::fmt::Display for KeymapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Duplicate(chord) => write!(f, "duplicate key binding {chord:?}"),
        }
    }
}
impl std::error::Error for KeymapError {}

/// Typed shortcut registry. The host decides when text editing or a popup owns a key.
pub struct Keymap<Action> {
    bindings: HashMap<KeyChord, Action>,
}

impl<Action> Default for Keymap<Action> {
    fn default() -> Self {
        Self {
            bindings: HashMap::new(),
        }
    }
}

impl<Action> Keymap<Action> {
    pub fn bind(&mut self, chord: KeyChord, action: Action) -> Result<(), KeymapError> {
        if self.bindings.contains_key(&chord) {
            return Err(KeymapError::Duplicate(chord));
        }
        self.bindings.insert(chord, action);
        Ok(())
    }

    #[must_use]
    pub fn resolve(&self, chord: KeyChord) -> Option<&Action> {
        self.bindings.get(&chord)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcut_lookup_keeps_modifiers_and_rejects_ambiguous_bindings() {
        let mut keymap = Keymap::default();
        let plain = KeyChord::character('T', KeyModifiers::default());
        let shifted = KeyChord::character(
            't',
            KeyModifiers {
                shift: true,
                ..KeyModifiers::default()
            },
        );
        keymap.bind(plain, 1).expect("plain binding");
        keymap.bind(shifted, 2).expect("shift binding");
        assert_eq!(keymap.resolve(plain), Some(&1));
        assert_eq!(keymap.resolve(shifted), Some(&2));
        assert_eq!(keymap.bind(plain, 3), Err(KeymapError::Duplicate(plain)));
        assert_eq!(keymap.resolve(plain), Some(&1));
    }
}
