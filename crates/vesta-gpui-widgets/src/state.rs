use crate::{PopupCloseReason, PopupDismissal};
#[cfg(test)]
use vesta_gpui::scrollbar_geometry;
use vesta_gpui::{
    ElementId, FlowDirection, FocusScope, InputTarget, KeyChord, KeyStroke, LayoutNode, Menu,
    MenuId, MenuItemId, Point, Rect, Size, UiWindowId, scrollbar_geometry_in_direction,
};

/// Host-provided visible scroll geometry; margins remain a presentation choice.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollRevealViewport {
    pub rect: Rect,
    pub margin: f32,
}

/// One inner-to-outer reveal request for an existing retained scroll state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollRevealStep {
    pub element: ElementId,
    pub target: Rect,
    pub viewport: ScrollRevealViewport,
}

/// Reveal a focused descendant through its current root-to-target action path.
/// After revealing an inner viewport, its outer ancestor reveals the viewport itself.
/// Invalid geometry rejects the whole plan before any returned steps are applied.
#[must_use]
pub fn scroll_reveal_path(
    path: &[ElementId],
    mut target: Rect,
    mut viewport: impl FnMut(ElementId) -> Option<ScrollRevealViewport>,
) -> Vec<ScrollRevealStep> {
    if !finite_scroll_rect(target) {
        return Vec::new();
    }
    let mut steps = Vec::new();
    for &element in path.iter().rev() {
        if let Some(viewport) = viewport(element) {
            if !finite_scroll_rect(viewport.rect) || !viewport.margin.is_finite() {
                return Vec::new();
            }
            steps.push(ScrollRevealStep {
                element,
                target,
                viewport,
            });
            target = viewport.rect;
        }
    }
    steps
}

/// Offer each axis's remainder to live scroll owners, innermost first. The host
/// supplies retained states by stable element identity; this function owns only
/// propagation. A foreign, disabled, removed or modal-excluded hit receives no update.
pub fn scroll_wheel_path<A>(
    scope: &FocusScope<'_, A>,
    target: InputTarget,
    movement: Point,
    scroll: impl FnMut(ElementId, Point) -> Point,
) -> Point {
    let Some(path) = crate::ScrollInputPath::new(scope, target) else {
        return movement;
    };
    path.scroll(movement, scroll)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollAxis {
    Horizontal,
    Vertical,
}

/// A thumb drag keeps its starting geometry so pointer movement maps to content offset.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollbarDrag {
    axis: ScrollAxis,
    flow: FlowDirection,
    pointer_start: f32,
    offset_start: f32,
    travel: f32,
}

impl ScrollbarDrag {
    #[must_use]
    pub const fn axis(self) -> ScrollAxis {
        self.axis
    }

    pub fn update(self, state: &mut ScrollAreaState, pointer: Point) -> bool {
        if !pointer.x.is_finite() || !pointer.y.is_finite() {
            return false;
        }
        let max = match self.axis {
            ScrollAxis::Horizontal => state.max.x,
            ScrollAxis::Vertical => state.max.y,
        };
        if max == 0.0 {
            return false;
        }
        let old = state.offset;
        match self.axis {
            ScrollAxis::Horizontal => {
                state.offset.x = (self.offset_start
                    + self.flow.horizontal(pointer.x - self.pointer_start) * state.max.x
                        / self.travel)
                    .clamp(0.0, state.max.x);
            }
            ScrollAxis::Vertical => {
                state.offset.y = (self.offset_start
                    + (pointer.y - self.pointer_start) * state.max.y / self.travel)
                    .clamp(0.0, state.max.y);
            }
        }
        state.offset != old
    }
}

/// Retained scroll offset for a declarative `ScrollBox`. Positive deltas move
/// content upward; unconsumed delta can be offered to an ancestor scroll area.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScrollAreaState {
    offset: Point,
    max: Point,
    flow: FlowDirection,
}

