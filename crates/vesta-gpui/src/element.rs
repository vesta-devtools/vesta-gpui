use std::{
    collections::HashSet,
    sync::atomic::{AtomicU32, Ordering},
};

use crate::{Point, Semantic, Style};

/// Stable identity used to correlate retained state with an ephemeral element tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ElementId(u64);

const SCOPED_ELEMENT_FLAG: u64 = 1_u64 << 63;
const MAX_SCOPE: u32 = 0x7fff_ffff;
static NEXT_SCOPE: AtomicU32 = AtomicU32::new(1);

impl ElementId {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Allocates process-wide non-overlapping element namespaces for retained UI instances.
/// Separate allocator handles share the same atomic namespace, so independently constructed
/// features cannot accidentally restart allocation at scope one.
#[derive(Debug, Default)]
pub struct ElementIdAllocator;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ElementScopeExhausted;

impl ElementIdAllocator {
    pub fn allocate_scope(&self) -> Result<ElementScope, ElementScopeExhausted> {
        NEXT_SCOPE
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                (current <= MAX_SCOPE).then_some(current + 1)
            })
            .map(ElementScope)
            .map_err(|_| ElementScopeExhausted)
    }
}

/// A retained instance namespace. Local IDs only need to be unique inside this scope.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ElementScope(u32);

impl ElementScope {
    #[must_use]
    pub const fn element(self, local: u32) -> ElementId {
        ElementId(SCOPED_ELEMENT_FLAG | ((self.0 as u64) << 32) | local as u64)
    }

