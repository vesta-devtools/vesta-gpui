//! Optional docking/workspace projection for reusable Vesta GPUI hosts.
//!
//! The authoritative dock tree remains in `vesta-workspace`. This crate only projects that
//! stable model into UI elements and typed user intent; it owns no renderer or product state.

#![forbid(unsafe_code)]

mod dock;
mod dock_input;
pub use dock_input::{
    cancel_tab_drag, finish_tab_drag_from, finish_tab_drag_with_admission, route_dock_pointer,
    route_dock_pointer_with_admission,
};
mod split_resize;
pub use split_resize::{SplitResizeError, SplitResizeSession};
mod tab_reveal;
pub use tab_reveal::{TabRevealGeometry, reveal_active_tabs};

pub use dock::{
    DOCK_DRAG_THRESHOLD, DockDragVisual, DockDropTarget, DockFrame, DockPaneFrame, DockSplitFrame,
    DockTabFrame, DockTabSplitRegion, DockWindowGeometry, FloatingMoveDrag, WorkspaceTabDrag,
    begin_tab_drag, dock_drag_overlay, dock_edge, dock_edge_band, dock_edge_preview,
    dock_split_ratio, dock_tab_split_regions, finish_tab_drag, leave_tab_drag,
    split_ratio_from_delta, tab_drag_visual, update_tab_drag,
};

use std::{error::Error, fmt};