impl ScrollAreaState {
    #[must_use]
    pub fn begin_thumb_drag(self, viewport: Rect, pointer: Point) -> Option<ScrollbarDrag> {
        let content = Size::new(viewport.width + self.max.x, viewport.height + self.max.y);
        if !finite_scroll_rect(viewport) || !finite_scroll_size(content) {
            return None;
        }
        let (vertical_track, vertical_thumb, horizontal_track, horizontal_thumb) =
            scrollbar_geometry_in_direction(viewport, content, self.offset, self.flow);
        self.begin_drag_from_geometry(
            pointer,
            vertical_track,
            vertical_thumb,
            horizontal_track,
            horizontal_thumb,
            self.flow,
        )
    }

    /// Use the same reserved gutters and tracks as the window's painter.
    #[must_use]
    pub fn begin_thumb_drag_in_layout(
        self,
        node: &LayoutNode,
        pointer: Point,
    ) -> Option<ScrollbarDrag> {
        let (width, height) = node.scroll_content.unwrap_or_default();
        if !finite_scroll_rect(node.rect)
            || !finite_scroll_rect(node.scroll_viewport())
            || !finite_scroll_size(Size::new(width, height))
        {
            return None;
        }
        let (vertical_track, vertical_thumb, horizontal_track, horizontal_thumb) =
            node.scrollbar_geometry(self.offset);
        self.begin_drag_from_geometry(
            pointer,
            vertical_track,
            vertical_thumb,
            horizontal_track,
            horizontal_thumb,
            node.flow_direction,
        )
    }

    fn begin_drag_from_geometry(
        self,
        pointer: Point,
        vertical_track: Option<Rect>,
        vertical_thumb: Option<Rect>,
        horizontal_track: Option<Rect>,
        horizontal_thumb: Option<Rect>,
        flow: FlowDirection,
    ) -> Option<ScrollbarDrag> {
        if !pointer.x.is_finite() || !pointer.y.is_finite() {
            return None;
        }
        let candidates = [
            (ScrollAxis::Vertical, vertical_track, vertical_thumb),
            (ScrollAxis::Horizontal, horizontal_track, horizontal_thumb),
        ];
        candidates.into_iter().find_map(|(axis, track, thumb)| {
            let (Some(track), Some(thumb)) = (track, thumb) else {
                return None;
            };
            if !finite_scroll_rect(track) || !finite_scroll_rect(thumb) || !thumb.contains(pointer)
            {
                return None;
            }
            let (pointer_start, offset_start, travel) = match axis {
                ScrollAxis::Horizontal => (
                    pointer.x,
                    self.offset.x,
                    (track.width - thumb.width).max(0.0),
                ),
                ScrollAxis::Vertical => (
                    pointer.y,
                    self.offset.y,
                    (track.height - thumb.height).max(0.0),
                ),
            };
            (travel > 0.0).then_some(ScrollbarDrag {
                axis,
                flow,
                pointer_start,
                offset_start,
                travel,
            })
        })
    }

    #[must_use]
    pub fn new(initial_y: f32) -> Self {
        Self::new_2d(Point::new(0.0, initial_y))
    }

    #[must_use]
    pub fn new_2d(initial: Point) -> Self {
        let component = |value: f32| {
            if value.is_finite() {
                value.max(0.0)
            } else {
                0.0
            }
        };
        let initial = Point::new(component(initial.x), component(initial.y));
        Self {
            offset: initial,
            max: initial,
            flow: FlowDirection::LeftToRight,
        }
    }

    #[must_use]
    pub const fn offset(self) -> Point {
        self.offset
    }

    #[must_use]
    pub const fn offset_y(self) -> f32 {
        self.offset.y
    }

    #[must_use]
    pub const fn max(self) -> Point {
        self.max
    }

    #[must_use]
    pub const fn max_y(self) -> f32 {
        self.max.y
    }

    /// Synchronize direction and both axes from the window's current projection.
    pub fn sync_layout(&mut self, node: &LayoutNode) -> bool {
        let (width, height) = node.scroll_content.unwrap_or_default();
        let viewport = node.scroll_viewport();
        if !finite_scroll_size(Size::new(width, height))
            || !finite_scroll_size(Size::new(viewport.width, viewport.height))
        {
            return false;
        }
        let direction_changed = self.flow != node.flow_direction;
        self.flow = node.flow_direction;
        self.sync_content_size(
            Size::new(width, height),
            Size::new(viewport.width, viewport.height),
        ) || direction_changed
    }

