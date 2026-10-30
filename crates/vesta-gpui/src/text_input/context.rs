//! Host-supplied fonts and bounded shaping caches, independent of editor history.
use super::{
    EllipsizedText,
    static_text::{StaticTextBaseKey, StaticTextCache, StaticTextWrappedKey},
};
use crate::FontRole;
use parley::{FontContext, LayoutContext};
use std::{collections::HashMap, sync::Arc};

/// One font/measurement resource owner. It never discovers fonts or performs I/O.
pub struct TextInputContext {
    measurement_source: Arc<()>,
    pub(super) fonts: FontContext,
    pub(super) layout: LayoutContext<()>,
    pub(super) font_revision: u64,
    pub(super) font_stacks: HashMap<FontRole, String>,
    pub(super) static_text_bases: StaticTextCache<StaticTextBaseKey>,
    pub(super) static_text_wrapped: StaticTextCache<StaticTextWrappedKey>,
    pub(super) static_text_ellipsized: StaticTextCache<StaticTextWrappedKey, EllipsizedText>,
}

/// Opaque in-memory validity token for measurements from a text context.
/// It is not a persistent identity or a serialized document revision.
#[derive(Clone, Debug)]
pub struct TextMeasurementRevision {
    source: Arc<()>,
    font_revision: u64,
}
impl PartialEq for TextMeasurementRevision {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.source, &other.source) && self.font_revision == other.font_revision
    }
}
impl Eq for TextMeasurementRevision {}

impl Default for TextInputContext {
    fn default() -> Self {
        Self {
            measurement_source: Arc::new(()),
            fonts: FontContext {
                collection: parley::fontique::Collection::new(
                    parley::fontique::CollectionOptions {
                        shared: false,
                        system_fonts: false,
                    },
                ),
                source_cache: Default::default(),
            },
            layout: LayoutContext::new(),
            font_revision: 0,
            font_stacks: HashMap::new(),
            static_text_bases: StaticTextCache::default(),
            static_text_wrapped: StaticTextCache::default(),
            static_text_ellipsized: StaticTextCache::default(),
        }
    }
}

impl TextInputContext {
    /// Changes after successful font registration, a changed role stack, or
    /// replacement by another context. No-op/failed font updates preserve it.
    pub fn measurement_revision(&self) -> TextMeasurementRevision {
        TextMeasurementRevision {
            source: Arc::clone(&self.measurement_source),
            font_revision: self.font_revision,
        }
    }

    fn invalidate_fonts(&mut self) {
        self.font_revision = self.font_revision.wrapping_add(1);
        self.static_text_bases.clear();
        self.static_text_wrapped.clear();
        self.static_text_ellipsized.clear();
    }

    /// Register host-provided bytes; this API never searches the filesystem.
    pub fn register_font(&mut self, bytes: Arc<Vec<u8>>) -> bool {
        let families = self
            .fonts
            .collection
            .register_fonts(parley::fontique::Blob::new(bytes), None);
        if families.is_empty() {
            return false;
        }
        self.fonts.collection.append_generic_families(
            parley::fontique::GenericFamily::SansSerif,
            families.iter().map(|(id, _)| *id),
        );
        self.invalidate_fonts();
        true
    }

    /// Choose a host-registered family for a semantic static-text role.
    pub fn set_font_stack(&mut self, role: FontRole, stack: impl Into<String>) {
        let stack = stack.into();
        if stack.is_empty() || self.font_stacks.get(&role) == Some(&stack) {
            return;
        }
        self.font_stacks.insert(role, stack);
        self.invalidate_fonts();
    }
}