use vesta_gpui::{
    Align, Axis, Dimension, Element, ElementId, ElementKey, ElementKind, ElementScope, FontRole,
    Insets, Justify, Point, Rect, ScrollbarGutter, ScrollbarStyle, Semantic, SemanticRole, Style,
    TextHAlign, TextVAlign,
};
use vesta_gpui_widgets::{
    CHROME_TAB_HEIGHT, CompactSign, ControlState, WidgetIdentity, WidgetTheme, button,
    compact_sign, tab,
};
use vesta_workspace::{
    ContainerRef, FloatingWindowId, ItemId, Pane, PaneId, SplitAxis, SplitId, WorkspaceModel,
    WorkspaceNode,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkspaceViewAction {
    Tab(ItemId),
    DockTarget(PaneId),
    CloseTab(ItemId),
    Navigate {
        pane: PaneId,
        active: ItemId,
        direction: PaneNavigationDirection,
    },
    HeaderCommand {
        pane: PaneId,
        active: ItemId,
        command: u8,
    },
    BeginResize {
        container: ContainerRef,
        split: SplitId,
        axis: SplitAxis,
    },
    BeginMoveFloating(FloatingWindowId),
    CloseFloating(FloatingWindowId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaneNavigationDirection {
    Back,
    Forward,
}

/// Host-evaluated state for one pane header. Command ids 1..=15 remain stable for a host.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PaneHeader {
    /// Decorative tab glyphs in the host's registered Icon font, keyed by stable item identity.
    pub tab_icons: Vec<(ItemId, char)>,
    pub navigation: Option<(bool, bool)>,
    pub commands: Vec<PaneHeaderCommand>,
    pub tab_scroll_x: f32,
    /// None uses the shared reserved gutter with the workspace theme's colors.
    pub tab_scrollbar: Option<ScrollbarStyle>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaneHeaderCommand {
    pub id: u8,
    pub caption: String,
    pub name: String,
    pub enabled: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WorkspaceView {
    pub element: Element,
    pub actions: Vec<(ElementId, WorkspaceViewAction)>,
    pub regions: WorkspaceViewRegions,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WorkspaceViewRegions {
    pub panes: Vec<WorkspacePaneRegion>,
    pub tabs: Vec<WorkspaceTabRegion>,
    pub splits: Vec<WorkspaceSplitRegion>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorkspacePaneRegion {
    pub pane: PaneId,
    pub container: ContainerRef,
    pub root: ElementId,
    pub tab_strip: ElementId,
    pub tab_list: ElementId,
    pub body: ElementId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorkspaceTabRegion {
    pub pane: PaneId,
    pub item: ItemId,
    pub tab: ElementId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorkspaceSplitRegion {
    pub container: ContainerRef,
    pub split: SplitId,
    pub axis: SplitAxis,
    pub root: ElementId,
}

/// Transient host input used only to paint workspace controls. It never enters the dock model.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WorkspaceViewInteraction {
    pub hovered: Option<ElementId>,
    pub focused: Option<ElementId>,
    pub close_on_hover: bool,
}

pub const MIN_DOCK_PANE_EXTENT: f32 = 120.0;
const PANE_NAVIGATION_WIDTH: f32 = 52.0;

/// Resolve one split drag against that split's own layout rectangle.
#[must_use]
pub fn split_ratio_at(
    rect: Rect,
    axis: SplitAxis,
    pointer: Point,
    minimum_pane_extent: f32,
) -> f32 {
    let (position, origin, extent) = match axis {
        SplitAxis::Row => (pointer.x, rect.x, rect.width),
        SplitAxis::Column => (pointer.y, rect.y, rect.height),
    };
    if !extent.is_finite() || extent <= 0.0 || !position.is_finite() {
        return 0.5;
    }
    let minimum = (minimum_pane_extent.max(0.0) / extent).clamp(0.05, 0.5);
    if minimum >= 0.5 {
        0.5
    } else {
        ((position - origin) / extent).clamp(minimum, 1.0 - minimum)
    }
}

impl WorkspaceView {
    #[must_use]
    pub fn action_at(&self, element: ElementId) -> Option<WorkspaceViewAction> {
        self.actions
            .iter()
            .find_map(|(candidate, action)| (*candidate == element).then_some(*action))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorkspaceIdentityOverflow;

impl fmt::Display for WorkspaceIdentityOverflow {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("workspace identity exceeds the reusable UI projection range")
    }
}

impl Error for WorkspaceIdentityOverflow {}

pub fn workspace_view(
    scope: ElementScope,
    model: &WorkspaceModel,
    theme: WidgetTheme,
) -> Result<WorkspaceView, WorkspaceIdentityOverflow> {
    workspace_view_with_content(scope, model, theme, |_| None)
}

/// Project a dock tree while the host supplies active pane content. The dock
/// layer keeps only pane/tab identity and intent; product views remain owned by
/// their host and can contain ordinary reusable widgets.
pub fn workspace_view_with_content(
    scope: ElementScope,
    model: &WorkspaceModel,
    theme: WidgetTheme,
    panel_content: impl FnMut(&Pane) -> Option<Element>,
) -> Result<WorkspaceView, WorkspaceIdentityOverflow> {
    workspace_view_with_interaction(
        scope,
        model,
        theme,
        WorkspaceViewInteraction::default(),
        panel_content,
    )
}

/// Project a workspace with the host's current pointer and keyboard focus state.
pub fn workspace_view_with_interaction(
    scope: ElementScope,
    model: &WorkspaceModel,
    theme: WidgetTheme,
    interaction: WorkspaceViewInteraction,
    panel_content: impl FnMut(&Pane) -> Option<Element>,
) -> Result<WorkspaceView, WorkspaceIdentityOverflow> {
    workspace_view_with_header(
        scope,
        model,
        theme,
        interaction,
        None,
        |_| PaneHeader::default(),
        panel_content,
    )
}

/// Project optional pane navigation and active-item commands supplied by the host.
/// Zoom changes only this projection; the dock tree and saved session stay intact.
pub fn workspace_view_with_header(
    scope: ElementScope,
    model: &WorkspaceModel,
    theme: WidgetTheme,
    interaction: WorkspaceViewInteraction,
    zoomed_pane: Option<PaneId>,
    mut pane_header: impl FnMut(&Pane) -> PaneHeader,
    mut panel_content: impl FnMut(&Pane) -> Option<Element>,
) -> Result<WorkspaceView, WorkspaceIdentityOverflow> {
    workspace_view_with_header_mode(
        scope,
        model,
        theme,
        interaction,
        zoomed_pane,
        &mut pane_header,
        &mut panel_content,
        true,
    )
}

/// Project only the main workspace container for a host that maps logical floating windows to
/// separate OS windows. The model and typed actions are shared with the complete fallback view.
pub fn workspace_main_view_with_header(
    scope: ElementScope,
    model: &WorkspaceModel,
    theme: WidgetTheme,
    interaction: WorkspaceViewInteraction,
    zoomed_pane: Option<PaneId>,
    mut pane_header: impl FnMut(&Pane) -> PaneHeader,
    mut panel_content: impl FnMut(&Pane) -> Option<Element>,
) -> Result<WorkspaceView, WorkspaceIdentityOverflow> {
    workspace_view_with_header_mode(
        scope,
        model,
        theme,
        interaction,
        zoomed_pane,
        &mut pane_header,
        &mut panel_content,
        false,
    )
}

/// Project exactly one workspace container. Native multi-window hosts use this to build each
/// logical floating window with the same pane, tab, split, and host-owned content projection as
/// the main window, while retaining independent per-window input and renderer state.
#[allow(clippy::too_many_arguments)]
pub fn workspace_container_view_with_header(
    scope: ElementScope,
    model: &WorkspaceModel,
    container: ContainerRef,
    theme: WidgetTheme,
    interaction: WorkspaceViewInteraction,
    zoomed_pane: Option<PaneId>,
    mut pane_header: impl FnMut(&Pane) -> PaneHeader,
    mut panel_content: impl FnMut(&Pane) -> Option<Element>,
) -> Result<Option<WorkspaceView>, WorkspaceIdentityOverflow> {
    let node = match container {
        ContainerRef::Main => model.root(),
        ContainerRef::Floating(window) => {
            let Some(window) = model.floating_window(window) else {
                return Ok(None);
            };
            &window.root
        }
    };
    let mut context = Projection {
        scope,
        model,
        theme,
        interaction,
        actions: Vec::new(),
        regions: WorkspaceViewRegions::default(),
        pane_header: &mut pane_header,
        panel_content: &mut panel_content,
    };
    let child = if let Some(pane) = zoomed_pane.filter(|pane| node.panes().contains(pane)) {
        context.pane(container, pane)?
    } else {
        context.node(container, node)?
    };
    let root = Element::container(scope.element(1), ElementKey::new("workspace.root"))
        .styled(
            Style::default()
                .size(Dimension::Percent(1.0), Dimension::Percent(1.0))
                .background(theme.palette.canvas)
                .pointer_events(false),
        )
        .child(child);
    Ok(Some(WorkspaceView {
        element: root,
        actions: context.actions,
        regions: context.regions,
    }))
}

#[allow(clippy::too_many_arguments)]
fn workspace_view_with_header_mode(
    scope: ElementScope,
    model: &WorkspaceModel,
    theme: WidgetTheme,
    interaction: WorkspaceViewInteraction,
    zoomed_pane: Option<PaneId>,
    pane_header: &mut dyn FnMut(&Pane) -> PaneHeader,
    panel_content: &mut dyn FnMut(&Pane) -> Option<Element>,
    include_floating: bool,
) -> Result<WorkspaceView, WorkspaceIdentityOverflow> {
    let mut context = Projection {
        scope,
        model,
        theme,
        interaction,
        actions: Vec::new(),
        regions: WorkspaceViewRegions::default(),
        pane_header,
        panel_content,
    };
    let main = if let Some(pane) = zoomed_pane.filter(|pane| model.root().panes().contains(pane)) {
        context.pane(ContainerRef::Main, pane)?
    } else {
        context.node(ContainerRef::Main, model.root())?
    };
    let mut root = Element::container(scope.element(1), ElementKey::new("workspace.root"))
        .styled(
            Style::default()
                .size(Dimension::Percent(1.0), Dimension::Percent(1.0))
                .background(theme.palette.canvas)
                .pointer_events(false),
        )
        .child(main);
    if include_floating {
        for window in model.floating_windows() {
            root = root.child(context.floating(window.id)?);
        }
    }
    Ok(WorkspaceView {
        element: root,
        actions: context.actions,
        regions: context.regions,
    })
}

struct Projection<'a> {
    scope: ElementScope,
    model: &'a WorkspaceModel,
    theme: WidgetTheme,
    interaction: WorkspaceViewInteraction,
    actions: Vec<(ElementId, WorkspaceViewAction)>,
    regions: WorkspaceViewRegions,
    pane_header: &'a mut dyn FnMut(&Pane) -> PaneHeader,
    panel_content: &'a mut dyn FnMut(&Pane) -> Option<Element>,
}

impl Projection<'_> {
    fn node(
        &mut self,
        container: ContainerRef,
        node: &WorkspaceNode,
    ) -> Result<Element, WorkspaceIdentityOverflow> {
        match node {
            WorkspaceNode::Leaf(pane) => self.pane(container, *pane),
            WorkspaceNode::Split {
                id,
                axis,
                ratio,
                first,
                second,
            } => {
                let local = stable_local(IdentityKind::SplitRoot, id.0)?;
                self.regions.splits.push(WorkspaceSplitRegion {
                    container,
                    split: *id,
                    axis: *axis,
                    root: self.scope.element(local),
                });
                let splitter_local = stable_local(IdentityKind::Splitter, id.0)?;
                let splitter_id = self.scope.element(splitter_local);
                let mut handle = vesta_gpui_widgets::splitter(
                    vesta_gpui_widgets::SplitterIds {
                        root: WidgetIdentity::new(
                            splitter_id,
                            format!("workspace.splitter.{}", id.0),
                        ),
                        line: WidgetIdentity::new(
                            self.scope
                                .element(stable_local(IdentityKind::SplitterLine, id.0)?),
                            format!("workspace.splitter.{}.line", id.0),
                        ),
                    },
                    WorkspaceViewAction::BeginResize {
                        container,
                        split: *id,
                        axis: *axis,
                    },
                    self.interaction.hovered == Some(splitter_id),
                    *axis == SplitAxis::Row,
                    self.theme,
                );
                handle.element.semantic = Some(
                    Semantic::new(SemanticRole::Splitter, "Resize dock panes")
                        .description(match axis {
                            SplitAxis::Row => "Left and Right resize horizontally",
                            SplitAxis::Column => "Up and Down resize vertically",
                        })
                        .numeric(f64::from(*ratio) * 100.0, 0.0, 100.0, Some(1.0)),
                );
                self.actions.extend(handle.actions);
                let direction = match axis {
                    SplitAxis::Row => Axis::Row,
                    SplitAxis::Column => Axis::Column,
                };
                let (first_width, first_height) = match axis {
                    SplitAxis::Row => (Dimension::Percent(*ratio), Dimension::Percent(1.0)),
                    SplitAxis::Column => (Dimension::Percent(1.0), Dimension::Percent(*ratio)),
                };
                let mut first_element = self.node(container, first)?;
                first_element.style.width = first_width;
                first_element.style.height = first_height;
                first_element.style.min_width = Dimension::Px(0.0);
                first_element.style.min_height = Dimension::Px(0.0);
                let mut second_element = self.node(container, second)?;
                second_element.style.width = Dimension::Auto;
                second_element.style.height = Dimension::Auto;
                second_element.style.min_width = Dimension::Px(0.0);
                second_element.style.min_height = Dimension::Px(0.0);
                second_element.style.flex_basis = Some(Dimension::Px(0.0));
                second_element.style.flex_grow = 1.0;
                second_element.style.flex_shrink = 1.0;
                Ok(Element::container(
                    self.scope.element(local),
                    ElementKey::new(format!("workspace.split.{}", id.0)),
                )
                .styled(
                    Style::default()
                        .size(Dimension::Percent(1.0), Dimension::Percent(1.0))
                        .min_width(Dimension::Px(0.0))
                        .min_height(Dimension::Px(0.0))
                        .axis(direction)
                        .pointer_events(false),
                )
                .child(first_element)
                .child(handle.element)
                .child(second_element))
            }
        }
    }

    fn pane(
        &mut self,
        container: ContainerRef,
        id: PaneId,
    ) -> Result<Element, WorkspaceIdentityOverflow> {
        let pane = self.model.pane(id).ok_or(WorkspaceIdentityOverflow)?;
        let content = (self.panel_content)(pane);
        let active_surface = content
            .as_ref()
            .and_then(|element| element.style.visual.background)
            .unwrap_or(self.theme.palette.field);
        let local = stable_local(IdentityKind::PaneRoot, id.0)?;
        let tabs_local = stable_local(IdentityKind::PaneTabs, id.0)?;
        let body_local = stable_local(IdentityKind::PaneBody, id.0)?;
        let body_label_local = stable_local(IdentityKind::PaneBodyLabel, id.0)?;
        let body_id = self.scope.element(body_local);
        self.regions.panes.push(WorkspacePaneRegion {
            pane: id,
            container,
            root: self.scope.element(local),
            tab_strip: self.scope.element(tabs_local),
            tab_list: self
                .scope
                .element(stable_local(IdentityKind::PaneTabList, id.0)?),
            body: body_id,
        });
        self.actions
            .push((body_id, WorkspaceViewAction::DockTarget(id)));
        let header = (self.pane_header)(pane);
        let scrollbar = header
            .tab_scrollbar
            .unwrap_or_else(|| self.theme.scrollbar_style());
        let reserve = scrollbar.gutter == ScrollbarGutter::Reserve;
        let header_height = if reserve {
            Dimension::Auto
        } else {
            Dimension::Px(CHROME_TAB_HEIGHT)
        };
        let header_align = if reserve { Align::Start } else { Align::Center };
        let mut tabs = Element::scroll_box(
            self.scope
                .element(stable_local(IdentityKind::PaneTabList, id.0)?),
            ElementKey::new(format!("workspace.pane.{}.tab_list", id.0)),
            Point::new(header.tab_scroll_x.max(0.0), 0.0),
        )
        .styled(
            Style::default()
                .size(Dimension::Px(0.0), header_height)
                .min_height(Dimension::Px(CHROME_TAB_HEIGHT))
                .min_width(Dimension::Px(0.0))
                .flex(1.0, 1.0)
                .axis(Axis::Row)
                .align(header_align)
                .gap(0.0)
                .padding(Insets::all(0.0))
                .clipped()
                .scrollbar(scrollbar)
                .pointer_events(reserve),
        );
        for item in &pane.items {
            let tab_local = stable_local(IdentityKind::ItemTab, item.id.0)?;
            let label_local = stable_local(IdentityKind::ItemLabel, item.id.0)?;
            let close_local = stable_local(IdentityKind::ItemClose, item.id.0)?;
            let close_label_local = stable_local(IdentityKind::ItemCloseLabel, item.id.0)?;
            self.regions.tabs.push(WorkspaceTabRegion {
                pane: id,
                item: item.id,
                tab: self.scope.element(tab_local),
            });
            let mut tab_widget = tab(
                WidgetIdentity::new(
                    self.scope.element(tab_local),
                    format!("workspace.item.{}.tab", item.id.0),
                ),
                WidgetIdentity::new(
                    self.scope.element(label_local),
                    format!("workspace.item.{}.label", item.id.0),
                ),
                &item.title,
                pane.active == Some(item.id),
                WorkspaceViewAction::Tab(item.id),
                ControlState::Idle,
                self.theme,
            );
            if pane.active == Some(item.id) {
                tab_widget.element.style.visual.background = Some(active_surface);
            }
            if let Some((_, glyph)) = header.tab_icons.iter().find(|(id, _)| *id == item.id) {
                tab_widget.element.style.axis = Axis::Row;
                tab_widget.element.style.gap = 6.0;
                tab_widget.element.children.insert(
                    0,
                    Element::text(
                        self.scope
                            .element(stable_local(IdentityKind::ItemIcon, item.id.0)?),
                        ElementKey::new(format!("workspace.item.{}.icon", item.id.0)),
                        glyph.to_string(),
                    )
                    .styled(
                        Style::default()
                            .size(Dimension::Px(16.0), Dimension::Percent(1.0))
                            .text(16.0, self.theme.palette.text)
                            .font_role(FontRole::Icon)
                            .text_align(TextHAlign::Center, TextVAlign::Center)
                            .pointer_events(false),
                    ),
                );
            }
            self.actions.extend(tab_widget.actions);
            let mut close_widget = button(
                WidgetIdentity::new(
                    self.scope.element(close_local),
                    format!("workspace.item.{}.close", item.id.0),
                ),
                WidgetIdentity::new(
                    self.scope.element(close_label_local),
                    format!("workspace.item.{}.close.label", item.id.0),
                ),
                "×",
                WorkspaceViewAction::CloseTab(item.id),
                ControlState::Idle,
                self.theme,
            );
            close_widget.element.style.width = Dimension::Px(26.0);
            close_widget.element.style.height = Dimension::Px(CHROME_TAB_HEIGHT);
            close_widget.element.style.padding = Insets::all(0.0);
            close_widget.element.style.visual.background = Some(if pane.active == Some(item.id) {
                active_surface
            } else {
                self.theme.palette.panel
            });
            close_widget.element.style.visual.border_width = 0.0;
            close_widget.element.style.visual.corner_radius = 0.0;
            close_widget.element.semantic = Some(Semantic::new(
                SemanticRole::Button,
                format!("Close {}", item.title),
            ));
            let close_visible = !self.interaction.close_on_hover
                || self.interaction.hovered == Some(self.scope.element(tab_local))
                || self.interaction.hovered == Some(self.scope.element(close_local))
                || self.interaction.focused == Some(self.scope.element(close_local));
            if !close_visible {
                close_widget.element.children[0].kind = ElementKind::Text(String::new());
                close_widget.actions.clear();
            }
            self.actions.extend(close_widget.actions);
            let mut group = Element::container(
                self.scope
                    .element(stable_local(IdentityKind::ItemGroup, item.id.0)?),
                ElementKey::new(format!("workspace.item.{}.group", item.id.0)),
            )
            .styled(
                Style::default()
                    .axis(Axis::Row)
                    .align(Align::Center)
                    .size(Dimension::Auto, Dimension::Px(CHROME_TAB_HEIGHT))
                    .background(if pane.active == Some(item.id) {
                        active_surface
                    } else {
                        self.theme.palette.panel
                    })
                    .pointer_events(false),
            )
            .child(tab_widget.element)
            .child(close_widget.element)
            .child(
                Element::container(
                    self.scope
                        .element(stable_local(IdentityKind::ItemSeparator, item.id.0)?),
                    ElementKey::new(format!("workspace.item.{}.separator", item.id.0)),
                )
                .styled(
                    Style::default()
                        .size(Dimension::Px(1.0), Dimension::Percent(1.0))
                        .background(self.theme.palette.border)
                        .pointer_events(false),
                ),
            );
            if pane.active != Some(item.id) {
                group = group.child(self.header_divider(
                    IdentityKind::ItemBottomDivider,
                    item.id.0,
                    format!("workspace.item.{}.bottom_divider", item.id.0),
                )?);
            }
            tabs = tabs.child(group);
        }
        let mut tab_strip = Element::container(
            self.scope.element(tabs_local),
            ElementKey::new(format!("workspace.pane.{}.tabs", id.0)),
        )
        .styled(
            Style::default()
                .size(Dimension::Percent(1.0), header_height)
                .min_height(Dimension::Px(CHROME_TAB_HEIGHT))
                .min_width(Dimension::Px(0.0))
                .axis(Axis::Row)
                .align(header_align)
                .background(self.theme.palette.panel)
                .pointer_events(false),
        )
        .child(self.header_divider(
            IdentityKind::PaneHeaderDivider,
            id.0,
            format!("workspace.pane.{}.header_divider", id.0),
        )?);
        if let Some((can_back, can_forward)) = header.navigation {
            let mut navigation = Element::container(
                self.scope
                    .element(stable_local(IdentityKind::PaneNavigation, id.0)?),
                ElementKey::new(format!("workspace.pane.{}.navigation", id.0)),
            )
            .styled(
                Style::default()
                    .size(
                        Dimension::Px(PANE_NAVIGATION_WIDTH),
                        Dimension::Px(CHROME_TAB_HEIGHT),
                    )
                    .axis(Axis::Row)
                    .flex(0.0, 0.0)
                    .pointer_events(false),
            );
            if let Some(active) = pane.active {
                for (direction, enabled, root_kind, label_kind, caption, name) in [
                    (
                        PaneNavigationDirection::Back,
                        can_back,
                        IdentityKind::PaneBack,
                        IdentityKind::PaneBackLabel,
                        "←",
                        "Back in pane history",
                    ),
                    (
                        PaneNavigationDirection::Forward,
                        can_forward,
                        IdentityKind::PaneForward,
                        IdentityKind::PaneForwardLabel,
                        "→",
                        "Forward in pane history",
                    ),
                ] {
                    navigation = navigation.child(self.header_button(
                        id.0,
                        (root_kind, label_kind),
                        (caption, name),
                        enabled,
                        WorkspaceViewAction::Navigate {
                            pane: id,
                            active,
                            direction,
                        },
                    )?);
                }
            }
            navigation = navigation.child(self.header_divider(
                IdentityKind::PaneNavigationDivider,
                id.0,
                format!("workspace.pane.{}.navigation_divider", id.0),
            )?);
            navigation = navigation.child(
                Element::container(
                    self.scope
                        .element(stable_local(IdentityKind::PaneNavigationSeparator, id.0)?),
                    ElementKey::new(format!("workspace.pane.{}.navigation_separator", id.0)),
                )
                .styled(
                    Style::default()
                        .absolute(PANE_NAVIGATION_WIDTH - 1.0, 0.0)
                        .size(Dimension::Px(1.0), Dimension::Percent(1.0))
                        .background(self.theme.palette.chrome_divider)
                        .pointer_events(false),
                ),
            );
            tab_strip = tab_strip.child(navigation);
        }
        tab_strip = tab_strip.child(tabs);
        if id == self.model.active_pane()
            && let Some(active) = pane.active
            && !header.commands.is_empty()
        {
            let mut commands = Element::container(
                self.scope
                    .element(stable_local(IdentityKind::PaneActions, id.0)?),
                ElementKey::new(format!("workspace.pane.{}.actions", id.0)),
            )
            .styled(
                Style::default()
                    .size(Dimension::Auto, Dimension::Px(CHROME_TAB_HEIGHT))
                    .axis(Axis::Row)
                    .pointer_events(false),
            );
            for command in header.commands {
                if !(1..=15).contains(&command.id) {
                    return Err(WorkspaceIdentityOverflow);
                }
                let value =
                    id.0.checked_mul(16)
                        .and_then(|base| base.checked_add(u64::from(command.id)))
                        .ok_or(WorkspaceIdentityOverflow)?;
                commands = commands.child(self.header_button(
                    value,
                    (IdentityKind::PaneCommand, IdentityKind::PaneCommandLabel),
                    (&command.caption, &command.name),
                    command.enabled,
                    WorkspaceViewAction::HeaderCommand {
                        pane: id,
                        active,
                        command: command.id,
                    },
                )?);
            }
            commands = commands.child(self.header_divider(
                IdentityKind::PaneActionsDivider,
                id.0,
                format!("workspace.pane.{}.actions_divider", id.0),
            )?);
            tab_strip = tab_strip.child(commands);
        }
        let active_text = pane
            .active_item()
            .map_or("Empty pane", |item| item.title.as_str());
        let custom_content = content.is_some();
        let mut body = Element::container(
            body_id,
            ElementKey::new(format!("workspace.pane.{}.body", id.0)),
        )
        .styled(
            Style::default()
                .size(Dimension::Percent(1.0), Dimension::Px(0.0))
                .min_height(Dimension::Px(0.0))
                .flex(1.0, 1.0)
                .align(if custom_content {
                    Align::Stretch
                } else {
                    Align::Center
                })
                .justify(if custom_content {
                    Justify::Start
                } else {
                    Justify::Center
                })
                .background(self.theme.palette.field)
                .pointer_events(true),
        );
        body = if let Some(content) = content {
            body.child(content)
        } else {
            body.child(
                Element::text(
                    self.scope.element(body_label_local),
                    ElementKey::new(format!("workspace.pane.{}.body.label", id.0)),
                    active_text,
                )
                .styled(
                    Style::default()
                        .size(Dimension::Percent(1.0), Dimension::Px(24.0))
                        .text(
                            self.theme.typography.font_size,
                            self.theme.palette.text_muted,
                        )
                        .text_align(TextHAlign::Center, TextVAlign::Center)
                        .pointer_events(false),
                ),
            )
        };
        Ok(Element::container(
            self.scope.element(local),
            ElementKey::new(format!("workspace.pane.{}", id.0)),
        )
        .styled(
            Style::default()
                .size(Dimension::Percent(1.0), Dimension::Percent(1.0))
                .min_width(Dimension::Px(0.0))
                .min_height(Dimension::Px(0.0))
                .axis(Axis::Column)
                .background(self.theme.palette.field)
                .clipped()
                .pointer_events(false),
        )
        .child(tab_strip)
        .child(body))
    }

    fn header_button(
        &mut self,
        value: u64,
        kinds: (IdentityKind, IdentityKind),
        labels: (&str, &str),
        enabled: bool,
        action: WorkspaceViewAction,
    ) -> Result<Element, WorkspaceIdentityOverflow> {
        let (root_kind, label_kind) = kinds;
        let (caption, name) = labels;
        let root = self.scope.element(stable_local(root_kind, value)?);
        let mut control = button(
            WidgetIdentity::new(root, format!("workspace.header.{value}.{:?}", root_kind)),
            WidgetIdentity::new(
                self.scope.element(stable_local(label_kind, value)?),
                format!("workspace.header.{value}.{:?}.label", root_kind),
            ),
            caption,
            action,
            if !enabled {
                ControlState::Disabled
            } else if self.interaction.hovered == Some(root) {
                ControlState::Hovered
            } else {
                ControlState::Idle
            },
            self.theme,
        );
        let navigation = match action {
            WorkspaceViewAction::Navigate { direction, .. } => Some(direction),
            _ => None,
        };
        control.element.style.width = Dimension::Px(if navigation.is_some() { 26.0 } else { 32.0 });
        control.element.style.height = Dimension::Px(CHROME_TAB_HEIGHT);
        control.element.style.flex_shrink = 0.0;
        control.element.style.padding = Insets::all(0.0);
        control.element.style.visual.background =
            Some(if enabled && self.interaction.hovered == Some(root) {
                self.theme.palette.surface_hovered
            } else {
                self.theme.palette.panel
            });
        control.element.style.visual.border_width = 0.0;
        control.element.style.visual.corner_radius = 0.0;
        control.element.semantic = Some(Semantic::new(SemanticRole::Button, name));
        control.element.tooltip = Some(name.to_owned());
        if let Some(label) = control.element.children.first_mut() {
            if let Some(direction) = navigation {
                let color = if enabled {
                    self.theme.palette.text
                } else {
                    self.theme.palette.text_disabled
                };
                let (head, shaft, shaft_kind) = match direction {
                    PaneNavigationDirection::Back => (
                        [
                            Point::new(8.5, 2.5),
                            Point::new(3.5, 7.0),
                            Point::new(8.5, 11.5),
                        ],
                        [Point::new(3.5, 7.0), Point::new(12.5, 7.0)],
                        IdentityKind::PaneBackShaft,
                    ),
                    PaneNavigationDirection::Forward => (
                        [
                            Point::new(5.5, 2.5),
                            Point::new(10.5, 7.0),
                            Point::new(5.5, 11.5),
                        ],
                        [Point::new(1.5, 7.0), Point::new(10.5, 7.0)],
                        IdentityKind::PaneForwardShaft,
                    ),
                };
                let mut icon = Element::container(label.id, label.key.clone()).styled(
                    Style::default()
                        .size(Dimension::Px(14.0), Dimension::Px(14.0))
                        .polyline(head.to_vec(), 1.2)
                        .background(color)
                        .pointer_events(false),
                );
                icon = icon.child(
                    Element::container(
                        self.scope.element(stable_local(shaft_kind, value)?),
                        ElementKey::new(format!("workspace.header.{value}.{shaft_kind:?}.shaft")),
                    )
                    .styled(
                        Style::default()
                            .size(Dimension::Px(14.0), Dimension::Px(14.0))
                            .absolute(0.0, 0.0)
                            .polyline(shaft.to_vec(), 1.2)
                            .background(color)
                            .pointer_events(false),
                    ),
                );
                *label = icon;
            } else if caption == "+" {
                *label = compact_sign(
                    WidgetIdentity {
                        element: label.id,
                        key: label.key.clone(),
                    },
                    CompactSign::Plus,
                    if enabled {
                        self.theme.palette.text
                    } else {
                        self.theme.palette.text_disabled
                    },
                );
            } else {
                label.style.visual.font_size = 18.0;
                // Glyph line boxes are centered, but their visible ink is not.
                // Keep these compact Pane command icons on the navigation/Tab centerline.
                label.style.top = match caption {
                    "◫" => Some(2.5),
                    "⤢" => Some(-1.25),
                    _ => None,
                };
            }
        }
        self.actions.extend(control.actions);
        Ok(control.element)
    }

    fn header_divider(
        &self,
        kind: IdentityKind,
        value: u64,
        key: String,
    ) -> Result<Element, WorkspaceIdentityOverflow> {
        Ok(Element::container(
            self.scope.element(stable_local(kind, value)?),
            ElementKey::new(key),
        )
        .styled(
            Style::default()
                .absolute(0.0, CHROME_TAB_HEIGHT - 1.0)
                .size(Dimension::Percent(1.0), Dimension::Px(1.0))
                .background(self.theme.palette.chrome_divider)
                .pointer_events(false),
        ))
    }

    fn floating(&mut self, id: FloatingWindowId) -> Result<Element, WorkspaceIdentityOverflow> {
        let window = self
            .model
            .floating_window(id)
            .ok_or(WorkspaceIdentityOverflow)?;
        let local = stable_local(IdentityKind::WindowRoot, id.0)?;
        let title_local = stable_local(IdentityKind::WindowTitle, id.0)?;
        self.actions.push((
            self.scope.element(title_local),
            WorkspaceViewAction::BeginMoveFloating(id),
        ));
        let close_local = stable_local(IdentityKind::WindowClose, id.0)?;
        let close_label_local = stable_local(IdentityKind::WindowCloseLabel, id.0)?;
        let mut close = button(
            WidgetIdentity::new(
                self.scope.element(close_local),
                format!("workspace.window.{}.close", id.0),
            ),
            WidgetIdentity::new(
                self.scope.element(close_label_local),
                format!("workspace.window.{}.close.label", id.0),
            ),
            "×",
            WorkspaceViewAction::CloseFloating(id),
            ControlState::Idle,
            self.theme,
        );
        close.element.style.width = Dimension::Px(26.0);
        close.element.style.height = Dimension::Px(CHROME_TAB_HEIGHT);
        close.element.style.padding = Insets::all(0.0);
        close.element.style.visual.border_width = 0.0;
        close.element.style.visual.corner_radius = 0.0;
        self.actions.extend(close.actions);
        Ok(Element::container(
            self.scope.element(local),
            ElementKey::new(format!("workspace.window.{}", id.0)),
        )
        .styled(
            Style::default()
                .absolute(window.placement.x, window.placement.y)
                .size(
                    Dimension::Px(window.placement.width),
                    Dimension::Px(window.placement.height),
                )
                .axis(Axis::Column)
                .background(self.theme.palette.panel)
                .border(1.5, self.theme.palette.focus)
                .radius(self.theme.metrics.radius)
                .clipped()
                .z_index(40)
                .pointer_events(false),
        )
        .child(
            Element::container(
                self.scope.element(title_local),
                ElementKey::new(format!("workspace.window.{}.title", id.0)),
            )
            .styled(
                Style::default()
                    .size(Dimension::Percent(1.0), Dimension::Px(CHROME_TAB_HEIGHT))
                    .axis(Axis::Row)
                    .align(Align::Center)
                    .padding(Insets::symmetric(8.0, 2.0))
                    .background(self.theme.palette.surface)
                    .pointer_events(true),
            )
            .child(
                Element::text(
                    self.scope
                        .element(stable_local(IdentityKind::WindowTitleLabel, id.0)?),
                    ElementKey::new(format!("workspace.window.{}.title.label", id.0)),
                    &window.title,
                )
                .styled(
                    Style::default()
                        .size(Dimension::Auto, Dimension::Percent(1.0))
                        .flex(1.0, 1.0)
                        .text(self.theme.typography.font_size, self.theme.palette.text)
                        .pointer_events(false),
                ),
            )
            .child(close.element),
        )
        .child(self.node(ContainerRef::Floating(id), &window.root)?))
    }
}

#[derive(Clone, Copy, Debug)]
enum IdentityKind {
    PaneRoot = 1,
    PaneTabs = 2,
    PaneBody = 3,
    PaneBodyLabel = 4,
    ItemGroup = 5,
    ItemTab = 6,
    ItemLabel = 7,
    ItemClose = 8,
    ItemCloseLabel = 9,
    SplitRoot = 10,
    Splitter = 11,
    WindowRoot = 12,
    WindowTitle = 13,
    WindowTitleLabel = 14,
    WindowClose = 15,
    WindowCloseLabel = 16,
    SplitterLine = 17,
    ItemSeparator = 18,
    PaneTabList = 19,
    PaneNavigation = 20,
    PaneBack = 21,
    PaneBackLabel = 22,
    PaneForward = 23,
    PaneForwardLabel = 24,
    PaneActions = 25,
    PaneCommand = 26,
    PaneCommandLabel = 27,
    PaneBackShaft = 28,
    PaneForwardShaft = 29,
    PaneHeaderDivider = 30,
    PaneNavigationDivider = 31,
    PaneActionsDivider = 32,
    ItemBottomDivider = 33,
    PaneNavigationSeparator = 34,
    ItemIcon = 35,
}

fn stable_local(kind: IdentityKind, value: u64) -> Result<u32, WorkspaceIdentityOverflow> {
    let value = u32::try_from(value).map_err(|_| WorkspaceIdentityOverflow)?;
    if value > 0x00ff_ffff {
        return Err(WorkspaceIdentityOverflow);
    }
    Ok(((kind as u32) << 24) | value)
}

#[cfg(test)]
mod tests;