    /// Reconcile both axes after a declarative layout rebuild.
    pub fn sync_content_size(&mut self, content: Size, visible: Size) -> bool {
        if !finite_scroll_size(content) || !finite_scroll_size(visible) {
            return false;
        }
        self.max = Point::new(
            (content.width - visible.width).max(0.0),
            (content.height - visible.height).max(0.0),
        );
        let old = self.offset;
        self.offset.x = self.offset.x.min(self.max.x);
        self.offset.y = self.offset.y.min(self.max.y);
        self.offset != old
    }

    /// Reconcile a rebuilt layout with retained scroll state.
    pub fn sync_content_height(&mut self, content_height: f32, visible_height: f32) -> bool {
        if !finite_scroll_size(Size::new(content_height, visible_height)) {
            return false;
        }
        self.max.y = (content_height - visible_height).max(0.0);
        let old = self.offset.y;
        self.offset.y = self.offset.y.min(self.max.y);
        self.offset.y != old
    }

    /// Returns the unconsumed movement for each axis so an ancestor may scroll it.
    pub fn scroll_by_point(&mut self, delta: Point) -> Point {
        if !delta.x.is_finite() || !delta.y.is_finite() {
            return delta;
        }
        let logical_x = self.flow.horizontal(delta.x);
        let old = self.offset;
        self.offset.x = (self.offset.x + logical_x).clamp(0.0, self.max.x);
        self.offset.y = (self.offset.y + delta.y).clamp(0.0, self.max.y);
        Point::new(
            self.flow.horizontal(logical_x - (self.offset.x - old.x)),
            delta.y - (self.offset.y - old.y),
        )
    }

    /// Returns the portion not consumed at this scroll boundary.
    pub fn scroll_by(&mut self, delta_y: f32) -> f32 {
        self.scroll_by_point(Point::new(0.0, delta_y)).y
    }

    pub fn reveal(&mut self, target: Rect, visible: Rect, margin: f32) -> bool {
        if !finite_scroll_rect(target) || !finite_scroll_rect(visible) || !margin.is_finite() {
            return false;
        }
        let margin = margin.max(0.0);
        // A requested margin must not push a nearly full target out of view.
        let margin_x = margin.min(((visible.width - target.width) * 0.5).max(0.0));
        let margin_y = margin.min(((visible.height - target.height) * 0.5).max(0.0));
        let delta_x = if target.x < visible.x + margin_x {
            target.x - visible.x - margin_x
        } else if target.right() > visible.right() - margin_x {
            target.right() - visible.right() + margin_x
        } else {
            0.0
        };
        let delta_y = if target.y < visible.y + margin_y {
            target.y - visible.y - margin_y
        } else if target.bottom() > visible.bottom() - margin_y {
            target.bottom() - visible.bottom() + margin_y
        } else {
            0.0
        };
        let old = self.offset;
        // A revealed edge can return a few f32 rounding units beyond its margin
        // after layout subtracts the retained offset. Do not schedule another
        // frame solely to chase that cancellation noise. Real wheel/drag deltas
        // are not subject to this reveal-only arithmetic tolerance.
        let stable_delta = |delta: f32, start: f32, end: f32, low: f32, high: f32| {
            let precision = [start, end, low, high]
                .into_iter()
                .map(f32::abs)
                .fold(1.0_f32, f32::max)
                * f32::EPSILON
                * 4.0;
            if delta.abs() <= precision { 0.0 } else { delta }
        };
        self.scroll_by_point(Point::new(
            stable_delta(
                delta_x,
                target.x,
                target.right(),
                visible.x,
                visible.right(),
            ),
            stable_delta(
                delta_y,
                target.y,
                target.bottom(),
                visible.y,
                visible.bottom(),
            ),
        ));
        self.offset != old
    }
}