    #[must_use]
    pub fn identity(self, local: u32, local_key: impl Into<String>) -> ElementIdentity {
        ElementIdentity {
            id: self.element(local),
            key: ElementKey::new(format!("scope.{}/{}", self.0, local_key.into())),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ElementIdentity {
    pub id: ElementId,
    pub key: ElementKey,
}

/// An explicit stable key. It is never derived from display text or child position.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ElementKey(String);

impl ElementKey {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Semantic content of an element.
#[derive(Clone, Debug, PartialEq)]
pub enum ElementKind {
    Container,
    Text(String),
    /// Backend-neutral image source token. The host resolves and validates it.
    Image(String),
    /// Explicit image presentation; source pixels are still resolved by the host.
    FittedImage {
        source: String,
        presentation: crate::ImagePresentation,
    },
    /// Immutable text-editor projection. The editable state is retained by its owner.
    TextInput {
        snapshot: crate::TextInputSnapshot,
        focused: bool,
    },
    /// 滚动容器：内容在容器内裁剪，并按 `offset` 平移（像素）。
    /// offset 是宿主持久化的滚动位置（≥ 0），元素树每帧重建时由宿主回填。
    ScrollBox(Point),
}

/// Source policy for a text element. Legacy widgets without an explicit source
/// may be adapted by the host while they migrate to keyed copy.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum TextSource {
    #[default]
    Unspecified,
    Literal,
    Localized {
        key: String,
    },
}

/// A node in the ephemeral declarative element tree.
#[derive(Clone, Debug, PartialEq)]
pub struct Element {
    pub id: ElementId,
    pub key: ElementKey,
    pub kind: ElementKind,
    pub text_source: TextSource,
    pub style: Style,
    pub children: Vec<Self>,
    /// 基建级悬停提示：元素声明自己的 tooltip 文本，宿主在命中元素后直接读取，
    /// 无需集中映射。不参与布局与命中，仅作为可访问的元数据。
    pub tooltip: Option<String>,
    /// Backend-neutral accessibility description. It is independent of paint primitives.
    pub semantic: Option<Semantic>,
}

impl Element {
    #[must_use]
    pub fn container(id: ElementId, key: ElementKey) -> Self {
        Self {
            id,
            key,
            kind: ElementKind::Container,
            text_source: TextSource::Unspecified,
            style: Style::default(),
            children: Vec::new(),
            tooltip: None,
            semantic: None,
        }
    }

    #[must_use]
    pub fn text(id: ElementId, key: ElementKey, text: impl Into<String>) -> Self {
        Self {
            id,
            key,
            kind: ElementKind::Text(text.into()),
            text_source: TextSource::Unspecified,
            style: Style::default(),
            children: Vec::new(),
            tooltip: None,
            semantic: None,
        }
    }

    #[must_use]
    pub fn image(id: ElementId, key: ElementKey, source: impl Into<String>) -> Self {
        Self {
            id,
            key,
            kind: ElementKind::Image(source.into()),
            text_source: TextSource::Unspecified,
            style: Style::default(),
            children: Vec::new(),
            tooltip: None,
            semantic: None,
        }
    }

    #[must_use]
    pub fn fitted_image(
        id: ElementId,
        key: ElementKey,
        source: impl Into<String>,
        presentation: crate::ImagePresentation,
    ) -> Self {
        let mut element = Self::image(id, key, "");
        element.kind = ElementKind::FittedImage {
            source: source.into(),
            presentation,
        };
        element
    }

    #[must_use]
    pub fn styled(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    #[must_use]
    pub fn text_source(mut self, source: TextSource) -> Self {
        self.text_source = source;
        self
    }

    #[must_use]
    pub fn text_input(
        id: ElementId,
        key: ElementKey,
        snapshot: crate::TextInputSnapshot,
        focused: bool,
    ) -> Self {
        let mut element = Self::container(id, key);
        element.kind = ElementKind::TextInput { snapshot, focused };
        element
    }

    /// 声明提示文本；宿主可在悬停或焦点路径显示，并投影为无障碍说明。
    #[must_use]
    pub fn tooltip(mut self, text: impl Into<String>) -> Self {
        self.tooltip = Some(text.into());
        self
    }

    #[must_use]
    pub fn semantic(mut self, semantic: Semantic) -> Self {
        self.semantic = Some(semantic);
        self
    }

    /// 创建一个滚动容器：内容在容器内裁剪，并按 `offset` 平移。
    /// 内容子元素应保持自然高度（`flex_shrink(0)`），超出部分被裁剪、
    /// 可由宿主通过滚轮改变 `offset` 查看。
    #[must_use]
    pub fn scroll_box(id: ElementId, key: ElementKey, offset: Point) -> Self {
        Self {
            id,
            key,
            kind: ElementKind::ScrollBox(offset),
            text_source: TextSource::Unspecified,
            style: Style::default(),
            children: Vec::new(),
            tooltip: None,
            semantic: None,
        }
    }

    #[must_use]
    pub fn child(mut self, child: Self) -> Self {
        self.children.push(child);
        self
    }

    #[must_use]
    pub fn children(mut self, children: impl IntoIterator<Item = Self>) -> Self {
        self.children.extend(children);
        self
    }

    /// Depth-first lookup by stable identity.
    #[must_use]
    pub fn find(&self, id: ElementId) -> Option<&Self> {
        if self.id == id {
            Some(self)
        } else {
            self.children.iter().find_map(|child| child.find(id))
        }
    }

    pub(crate) fn validate_unique_identity(
        &self,
        ids: &mut HashSet<ElementId>,
        keys: &mut HashSet<ElementKey>,
    ) -> Result<(), DuplicateIdentity> {
        if !ids.insert(self.id) {
            return Err(DuplicateIdentity::Id(self.id));
        }
        if !keys.insert(self.key.clone()) {
            return Err(DuplicateIdentity::Key(self.key.clone()));
        }
        for child in &self.children {
            child.validate_unique_identity(ids, keys)?;
        }
        Ok(())
    }

    pub(crate) fn children_in_paint_order(&self) -> Vec<&Self> {
        let mut indexed: Vec<_> = self.children.iter().enumerate().collect();
        indexed.sort_by_key(|(index, child)| (child.style.z_index, *index));
        indexed.into_iter().map(|(_, child)| child).collect()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum DuplicateIdentity {
    Id(ElementId),
    Key(ElementKey),
}

#[cfg(test)]
mod scoped_identity_tests {
    use super::*;

    #[test]
    fn independently_allocated_scopes_cannot_collide_on_local_ids() {
        let allocator = ElementIdAllocator;
        let first = allocator.allocate_scope().expect("first scope");
        let second = allocator.allocate_scope().expect("second scope");

        assert_ne!(first.element(1), second.element(1));
        assert_eq!(first.element(1), first.element(1));
    }
}
