//! Per-window scroll projection and the existing retained-state adapters.
use super::*;

/// One scroll viewport of one window's layout (ADR-0253 decision 7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ScrollTarget {
    Gallery,
    Nested,
    Horizontal,
    VirtualList,
    VirtualTable,
    Tree,
    Page(ItemId),
    Tabs(PaneId),
}

/// Scroll viewport rectangles captured from one window's own layout. Scroll states stay
/// shared; only the geometry that maps a pointer to a viewport is per window.
#[derive(Clone, Debug, Default)]
pub(super) struct ScrollGeometry {
    pub(super) gallery: Option<Rect>,
    pub(super) nested: Option<Rect>,
    pub(super) horizontal: Option<Rect>,
    pub(super) virtual_list: Option<Rect>,
    pub(super) virtual_table: Option<Rect>,
    pub(super) tree: Option<Rect>,
    pub(super) pages: HashMap<ItemId, Rect>,
    pub(super) tabs: HashMap<ElementId, (PaneId, Rect)>,
    pub(super) nodes: HashMap<ElementId, LayoutNode>,
    /// Full layout bounds for outer-ancestor reveal; pointer routing uses clipped rects.
    pub(super) unclipped: HashMap<ElementId, Rect>,
}

impl ScrollGeometry {
    pub(super) fn from_layout(
        gallery: ElementScope,
        layout: &LayoutTree,
        pages: impl Iterator<Item = ItemId>,
    ) -> Self {
        let clipped = |element: ElementId| {
            layout.node(element).map(|node| {
                node.rect
                    .intersection(node.inherited_clip)
                    .unwrap_or(node.rect)
            })
        };
        let mut geometry = Self {
            gallery: clipped(gallery.element(3)),
            nested: clipped(gallery.element(NESTED_SCROLL)),
            horizontal: clipped(gallery.element(HORIZONTAL_SCROLL)),
            virtual_list: clipped(gallery.element(VIRTUAL_SCROLL)),
            virtual_table: clipped(gallery.element(TABLE_VIRTUAL_ROOT + 2)),
            tree: clipped(gallery.element(TREE_SCROLL)),
            pages: pages
                .filter_map(|item| clipped(page_scroll_id(gallery, item)).map(|rect| (item, rect)))
                .collect(),
            unclipped: HashMap::new(),
            ..Self::default()
        };
        geometry.unclipped = geometry
            .targets(gallery)
            .into_iter()
            .filter_map(|(id, _, _)| layout.node(id).map(|node| (id, node.scroll_viewport())))
            .collect();
        geometry.nodes = geometry
            .targets(gallery)
            .into_iter()
            .filter_map(|(id, _, _)| layout.node(id).map(|node| (id, node.clone())))
            .collect();
        geometry
    }

    pub(super) fn with_tabs(mut self, layout: &LayoutTree, regions: &WorkspaceViewRegions) -> Self {
        for region in &regions.panes {
            if let Some(node) = layout.node(region.tab_list) {
                let rect = node
                    .rect
                    .intersection(node.inherited_clip)
                    .unwrap_or(node.rect);
                self.tabs.insert(node.id, (region.pane, rect));
                self.nodes.insert(node.id, node.clone());
                self.unclipped.insert(node.id, node.scroll_viewport());
            }
        }
        self
    }

    /// Candidate scrollbars in the order the main window has always tested them.
    pub(super) fn targets(&self, gallery: ElementScope) -> Vec<(ElementId, Rect, ScrollTarget)> {
        let mut targets = Vec::new();
        for (rect, target, element) in [
            (self.tree, ScrollTarget::Tree, gallery.element(TREE_SCROLL)),
            (
                self.virtual_list,
                ScrollTarget::VirtualList,
                gallery.element(VIRTUAL_SCROLL),
            ),
            (
                self.virtual_table,
                ScrollTarget::VirtualTable,
                gallery.element(TABLE_VIRTUAL_ROOT + 2),
            ),
            (
                self.nested,
                ScrollTarget::Nested,
                gallery.element(NESTED_SCROLL),
            ),
            (
                self.horizontal,
                ScrollTarget::Horizontal,
                gallery.element(HORIZONTAL_SCROLL),
            ),
            (self.gallery, ScrollTarget::Gallery, gallery.element(3)),
        ] {
            if let Some(rect) = rect {
                targets.push((element, rect, target));
            }
        }
        let mut pages = self.pages.iter().collect::<Vec<_>>();
        pages.sort_by_key(|(item, _)| item.0);
        targets.extend(pages.into_iter().map(|(&item, &rect)| {
            (
                page_scroll_id(gallery, item),
                rect,
                ScrollTarget::Page(item),
            )
        }));
        let mut tabs = self.tabs.iter().collect::<Vec<_>>();
        tabs.sort_by_key(|(_, (pane, _))| pane.0);
        targets.extend(
            tabs.into_iter()
                .map(|(&id, &(pane, rect))| (id, rect, ScrollTarget::Tabs(pane))),
        );
        targets
    }
}

/// The shared scroll states a window can drive; `gallery` is `None` for native floating
/// windows, which do not project the main gallery scroll container.
pub(super) struct ScrollStates<'a> {
    pub(super) tree: &'a mut ScrollAreaState,
    pub(super) virtual_list: &'a mut ScrollAreaState,
    pub(super) virtual_table: &'a mut ScrollAreaState,
    pub(super) nested: &'a mut ScrollAreaState,
    pub(super) horizontal: &'a mut ScrollAreaState,
    pub(super) gallery: Option<&'a mut ScrollAreaState>,
    pub(super) pages: &'a mut HashMap<ItemId, ScrollAreaState>,
    pub(super) tabs: &'a mut HashMap<PaneId, ScrollAreaState>,
}

pub(super) use vesta_gpui_workspace::TabRevealGeometry;

/// Reveal after selection or reflow; a user's wheel/thumb movement remains intact.
pub(super) fn reveal_active_tabs(
    regions: &WorkspaceViewRegions,
    layout: &LayoutTree,
    model: &WorkspaceModel,
    states: &mut HashMap<PaneId, ScrollAreaState>,
    previous: &mut HashMap<PaneId, TabRevealGeometry>,
) -> bool {
    vesta_gpui_workspace::reveal_active_tabs(regions, layout, model, states, previous, |region| {
        region.pane
    })
}

impl<'a> ScrollStates<'a> {
    pub(super) fn sync_layout(
        &mut self,
        gallery: ElementScope,
        geometry: &ScrollGeometry,
        layout: &LayoutTree,
    ) -> bool {
        let mut changed = false;
        for (element, _, target) in geometry.targets(gallery) {
            if let Some(node) = layout.node(element)
                && let Some(state) = self.for_target(target)
            {
                changed |= state.sync_layout(node);
            }
        }
        changed
    }

    pub(super) fn for_target(&mut self, target: ScrollTarget) -> Option<&mut ScrollAreaState> {
        match target {
            ScrollTarget::Tree => Some(self.tree),
            ScrollTarget::VirtualList => Some(self.virtual_list),
            ScrollTarget::VirtualTable => Some(self.virtual_table),
            ScrollTarget::Nested => Some(self.nested),
            ScrollTarget::Horizontal => Some(self.horizontal),
            ScrollTarget::Gallery => self.gallery.as_deref_mut(),
            ScrollTarget::Page(item) => Some(self.pages.entry(item).or_default()),
            ScrollTarget::Tabs(pane) => Some(self.tabs.entry(pane).or_default()),
        }
    }
}