fn finite_scroll_size(size: Size) -> bool {
    size.width.is_finite() && size.height.is_finite() && size.width >= 0.0 && size.height >= 0.0
}

fn finite_scroll_rect(rect: Rect) -> bool {
    rect.x.is_finite()
        && rect.y.is_finite()
        && finite_scroll_size(Size::new(rect.width, rect.height))
        && rect.right().is_finite()
        && rect.bottom().is_finite()
}

/// Keyboard intent from one open menu. The host resolves descriptors and executes actions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuKeyAction {
    Updated,
    OpenSubmenu(MenuId),
    Activate(MenuItemId),
    Close,
    TraverseFocus { backwards: bool },
    Consumed,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MenuController {
    path: Vec<MenuId>,
    highlighted: Vec<Option<MenuItemId>>,
    return_focus: Option<InputTarget>,
    owner_window: Option<UiWindowId>,
}

impl MenuController {
    #[must_use]
    pub fn is_open(&self) -> bool {
        !self.path.is_empty()
    }

    #[must_use]
    pub fn path(&self) -> &[MenuId] {
        &self.path
    }

    #[must_use]
    pub fn current(&self) -> Option<MenuId> {
        self.path.last().copied()
    }

    #[must_use]
    pub fn highlighted(&self) -> Option<MenuItemId> {
        self.highlighted.last().copied().flatten()
    }

    /// Parent menus retain their own highlighted submenu opener while a child is active.
    #[must_use]
    pub fn highlighted_in(&self, menu: MenuId) -> Option<MenuItemId> {
        self.path
            .iter()
            .position(|id| *id == menu)
            .and_then(|depth| self.highlighted.get(depth))
            .copied()
            .flatten()
    }

    #[must_use]
    pub fn return_focus(&self) -> Option<InputTarget> {
        self.return_focus
    }

    pub fn open(&mut self, menu: MenuId) {
        self.path.clear();
        self.highlighted.clear();
        self.return_focus = None;
        self.owner_window = None;
        self.path.push(menu);
        self.highlighted.push(None);
    }

    /// Save the opener's focus for keyboard dismissal. The host decides
    /// whether to restore it after `close`, since a pointer may focus another
    /// control while the menu is open.
    pub fn open_for(&mut self, menu: MenuId, return_focus: Option<InputTarget>) {
        self.open(menu);
        self.return_focus = return_focus;
        self.owner_window = return_focus.map(|target| target.window);
    }

    /// Explicit window ownership is required even if this menu has no opener.
    pub fn open_in(&mut self, window: UiWindowId, menu: MenuId, return_focus: Option<InputTarget>) {
        self.open(menu);
        self.owner_window = Some(window);
        self.return_focus = return_focus.filter(|target| target.window == window);
    }

    pub fn toggle(&mut self, menu: MenuId) {
        if self.path.first().copied() == Some(menu) {
            let _ = self.close();
        } else {
            self.open(menu);
        }
    }

    pub fn close(&mut self) -> Option<InputTarget> {
        self.path.clear();
        self.highlighted.clear();
        self.owner_window = None;
        self.return_focus.take()
    }

    /// Consume the owning session once, including a reconciled-away menu root.
    pub fn dismiss(
        &mut self,
        source_window: UiWindowId,
        reason: PopupCloseReason,
    ) -> Option<PopupDismissal> {
        if self.owner_window != Some(source_window) {
            return None;
        }
        Some(PopupDismissal {
            source_window,
            reason,
            opener: self.close(),
        })
    }

    /// A descriptor update cannot re-enter an ancestor under duplicate popup IDs.
    pub fn enter_submenu(&mut self, menu: MenuId) {
        if self.path.contains(&menu) {
            return;
        }
        self.path.push(menu);
        self.highlighted.push(None);
    }

    pub fn leave_submenu(&mut self) -> bool {
        if self.path.len() <= 1 {
            return false;
        }
        self.path.pop();
        self.highlighted.pop();
        true
    }

    pub fn highlight(&mut self, item: Option<MenuItemId>) {
        if let Some(highlighted) = self.highlighted.last_mut() {
            *highlighted = item;
        }
    }

