//! Synthetic Gallery ramp values, transactions and projection (ADR-0257).

use super::*;

const HISTORY_LIMIT: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct RampControlPoint {
    pub id: u32,
    pub position: f32,
    pub rgba: [u8; 4],
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct RampSnapshot {
    pub points: Vec<RampControlPoint>,
    pub primary: u32,
    pub selection: ListSelectionState<u32>,
    pub interpolation: RampInterpolation,
}

pub(super) struct RampDemo {
    pub points: Vec<RampControlPoint>,
    pub primary: u32,
    pub selection: ListSelectionState<u32>,
    pub interpolation: RampInterpolation,
    pub undo: Vec<RampSnapshot>,
    pub redo: Vec<RampSnapshot>,
    next_id: u32,
}

impl Default for RampDemo {
    fn default() -> Self {
        Self {
            points: vec![
                RampControlPoint {
                    id: 1,
                    position: 0.0,
                    rgba: [103, 157, 221, 255],
                },
                RampControlPoint {
                    id: 2,
                    position: 0.5,
                    rgba: [135, 194, 140, 255],
                },
                RampControlPoint {
                    id: 3,
                    position: 1.0,
                    rgba: [222, 126, 110, 255],
                },
            ],
            primary: 2,
            selection: ListSelectionState::single(2),
            interpolation: RampInterpolation::Linear,
            undo: Vec::new(),
            redo: Vec::new(),
            next_id: 4,
        }
    }
}

impl RampDemo {
    pub fn point(&self, id: u32) -> Option<&RampControlPoint> {
        self.points.iter().find(|point| point.id == id)
    }

    pub fn snapshot(&self) -> RampSnapshot {
        RampSnapshot {
            points: self.points.clone(),
            primary: self.primary,
            selection: self.selection.clone(),
            interpolation: self.interpolation,
        }
    }

    pub fn restore(&mut self, snapshot: RampSnapshot) {
        self.points = snapshot.points;
        self.primary = snapshot.primary;
        self.selection = snapshot.selection;
        self.interpolation = snapshot.interpolation;
    }

    /// Selection is view state; only changed ramp values create a transaction.
    pub fn commit_snapshot(&mut self, before: RampSnapshot) -> bool {
        if self.points == before.points && self.interpolation == before.interpolation {
            return false;
        }
        push_bounded(&mut self.undo, before);
        self.redo.clear();
        true
    }

    pub fn select(&mut self, id: u32) {
        if self.point(id).is_some() {
            self.primary = id;
            self.selection = ListSelectionState::single(id);
        }
    }

    pub fn ordered_points(&self) -> Vec<&RampControlPoint> {
        let mut points = self.points.iter().collect::<Vec<_>>();
        points.sort_by(|left, right| {
            left.position
                .total_cmp(&right.position)
                .then_with(|| left.id.cmp(&right.id))
        });
        points
    }

    pub fn select_modified(&mut self, id: u32, modifiers: SelectionModifiers) {
        if self.point(id).is_none() {
            return;
        }
        let order = self
            .ordered_points()
            .iter()
            .map(|point| point.id)
            .collect::<Vec<_>>();
        self.selection.select(id, &order, modifiers);
        self.primary = if self.selection.contains(&id) {
            id
        } else {
            self.selection.selected_ids().next().copied().unwrap_or(id)
        };
    }

    pub fn add(&mut self) {
        let Some(next_id) = self.next_id.checked_add(1) else {
            return;
        };
        let before = self.snapshot();
        let rgba = self
            .point(self.primary)
            .map_or([255; 4], |point| point.rgba);
        self.points.push(RampControlPoint {
            id: self.next_id,
            position: 0.5,
            rgba,
        });
        self.primary = self.next_id;
        self.selection = ListSelectionState::single(self.primary);
        self.next_id = next_id;
        self.commit_snapshot(before);
    }

    pub fn step(&mut self, direction: i8) {
        let before = self.snapshot();
        apply_ramp_delta(&mut self.points, &before, f32::from(direction) * 0.05);
        self.commit_snapshot(before);
    }

    pub fn set_position(&mut self, id: u32, position: f32) -> bool {
        let Some(point) = self.point(id) else {
            return false;
        };
        if !position.is_finite() {
            return false;
        }
        let position = position.clamp(0.0, 1.0);
        if position == point.position {
            return false;
        }
        let before = self.snapshot();
        if let Some(point) = self.points.iter_mut().find(|point| point.id == id) {
            point.position = position;
        }
        self.commit_snapshot(before)
    }

    pub fn remove(&mut self) {
        if !self.can_remove() {
            return;
        }
        let before = self.snapshot();
        self.points
            .retain(|point| !self.selection.contains(&point.id));
        self.select(self.points[0].id);
        self.commit_snapshot(before);
    }

    pub fn can_remove(&self) -> bool {
        let count = self
            .points
            .iter()
            .filter(|point| self.selection.contains(&point.id))
            .count();
        count > 0 && self.points.len().saturating_sub(count) >= 2
    }

    pub fn set_color(&mut self, id: u32, rgba: [u8; 4]) -> bool {
        let Some(point) = self.point(id) else {
            return false;
        };
        if point.rgba == rgba {
            return false;
        }
        let before = self.snapshot();
        if let Some(point) = self.points.iter_mut().find(|point| point.id == id) {
            point.rgba = rgba;
        }
        self.commit_snapshot(before)
    }

    pub fn cycle_interpolation(&mut self) {
        let before = self.snapshot();
        self.interpolation = self.interpolation.next();
        self.commit_snapshot(before);
    }

    pub fn undo(&mut self) {
        if let Some(previous) = self.undo.pop() {
            let current = self.snapshot();
            push_bounded(&mut self.redo, current);
            self.restore(previous);
        }
    }

    pub fn redo(&mut self) {
        if let Some(next) = self.redo.pop() {
            let current = self.snapshot();
            push_bounded(&mut self.undo, current);
            self.restore(next);
        }
    }
}

fn push_bounded(history: &mut Vec<RampSnapshot>, snapshot: RampSnapshot) {
    if history.len() == HISTORY_LIMIT {
        history.remove(0);
    }
    history.push(snapshot);
}

/// One clamped translation preserves group spacing and derives every update from its origin.
fn apply_ramp_delta(points: &mut [RampControlPoint], start: &RampSnapshot, requested: f32) -> bool {
    let initial = start
        .points
        .iter()
        .map(|point| (point.id, point.position))
        .collect::<Vec<_>>();
    let selected = start.selection.selected_ids().copied().collect::<Vec<_>>();
    let live = points.iter().map(|point| point.id).collect::<Vec<_>>();
    let Some(edits) =
        vesta_gpui_widgets::ramp_group_translation(&initial, &selected, &live, requested)
    else {
        return false;
    };
    let mut changed = false;
    for (id, position) in edits {
        if let Some(point) = points.iter_mut().find(|point| point.id == id)
            && (point.position - position).abs() > f32::EPSILON
        {
            changed = true;
            point.position = position;
        }
    }
    changed
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum ColorEditTarget {
    #[default]
    Swatch,
    RampPoint(u32),
}

/// Window-owned capture session; values and history remain shared by all windows.
#[derive(Clone, Debug)]
pub(super) struct RampDrag {
    pub id: u32,
    pub session: ValueEditSession<RampSnapshot>,
    pub grab_offset: f32,
    pub moved: bool,
}

pub(super) fn ramp_point_action(
    actions: &ActionRoute<DemoAction>,
    hit: ElementId,
) -> Option<(ElementId, u32)> {
    match actions.target_for_hit(hit).map(|(_, action)| *action) {
        Some(DemoAction::RampSelect(id)) => Some((hit, id)),
        _ => None,
    }
}

pub(super) fn ramp_key_action(
    ramp: &RampDemo,
    actions: &ActionRoute<DemoAction>,
    element: ElementId,
    key: &Key,
) -> Option<DemoAction> {
    let (_, id) = ramp_point_action(actions, element)?;
    let key = match key {
        Key::Named(NamedKey::ArrowLeft | NamedKey::ArrowDown) => SliderKey::Decrease,
        Key::Named(NamedKey::ArrowRight | NamedKey::ArrowUp) => SliderKey::Increase,
        Key::Named(NamedKey::PageDown) => SliderKey::PageDecrease,
        Key::Named(NamedKey::PageUp) => SliderKey::PageIncrease,
        Key::Named(NamedKey::Home) => SliderKey::Start,
        Key::Named(NamedKey::End) => SliderKey::End,
        _ => return None,
    };
    let position = slider_value_for_key(ramp.point(id)?.position, key, 0.05);
    Some(DemoAction::RampSetPosition(id, position))
}

pub(super) fn ramp_accessibility_action(
    actions: &ActionRoute<DemoAction>,
    element: ElementId,
    data: Option<&AccessibilityActionData>,
) -> Option<DemoAction> {
    let (_, id) = ramp_point_action(actions, element)?;
    let AccessibilityActionData::NumericValue(value) = data? else {
        return None;
    };
    value.is_finite().then_some(DemoAction::RampSetPosition(
        id,
        value.clamp(0.0, 1.0) as f32,
    ))
}

pub(super) fn begin_ramp_drag_at(
    ramp: &RampDemo,
    owner: InputTarget,
    track: Rect,
    id: u32,
    pointer_x: f32,
) -> Option<RampDrag> {
    let position = ramp.point(id)?.position;
    let mut start = ramp.snapshot();
    start.primary = id;
    Some(RampDrag {
        id,
        session: ValueEditSession::new(owner, start),
        grab_offset: pointer_x - ramp_handle_x(track, position),
        moved: false,
    })
}

pub(super) fn press_ramp_point(
    ramp: &mut RampDemo,
    owner: InputTarget,
    track: Rect,
    id: u32,
    pointer_x: f32,
    modifiers: SelectionModifiers,
) -> Option<RampDrag> {
    ramp.point(id)?;
    if modifiers.additive || modifiers.range {
        ramp.select_modified(id, modifiers);
        return None;
    }
    if !ramp.selection.contains(&id) {
        ramp.select(id);
    }
    ramp.primary = id;
    begin_ramp_drag_at(ramp, owner, track, id, pointer_x)
}

pub(super) fn update_ramp_drag_at(
    drag: &mut RampDrag,
    source: UiWindowId,
    track: Rect,
    pointer_x: f32,
    ramp: &mut RampDemo,
) -> bool {
    if !drag.session.accepts(source) || !pointer_x.is_finite() {
        return false;
    }
    let position = ramp_position_for_pointer(track, pointer_x, drag.grab_offset);
    let snapshot = drag.session.initial();
    let Some(start) = snapshot.points.iter().find(|point| point.id == drag.id) else {
        return false;
    };
    let changed = apply_ramp_delta(&mut ramp.points, snapshot, position - start.position);
    drag.moved |= changed;
    changed
}

impl App {
    /// A new value transaction cannot leave an older capture able to restore stale values.
    pub(super) fn prepare_ramp_action(&mut self, action: DemoAction) {
        let changes_ramp = matches!(
            action,
            DemoAction::RampAdd
                | DemoAction::RampSetPosition(..)
                | DemoAction::RampStep(_)
                | DemoAction::RampRemove
                | DemoAction::RampUndo
                | DemoAction::RampRedo
                | DemoAction::RampCycleInterpolation
                | DemoAction::RampEditColor
        ) || (action == DemoAction::ApplyColor
            && matches!(self.demo.color_target, ColorEditTarget::RampPoint(_)));
        if !changes_ramp {
            return;
        }
        self.cancel_ramp_drag();
        let windows = self
            .native_windows
            .iter()
            .filter_map(|(id, host)| host.ramp_drag.is_some().then_some(*id))
            .collect::<Vec<_>>();
        for window in windows {
            self.cancel_native_ramp_drag(window);
        }
    }

    pub(super) fn ramp_point_at(&self, hit: ElementId) -> Option<u32> {
        ramp_point_action(&self.ui_actions, hit).map(|(_, id)| id)
    }

    pub(super) fn begin_ramp_drag(&mut self, hit: ElementId) -> bool {
        let Some(id) = self.ramp_point_at(hit) else {
            return false;
        };
        let Some(track) = self.ramp_track_rect else {
            return false;
        };
        let modifiers = self.text_bridge.modifiers();
        let drag = press_ramp_point(
            &mut self.demo.ramp,
            InputTarget {
                window: STANDALONE_WINDOW,
                element: hit,
            },
            track,
            id,
            self.cursor.x,
            SelectionModifiers {
                additive: modifiers.control_key(),
                range: modifiers.shift_key(),
            },
        );
        self.close_menu(false);
        self.set_text_focus(None);
        self.set_focus_target(Some(hit));
        if drag.is_some() {
            self.input_ownership.capture_pointer(
                InputTarget {
                    window: STANDALONE_WINDOW,
                    element: hit,
                },
                PointerButton::Primary,
            );
        }
        self.ramp_drag = drag;
        true
    }

    pub(super) fn update_ramp_drag(&mut self) -> bool {
        let Some(track) = self.ramp_track_rect else {
            return false;
        };
        let pointer_x = self.cursor.x;
        let Some(drag) = self.ramp_drag.as_mut() else {
            return false;
        };
        update_ramp_drag_at(
            drag,
            STANDALONE_WINDOW,
            track,
            pointer_x,
            &mut self.demo.ramp,
        )
    }

    /// Commits the drag: a moved point contributes exactly one history entry, and a press
    /// without movement leaves the history untouched.
    pub(super) fn finish_ramp_drag(&mut self) -> bool {
        let Some(mut drag) = self.ramp_drag.take() else {
            return false;
        };
        if !drag.moved {
            return false;
        }
        if let Some(PointerEditOutcome::Commit { before, .. }) =
            drag.session
                .finish(STANDALONE_WINDOW, &self.demo.ramp.snapshot(), true)
        {
            self.demo.ramp.commit_snapshot(before)
        } else {
            false
        }
    }

    pub(super) fn cancel_ramp_drag(&mut self) -> bool {
        let Some(mut drag) = self.ramp_drag.take() else {
            return false;
        };
        if let Some(PointerEditOutcome::Cancel { restore }) =
            drag.session
                .finish(STANDALONE_WINDOW, &self.demo.ramp.snapshot(), false)
        {
            self.demo.ramp.restore(restore);
        }
        self.input_ownership.cancel_capture();
        drag.moved
    }

    /// Cancels a ramp drag owned by one native floating window and restores the snapshot
    /// taken when that window's pointer went down.
    pub(super) fn cancel_native_ramp_drag(&mut self, window_id: WindowId) -> bool {
        let Some(host) = self.native_windows.get_mut(&window_id) else {
            return false;
        };
        let Some(mut drag) = host.ramp_drag.take() else {
            return false;
        };
        host.input.cancel_capture();
        if let Some(PointerEditOutcome::Cancel { restore }) =
            drag.session
                .finish(host.ui_window, &self.demo.ramp.snapshot(), false)
        {
            self.demo.ramp.restore(restore);
        }
        drag.moved
    }
}

pub(super) fn build_ramp(
    input: &UiBuildState<'_>,
    ids: &mut Ids,
    actions: &mut Vec<(ElementId, DemoAction)>,
) -> (Element, Element, Element, ElementId) {
    let scope = input.scopes.gallery;
    let demo = input.demo;
    let theme = input.theme;
    let ramp_identity = |slot, part: &str| {
        WidgetIdentity::new(
            scope.element(RAMP_ROOT + slot),
            format!("gallery.ramp.{part}"),
        )
    };
    let ramp_track = ramp_identity(1, "track").element;
    let ramp_points = demo
        .ramp
        .ordered_points()
        .into_iter()
        .map(|point| RampPoint {
            id: point.id,
            root: ramp_identity(100 + point.id * 2, &format!("point.{}", point.id)),
            label: ramp_identity(101 + point.id * 2, &format!("point.{}.label", point.id)),
            position: point.position,
            color: Color::srgba(
                f32::from(point.rgba[0]) / 255.0,
                f32::from(point.rgba[1]) / 255.0,
                f32::from(point.rgba[2]) / 255.0,
                f32::from(point.rgba[3]) / 255.0,
            ),
            selected: demo.ramp.selection.contains(&point.id),
            select_action: DemoAction::RampSelect(point.id),
        })
        .collect();
    let mut ramp = widget_element(
        ramp_editor(
            RampIds {
                root: ramp_identity(0, "root"),
                track: ramp_identity(1, "track"),
                band: input.scopes.ramp_band,
            },
            ramp_points,
            RampSpec::new(
                demo.ramp.interpolation,
                ((input.viewport_size.width - 100.0) / 2.0).clamp(120.0, 310.0),
            ),
            theme,
        )
        .expect("static gallery ramp"),
        actions,
    )
    .tooltip("Ctrl: toggle point; Shift: select range; drag selected points together");
    let mut ramp_controls = Element::container(ids.id(), ids.key("ramp.controls"))
        .styled(Style::default().axis(Axis::Row).gap(4.0));
    for (caption, action) in [
        ("Add", DemoAction::RampAdd),
        ("Left", DemoAction::RampStep(-1)),
        ("Right", DemoAction::RampStep(1)),
    ] {
        ramp_controls = ramp_controls.child(widget_element(
            button(
                ids.identity("ramp.control"),
                ids.identity("ramp.control.label"),
                caption,
                action,
                ControlState::Idle,
                theme,
            ),
            actions,
        ));
    }
    let mut ramp_history = Element::container(ids.id(), ids.key("ramp.history"))
        .styled(Style::default().axis(Axis::Row).gap(4.0));
    // The host owns the mode and projects its label; the widget only renders the band.
    ramp_history = ramp_history.child(widget_element(
        button(
            ids.identity("ramp.interpolation"),
            ids.identity("ramp.interpolation.label"),
            format!("Interp: {}", demo.ramp.interpolation.label()),
            DemoAction::RampCycleInterpolation,
            ControlState::Idle,
            theme,
        ),
        actions,
    ));
    for (caption, action, enabled) in [
        ("Remove", DemoAction::RampRemove, demo.ramp.can_remove()),
        ("Undo", DemoAction::RampUndo, !demo.ramp.undo.is_empty()),
        ("Redo", DemoAction::RampRedo, !demo.ramp.redo.is_empty()),
    ] {
        ramp_history = ramp_history.child(widget_element(
            button(
                ids.identity("ramp.history.control"),
                ids.identity("ramp.history.label"),
                caption,
                action,
                if enabled {
                    ControlState::Idle
                } else {
                    ControlState::Disabled
                },
                theme,
            ),
            actions,
        ));
    }
    let edit_color = widget_element(
        button(
            ramp_identity(2, "edit-color"),
            ramp_identity(3, "edit-color.label"),
            "Edit point color",
            DemoAction::RampEditColor,
            if demo.ramp.selection.contains(&demo.ramp.primary) {
                ControlState::Idle
            } else {
                ControlState::Disabled
            },
            theme,
        ),
        actions,
    );
    let mut body = Element::container(
        ramp_identity(4, "body").element,
        ramp_identity(4, "body").key,
    )
    .styled(Style::default().axis(Axis::Column).gap(4.0))
    .child(ramp)
    .child(edit_color);
    if demo.color_open
        && let ColorEditTarget::RampPoint(id) = demo.color_target
    {
        body = body
            .child(
                Element::text(
                    ramp_identity(5, "color-target").element,
                    ramp_identity(5, "color-target").key,
                    format!("Point {id} color"),
                )
                .styled(
                    Style::default().text(theme.typography.font_size, theme.palette.text_muted),
                ),
            )
            .child(build_color_picker(input, demo.color_target, actions));
    }
    ramp = body;
    (ramp, ramp_controls, ramp_history, ramp_track)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{activate_gallery_page, install_action_route_with_viewport};

    /// ADR-0177 decision 3 follow-up: a ramp handle can be dragged with the pointer, and the
    /// host maps movement through the shared widget geometry. One drag commits exactly one
    /// history entry, a press without movement commits nothing, and a cancel restores the
    /// pre-drag ramp.
    #[test]
    fn ramp_drag_follows_pointer_geometry_and_commits_one_history_entry() {
        use vesta_gpui_widgets::ramp_travel;

        let mut app = App::new();
        activate_gallery_page(&mut app, "gallery.inputs");
        let viewport = Viewport::new(1280.0, 2400.0, 1.0);
        let root = install_action_route_with_viewport(&mut app, Size::new(1280.0, 2400.0));
        let layout = LayoutEngine
            .compute(&root, viewport)
            .expect("gallery layout");
        app.hit_test = HitTestTree::build(&root, &layout);
        let track = layout
            .node(app.scopes.gallery.element(RAMP_ROOT + 1))
            .map(|node| node.rect)
            .expect("ramp track rect");
        app.ramp_track_rect = Some(track);

        let id = 2;
        let handle = layout
            .node(app.scopes.gallery.element(RAMP_ROOT + 100 + id * 2))
            .expect("ramp handle rect")
            .rect;
        let grab = Point::new(
            handle.x + handle.width * 0.5,
            handle.y + handle.height * 0.5,
        );
        app.cursor = grab;
        app.pointer_press();
        assert!(app.ramp_drag.is_some(), "press on a handle begins a drag");
        assert_eq!(app.demo.ramp.primary, id);
        assert!(
            app.input_ownership.capture().is_some(),
            "the handle drag captures the pointer"
        );
        assert!(
            !app.finish_ramp_drag(),
            "a press without movement commits nothing"
        );
        assert!(app.demo.ramp.undo.is_empty());

        let position = |app: &App| {
            app.demo
                .ramp
                .points
                .iter()
                .find(|point| point.id == id)
                .map(|point| point.position)
                .expect("ramp point")
        };
        let travel = ramp_travel(track.width);
        assert!(travel > 0.0);

        app.pointer_press();
        app.cursor = Point::new(grab.x + 40.0, grab.y);
        assert!(app.update_ramp_drag());
        assert!((position(&app) - (0.5 + 40.0 / travel)).abs() < 1e-4);
        app.cursor = Point::new(track.x + track.width + 500.0, grab.y);
        assert!(app.update_ramp_drag());
        assert_eq!(position(&app), 1.0, "dragging past the end clamps");
        app.cursor = Point::new(track.x - 500.0, grab.y);
        assert!(app.update_ramp_drag());
        assert_eq!(position(&app), 0.0, "dragging before the start clamps");
        assert!(app.finish_ramp_drag());
        assert_eq!(app.demo.ramp.undo.len(), 1, "one drag, one history entry");
        assert_eq!(
            app.demo.ramp.undo[0]
                .points
                .iter()
                .find(|point| point.id == id)
                .map(|point| point.position),
            Some(0.5)
        );

        app.apply_demo_action(DemoAction::RampUndo);
        assert!((position(&app) - 0.5).abs() < f32::EPSILON);

        app.cursor = grab;
        app.pointer_press();
        app.cursor = Point::new(grab.x + 30.0, grab.y);
        assert!(app.update_ramp_drag());
        assert!(
            app.cancel_ramp_drag(),
            "cancel reports the discarded movement"
        );
        assert!((position(&app) - 0.5).abs() < f32::EPSILON);
        assert!(app.demo.ramp.undo.is_empty());
        assert!(app.input_ownership.capture().is_none());

        let redo_count = app.demo.ramp.redo.len();
        app.cursor = grab;
        app.pointer_press();
        app.cursor = Point::new(grab.x + 30.0, grab.y);
        assert!(app.update_ramp_drag());
        app.cursor = grab;
        assert!(app.update_ramp_drag());
        assert!(
            !app.finish_ramp_drag(),
            "a drag returning to its start is not a transaction"
        );
        assert!(app.demo.ramp.undo.is_empty());
        assert_eq!(app.demo.ramp.redo.len(), redo_count);

        app.pointer_press();
        app.cursor = Point::new(grab.x + 30.0, grab.y);
        assert!(app.update_ramp_drag());
        app.apply_demo_action(DemoAction::RampCycleInterpolation);
        assert!(
            app.ramp_drag.is_none(),
            "a new transaction first cancels the old capture"
        );
        assert!((position(&app) - 0.5).abs() < f32::EPSILON);
        assert_eq!(app.demo.ramp.interpolation, RampInterpolation::Smooth);
        assert!(
            !app.cancel_ramp_drag(),
            "the old drag cannot roll back the new mode"
        );
        app.apply_demo_action(DemoAction::RampUndo);
        assert_eq!(app.demo.ramp.interpolation, RampInterpolation::Linear);

        let mut ramp = RampDemo::default();
        ramp.points[0].position = 0.2;
        assert!(
            press_ramp_point(
                &mut ramp,
                InputTarget {
                    window: STANDALONE_WINDOW,
                    element: ElementId::new(1)
                },
                track,
                1,
                track.x,
                SelectionModifiers {
                    additive: true,
                    range: false
                }
            )
            .is_none()
        );
        assert_eq!(
            ramp.selection.selected_ids().copied().collect::<Vec<_>>(),
            [1, 2]
        );
        assert!(ramp.undo.is_empty(), "modifier selection is view state");
        let pointer = ramp_handle_x(track, 0.2) + 4.0;
        let mut drag = press_ramp_point(
            &mut ramp,
            InputTarget {
                window: STANDALONE_WINDOW,
                element: ElementId::new(1),
            },
            track,
            1,
            pointer,
            SelectionModifiers::default(),
        )
        .expect("selected group drag");
        let before_group = drag.session.initial().clone();
        assert!(
            !update_ramp_drag_at(&mut drag, UiWindowId(999), track, pointer + 50.0, &mut ramp),
            "foreign window cannot advance the shared ramp preview"
        );
        assert!(update_ramp_drag_at(
            &mut drag,
            STANDALONE_WINDOW,
            track,
            pointer + ramp_travel(track.width),
            &mut ramp
        ));
        assert!((ramp.point(1).expect("first").position - 0.7).abs() < 1e-6);
        assert_eq!(ramp.point(2).expect("second").position, 1.0);
        assert_eq!(ramp.point(3).expect("unselected").position, 1.0);
        assert!(
            (ramp.point(2).expect("second").position
                - ramp.point(1).expect("first").position
                - 0.3)
                .abs()
                < 1e-6,
            "group clamping preserves spacing"
        );
        let Some(PointerEditOutcome::Commit { before, .. }) =
            drag.session
                .finish(STANDALONE_WINDOW, &ramp.snapshot(), true)
        else {
            panic!("moved ramp must commit once");
        };
        assert!(ramp.commit_snapshot(before));
        assert_eq!(ramp.undo.len(), 1);
        ramp.undo();
        assert_eq!(ramp.snapshot(), before_group);
        assert_eq!(
            ramp.selection.selected_ids().copied().collect::<Vec<_>>(),
            [1, 2]
        );
        ramp.select_modified(
            3,
            SelectionModifiers {
                additive: false,
                range: true,
            },
        );
        assert_eq!(
            ramp.selection.selected_ids().copied().collect::<Vec<_>>(),
            [1, 2, 3]
        );
        let before_rejected = ramp.snapshot();
        ramp.remove();
        assert_eq!(
            ramp.snapshot(),
            before_rejected,
            "group deletion cannot leave fewer than two points"
        );
        assert!(ramp.undo.is_empty());
        ramp.add();
        let added = ramp.primary;
        ramp.select_modified(
            2,
            SelectionModifiers {
                additive: true,
                range: false,
            },
        );
        let before_delete = ramp.snapshot();
        ramp.remove();
        assert!(ramp.point(added).is_none() && ramp.point(2).is_none());
        assert_eq!(ramp.points.len(), 2);
        ramp.undo();
        assert_eq!(
            ramp.snapshot(),
            before_delete,
            "Undo restores all deleted values and the selection anchor"
        );
    }
}