    /// Revalidate the open path against the available descriptors after an update.
    /// Stable enabled selections survive reorder. Invalid selections clear without
    /// choosing another action, and invalid submenu links discard their descendants.
    /// A missing root empties the path; `close` still consumes the saved opener.
    pub fn reconcile<A>(&mut self, menus: &[Menu<A>]) -> bool {
        let previous_path = self.path.clone();
        let previous_highlights = self.highlighted.clone();
        for depth in 0..self.path.len() {
            let Some(menu) = menus.iter().find(|menu| menu.id == self.path[depth]) else {
                self.path.truncate(depth);
                self.highlighted.truncate(depth);
                break;
            };
            let selected = menu.items.iter().find(|item| {
                Some(item.id) == self.highlighted[depth] && item.enabled && !item.separator
            });
            self.highlighted[depth] = selected.map(|item| item.id);
            if let Some(child) = self.path.get(depth + 1)
                && selected.and_then(|item| item.submenu) != Some(*child)
            {
                self.path.truncate(depth + 1);
                self.highlighted.truncate(depth + 1);
                break;
            }
        }
        self.path != previous_path || self.highlighted != previous_highlights
    }

    /// Pointer selection uses the same item and path state as keyboard navigation.
    /// Returning to an ancestor item dismisses its open descendants.
    pub fn highlight_in<A>(&mut self, menu: &Menu<A>, item: MenuItemId) -> bool {
        if !menu
            .items
            .iter()
            .any(|candidate| candidate.id == item && candidate.enabled && !candidate.separator)
        {
            return false;
        }
        let Some(depth) = self.path.iter().rposition(|id| *id == menu.id) else {
            return false;
        };
        if self.path.len() > depth + 1 && self.highlighted[depth] == Some(item) {
            return false;
        }
        let changed = self.path.len() != depth + 1 || self.highlighted[depth] != Some(item);
        self.path.truncate(depth + 1);
        self.highlighted.truncate(depth + 1);
        self.highlighted[depth] = Some(item);
        changed
    }

    pub fn step<A>(&mut self, menu: &Menu<A>, delta: i32) {
        let enabled = menu
            .items
            .iter()
            .filter(|item| item.enabled && !item.separator)
            .map(|item| item.id)
            .collect::<Vec<_>>();
        if enabled.is_empty() {
            self.highlight(None);
            return;
        }
        let current = self
            .highlighted()
            .and_then(|id| enabled.iter().position(|candidate| *candidate == id));
        let next = match (current, delta.signum()) {
            (None, direction) if direction < 0 => enabled.len() - 1,
            (None, _) => 0,
            (Some(index), direction) if direction < 0 => {
                index.checked_sub(1).unwrap_or(enabled.len() - 1)
            }
            (Some(index), _) => (index + 1) % enabled.len(),
        };
        self.highlight(Some(enabled[next]));
    }

    pub fn select_edge<A>(&mut self, menu: &Menu<A>, end: bool) {
        if self.current() != Some(menu.id) {
            return;
        }
        let mut enabled = menu
            .items
            .iter()
            .filter(|item| item.enabled && !item.separator);
        self.highlight(if end {
            enabled.next_back().map(|item| item.id)
        } else {
            enabled.next().map(|item| item.id)
        });
    }

    /// Navigate the current descriptor using stable enabled item identities. A stale or
    /// absent highlight never activates a replacement item. All keys belong to the menu
    /// until the host closes it, preserving shortcut precedence for an open popup.
    #[must_use]
    pub fn key_action<A>(&mut self, menu: &Menu<A>, chord: KeyChord) -> MenuKeyAction {
        if self.current() != Some(menu.id) {
            return MenuKeyAction::Consumed;
        }
        match chord.key {
            KeyStroke::ArrowDown => self.step(menu, 1),
            KeyStroke::ArrowUp => self.step(menu, -1),
            KeyStroke::Home => self.select_edge(menu, false),
            KeyStroke::End => self.select_edge(menu, true),
            KeyStroke::ArrowLeft => {
                if !self.leave_submenu() {
                    return MenuKeyAction::Close;
                }
            }
            KeyStroke::Escape => return MenuKeyAction::Close,
            KeyStroke::Tab => {
                return MenuKeyAction::TraverseFocus {
                    backwards: chord.modifiers.shift,
                };
            }
            KeyStroke::ArrowRight | KeyStroke::Enter | KeyStroke::Space => {
                let Some(item) = menu.items.iter().find(|item| {
                    Some(item.id) == self.highlighted() && item.enabled && !item.separator
                }) else {
                    return MenuKeyAction::Consumed;
                };
                return match chord.key {
                    KeyStroke::ArrowRight => item
                        .submenu
                        .map_or(MenuKeyAction::Consumed, MenuKeyAction::OpenSubmenu),
                    _ => MenuKeyAction::Activate(item.id),
                };
            }
            _ => return MenuKeyAction::Consumed,
        }
        MenuKeyAction::Updated
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TabBarState<T> {
    active: Option<T>,
    order: Vec<T>,
    pinned: Vec<T>,
    overflow_offset: usize,
}

impl<T: Copy + PartialEq> TabBarState<T> {
    #[must_use]
    pub fn new(order: impl IntoIterator<Item = T>, active: Option<T>) -> Self {
        let order = order.into_iter().collect::<Vec<_>>();
        let active = active.filter(|candidate| order.contains(candidate));
        Self {
            active,
            order,
            pinned: Vec::new(),
            overflow_offset: 0,
        }
    }

    #[must_use]
    pub fn active(&self) -> Option<T> {
        self.active
    }

    #[must_use]
    pub fn order(&self) -> &[T] {
        &self.order
    }

    pub fn activate(&mut self, tab: T) -> bool {
        if !self.order.contains(&tab) || self.active == Some(tab) {
            return false;
        }
        self.active = Some(tab);
        true
    }

    pub fn move_to(&mut self, tab: T, slot: usize) -> bool {
        let Some(source) = self.order.iter().position(|candidate| *candidate == tab) else {
            return false;
        };
        let value = self.order.remove(source);
        self.order.insert(slot.min(self.order.len()), value);
        true
    }

    #[must_use]
    pub fn is_pinned(&self, tab: T) -> bool {
        self.pinned.contains(&tab)
    }

    pub fn set_pinned(&mut self, tab: T, pinned: bool) -> bool {
        if !self.order.contains(&tab) || self.is_pinned(tab) == pinned {
            return false;
        }
        if pinned {
            self.pinned.push(tab);
        } else if let Some(index) = self.pinned.iter().position(|candidate| *candidate == tab) {
            self.pinned.remove(index);
        }
        true
    }

    pub fn set_overflow_offset(&mut self, offset: usize) {
        self.overflow_offset = offset.min(self.order.len().saturating_sub(1));
    }

    #[must_use]
    pub fn visible_tabs(&self, capacity: usize) -> Vec<T> {
        if capacity == 0 {
            return Vec::new();
        }
        let mut visible = self
            .order
            .iter()
            .copied()
            .filter(|tab| self.is_pinned(*tab))
            .take(capacity)
            .collect::<Vec<_>>();
        let remaining = capacity.saturating_sub(visible.len());
        visible.extend(
            self.order
                .iter()
                .copied()
                .filter(|tab| !self.is_pinned(*tab))
                .skip(self.overflow_offset)
                .take(remaining),
        );
        visible
    }

    pub fn close(&mut self, tab: T) -> bool {
        let Some(index) = self.order.iter().position(|candidate| *candidate == tab) else {
            return false;
        };
        self.order.remove(index);
        self.pinned.retain(|candidate| *candidate != tab);
        if self.active == Some(tab) {
            self.active = self.order.get(index).or_else(|| self.order.last()).copied();
        }
        true
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PopupDirection {
    Below,
    Right,
}

/// Places a popup inside the screen bounds and flips it when the preferred edge would overflow.
#[must_use]
pub fn place_popup(anchor: Rect, popup: Size, bounds: Rect, direction: PopupDirection) -> Point {
    let (mut x, mut y) = match direction {
        PopupDirection::Below => {
            let below = anchor.bottom();
            let y = if below + popup.height <= bounds.bottom() {
                below
            } else {
                anchor.y - popup.height
            };
            (anchor.x, y)
        }
        PopupDirection::Right => {
            let right = anchor.right();
            let x = if right + popup.width <= bounds.right() {
                right
            } else {
                anchor.x - popup.width
            };
            (x, anchor.y)
        }
    };
    x = x.clamp(bounds.x, (bounds.right() - popup.width).max(bounds.x));
    y = y.clamp(bounds.y, (bounds.bottom() - popup.height).max(bounds.y));
    Point::new(x, y)
}

#[cfg(test)]
mod scroll_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use vesta_gpui::{MenuItem, MenuItemId};

    #[test]
    fn menu_keyboard_step_skips_disabled_and_separator_items() {
        let menu = Menu::new(MenuId(1), "File")
            .item(MenuItem::section_header(MenuItemId(5), "Documents"))
            .item(MenuItem::new(MenuItemId(1), "Open", ()))
            .item(MenuItem::separator(MenuItemId(2)))
            .item(MenuItem::new(MenuItemId(3), "Unavailable", ()).enabled(false))
            .item(MenuItem::new(MenuItemId(4), "Save", ()));
        let mut state = MenuController::default();
        state.open(menu.id);
        state.step(&menu, 1);
        assert_eq!(state.highlighted(), Some(MenuItemId(1)));
        state.step(&menu, 1);
        assert_eq!(state.highlighted(), Some(MenuItemId(4)));
        state.select_edge(&menu, false);
        assert_eq!(state.highlighted(), Some(MenuItemId(1)));
        state.select_edge(&menu, true);
        assert_eq!(state.highlighted(), Some(MenuItemId(4)));

        let reordered = Menu::new(menu.id, "File")
            .item(MenuItem::new(MenuItemId(4), "Renamed", ()).checked(true))
            .item(MenuItem::new(MenuItemId(1), "Open", ()));
        assert!(!state.reconcile(&[reordered]));
        assert_eq!(state.highlighted(), Some(MenuItemId(4)));
        let disabled = Menu::new(menu.id, "File")
            .item(MenuItem::new(MenuItemId(4), "Renamed", ()).enabled(false))
            .item(MenuItem::new(MenuItemId(1), "Open", ()));
        assert!(state.reconcile(std::slice::from_ref(&disabled)));
        assert_eq!(
            state.highlighted(),
            None,
            "replacement cannot select another action"
        );
        state.step(&disabled, 1);
        assert_eq!(state.highlighted(), Some(MenuItemId(1)));
        assert!(state.reconcile(&[Menu::<()>::new(menu.id, "Empty")]));
        assert_eq!(state.highlighted(), None);
        let key = |key| KeyChord::new(key, vesta_gpui::KeyModifiers::default());
        assert_eq!(
            state.key_action(&disabled, key(KeyStroke::Enter)),
            MenuKeyAction::Consumed
        );
        assert_eq!(
            state.key_action(&disabled, key(KeyStroke::ArrowDown)),
            MenuKeyAction::Updated
        );
        assert_eq!(
            state.key_action(&disabled, key(KeyStroke::Enter)),
            MenuKeyAction::Activate(MenuItemId(1))
        );
        state.highlight(Some(MenuItemId(4)));
        assert_eq!(
            state.key_action(&disabled, key(KeyStroke::Enter)),
            MenuKeyAction::Consumed
        );
    }

    #[test]
    fn submenu_keeps_the_opener_focus_until_one_close_consumes_it() {
        let opener = InputTarget {
            window: vesta_gpui::UiWindowId(4),
            element: vesta_gpui::ElementId::new(12),
        };
        let mut state = MenuController::default();
        state.open_for(MenuId(1), Some(opener));
        let parent = Menu::new(MenuId(1), "File")
            .item(MenuItem::new(MenuItemId(10), "Reset", ()))
            .item(MenuItem::new(MenuItemId(11), "Disabled", ()).enabled(false))
            .item(MenuItem::new(MenuItemId(12), "Export", ()).submenu(MenuId(2)));
        assert!(state.highlight_in(&parent, MenuItemId(12)));
        state.enter_submenu(MenuId(2));
        assert_eq!(state.return_focus(), Some(opener));
        assert!(!state.highlight_in(&parent, MenuItemId(12)));
        assert_eq!(state.path(), &[MenuId(1), MenuId(2)]);
        let cyclic_child = Menu::new(MenuId(2), "Export")
            .item(MenuItem::new(MenuItemId(20), "Ancestor", ()).submenu(MenuId(1)));
        state.select_edge(&cyclic_child, false);
        for _ in 0..3 {
            let decision = state.key_action(
                &cyclic_child,
                KeyChord::new(KeyStroke::ArrowRight, Default::default()),
            );
            let MenuKeyAction::OpenSubmenu(target) = decision else {
                panic!("typed submenu decision");
            };
            state.enter_submenu(target);
            assert_eq!(
                state.path(),
                &[MenuId(1), MenuId(2)],
                "reject ancestor cycle"
            );
            // Hosts initialize the requested child's highlight after entry. A
            // refused ancestor descriptor must not replace the active highlight.
            state.select_edge(&parent, false);
            assert_eq!(state.highlighted_in(MenuId(1)), Some(MenuItemId(12)));
            assert_eq!(state.highlighted(), Some(MenuItemId(20)));
            assert_eq!(state.return_focus(), Some(opener));
        }
        assert!(!state.highlight_in(&parent, MenuItemId(11)));
        assert!(state.highlight_in(&parent, MenuItemId(10)));
        assert_eq!(state.path(), &[MenuId(1)]);
        assert_eq!(state.highlighted(), Some(MenuItemId(10)));
        state.highlight(Some(MenuItemId(12)));
        state.enter_submenu(MenuId(2));
        let child = Menu::new(MenuId(2), "Export").item(MenuItem::new(MenuItemId(20), "Image", ()));
        state.select_edge(&child, false);
        assert!(!state.reconcile(&[parent.clone(), child.clone()]));
        let mut retargeted = parent.clone();
        retargeted.items[2].submenu = Some(MenuId(3));
        assert!(state.reconcile(&[retargeted, child]));
        assert_eq!(state.path(), &[MenuId(1)]);
        assert_eq!(state.highlighted(), Some(MenuItemId(12)));
        assert_eq!(state.return_focus(), Some(opener));
        state.enter_submenu(MenuId(2));
        assert!(state.reconcile(&[parent]));
        assert_eq!(state.path(), &[MenuId(1)]);
        assert!(state.reconcile::<()>(&[]));
        assert!(!state.is_open());
        assert_eq!(state.return_focus(), Some(opener));
        assert_eq!(state.close(), Some(opener));
        assert_eq!(state.close(), None);

        // A context menu without an opener still has a source window. A foreign
        // close cannot consume it; a reconciled-away root still closes once.
        state.open_in(opener.window, MenuId(1), None);
        assert!(
            state
                .dismiss(UiWindowId(999), PopupCloseReason::Explicit)
                .is_none()
        );
        assert!(state.is_open());
        assert!(state.reconcile::<()>(&[]));
        assert_eq!(
            state
                .dismiss(opener.window, PopupCloseReason::OwnerRemoved)
                .expect("own vanished menu")
                .opener,
            None
        );
        assert!(
            state
                .dismiss(opener.window, PopupCloseReason::Explicit)
                .is_none()
        );
        state.open_in(UiWindowId(999), MenuId(1), Some(opener));
        assert_eq!(
            state.return_focus(),
            None,
            "foreign opener must not be retained"
        );
        assert!(
            state
                .dismiss(opener.window, PopupCloseReason::Explicit)
                .is_none()
        );
        assert!(
            state
                .dismiss(UiWindowId(999), PopupCloseReason::OutsidePress)
                .is_some()
        );
    }
}
