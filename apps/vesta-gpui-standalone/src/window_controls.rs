//! Shared scalar keyboard policy with window-owned focus and geometry (ADR-0263).

use super::*;

const TABLE_COLUMN_MIN_WIDTH: f32 = 36.0;

fn cancel_scrollbar_drag(
    window: UiWindowId,
    session: &mut Option<(ElementId, ScrollbarDrag)>,
    input: &mut InputOwnership,
) {
    let Some((element, _)) = session.take() else {
        return;
    };
    if input.capture().is_some_and(|capture| {
        capture.target.window == window
            && capture.target.element == element
            && capture.button == PointerButton::Primary
    }) {
        input.cancel_capture();
    }
}

/// Responsive tracks are derived from retained proportions; layout never writes them back.
pub(super) fn table_column_widths(fractions: &[f32; 3], width: f32) -> Option<[f32; 3]> {
    if !width.is_finite()
        || width < TABLE_COLUMN_MIN_WIDTH * 3.0
        || fractions
            .iter()
            .any(|value| !value.is_finite() || !(0.0..=1.0).contains(value))
        || (fractions.iter().map(|value| f64::from(*value)).sum::<f64>() - 1.0).abs()
            > f64::from(f32::EPSILON) * 4.0
    {
        return None;
    }
    let original = fractions.map(|fraction| fraction * width);
    if original
        .iter()
        .all(|value| value.is_finite() && *value >= TABLE_COLUMN_MIN_WIDTH)
    {
        return Some(original);
    }
    let mut fixed = [false; 3];
    let mut widths = [0.0; 3];
    for _ in 0..3 {
        let remaining = f64::from(width)
            - f64::from(TABLE_COLUMN_MIN_WIDTH)
                * fixed.iter().filter(|fixed| **fixed).count() as f64;
        let weight = fractions
            .iter()
            .enumerate()
            .filter(|(index, _)| !fixed[*index])
            .map(|(_, fraction)| f64::from(*fraction))
            .sum::<f64>();
        if weight <= 0.0 {
            return None;
        }
        let candidates =
            fractions.map(|fraction| (remaining * f64::from(fraction) / weight) as f32);
        let mut constrained = false;
        for index in 0..3 {
            if !fixed[index] && candidates[index] < TABLE_COLUMN_MIN_WIDTH {
                fixed[index] = true;
                widths[index] = TABLE_COLUMN_MIN_WIDTH;
                constrained = true;
            }
        }
        if !constrained {
            for index in 0..3 {
                if !fixed[index] {
                    widths[index] = candidates[index];
                }
            }
            return Some(widths);
        }
    }
    None
}

#[derive(Clone, Copy, Debug)]
pub(super) struct TableResizeDrag {
    pub(super) column: usize,
    pub(super) session: HorizontalEditSession<[f32; 3]>,
}

pub(super) fn cancel_table_resize_session(
    window: UiWindowId,
    drag: &mut Option<TableResizeDrag>,
    input: &mut InputOwnership,
    fractions: &mut [f32; 3],
) -> bool {
    let Some(mut current) = drag
        .as_ref()
        .copied()
        .filter(|drag| drag.session.target().window == window)
    else {
        return false;
    };
    *drag = None;
    let owner = current.session.target();
    if let Some(PointerEditOutcome::Cancel { restore }) =
        current.session.finish(window, fractions, false)
    {
        *fractions = restore;
    }
    if input
        .capture()
        .is_some_and(|capture| capture.target == owner && capture.button == PointerButton::Primary)
    {
        input.cancel_capture();
    }
    true
}

/// Column resize bounds for one table. Shared by every window that shows the table, using
/// that window's own table rect.
pub(super) fn table_resize_width_bounds(
    fractions: &[f32; 3],
    table_width: f32,
    column: usize,
) -> Option<(f32, f32)> {
    if column >= 2 {
        return None;
    }
    let widths = table_column_widths(fractions, table_width)?;
    let pair_width = widths[column] + widths[column + 1];
    pair_width.is_finite().then_some((
        TABLE_COLUMN_MIN_WIDTH,
        (pair_width - TABLE_COLUMN_MIN_WIDTH).max(TABLE_COLUMN_MIN_WIDTH),
    ))
}

/// Applies one pixel width to a table column, returning the next fractions when it changed.
pub(super) fn table_column_fractions_after(
    fractions: &[f32; 3],
    table_width: f32,
    column: usize,
    width: f32,
) -> Option<[f32; 3]> {
    let (min, max) = table_resize_width_bounds(fractions, table_width, column)?;
    if !width.is_finite() || !(min..=max).contains(&width) {
        return None;
    }
    let widths = table_column_widths(fractions, table_width)?;
    let fraction = width / table_width;
    if (fraction - widths[column] / table_width).abs() <= f32::EPSILON {
        return None;
    }
    let projected = widths.map(|width| width / table_width);
    let mut next = if projected
        .iter()
        .zip(fractions)
        .all(|(projected, original)| (*projected - original).abs() <= f32::EPSILON)
    {
        *fractions
    } else {
        projected
    };
    let pair = next[column] + next[column + 1];
    next[column] = fraction;
    next[column + 1] = pair - fraction;
    next.iter()
        .all(|value| value.is_finite() && *value > 0.0)
        .then_some(next)
}

impl App {
    pub(super) fn table_resize_spec(&self, column: usize) -> Option<TableColumnResize> {
        window_controls::table_resize_spec_for_geometry(
            self.scopes.gallery,
            self.table_rect?,
            &self.demo.table_column_fractions,
            column,
        )
    }

    pub(super) fn set_table_column_width(&mut self, column: usize, width: f32) -> bool {
        let Some(table_width) = self.table_rect.map(|rect| rect.width.max(1.0)) else {
            return false;
        };
        let Some(next) = table_column_fractions_after(
            &self.demo.table_column_fractions,
            table_width,
            column,
            width,
        ) else {
            return false;
        };
        self.demo.table_column_fractions = next;
        true
    }

    pub(super) fn begin_table_resize(&mut self, column: usize) -> bool {
        if self.table_resize_spec(column).is_none() {
            return false;
        }
        let element = table_resize_element(self.scopes.gallery, column);
        self.set_text_focus(None);
        self.set_focus_target(Some(element));
        self.input_ownership.capture_pointer(
            InputTarget {
                window: STANDALONE_WINDOW,
                element,
            },
            PointerButton::Primary,
        );
        self.table_resize = HorizontalEditSession::new(
            InputTarget {
                window: STANDALONE_WINDOW,
                element,
            },
            self.cursor.x,
            self.demo.table_column_fractions,
            0.0,
        )
        .ok()
        .map(|session| TableResizeDrag { column, session });
        self.table_resize.is_some()
    }

    pub(super) fn update_table_resize(&mut self) -> bool {
        let Some(mut drag) = self.table_resize else {
            return false;
        };
        let Some(spec) = self.table_resize_spec(drag.column) else {
            return false;
        };
        let Some(table_width) = self.table_rect.map(|rect| rect.width.max(1.0)) else {
            return false;
        };
        let Some(delta) = drag.session.offset(STANDALONE_WINDOW, self.cursor.x) else {
            return false;
        };
        self.table_resize = Some(drag);
        let Some(initial_widths) = table_column_widths(drag.session.initial(), table_width) else {
            return false;
        };
        let start_width = initial_widths[drag.column];
        let width = table_column_width_at(start_width, delta, &spec);
        self.set_table_column_width(drag.column, width)
    }

    pub(super) fn finish_table_resize(&mut self) -> bool {
        self.table_resize.take().is_some_and(|mut drag| {
            drag.session
                .finish(STANDALONE_WINDOW, &self.demo.table_column_fractions, true)
                .is_some()
        })
    }

    pub(super) fn cancel_table_resize(&mut self) -> bool {
        cancel_table_resize_session(
            STANDALONE_WINDOW,
            &mut self.table_resize,
            &mut self.input_ownership,
            &mut self.demo.table_column_fractions,
        )
    }

    pub(super) fn cancel_table_resize_for(&mut self, window: UiWindowId) -> bool {
        if window == STANDALONE_WINDOW {
            return self.cancel_table_resize();
        }
        let Some(host) = self
            .native_windows
            .values_mut()
            .find(|host| host.ui_window == window)
        else {
            return false;
        };
        cancel_table_resize_session(
            window,
            &mut host.table_resize,
            &mut host.input,
            &mut self.demo.table_column_fractions,
        )
    }

    pub(super) fn cancel_removed_value_draft(
        &mut self,
        window: UiWindowId,
        reason: PopupCloseReason,
    ) {
        if matches!(
            reason,
            PopupCloseReason::OwnerRemoved | PopupCloseReason::WindowUnavailable
        ) {
            // Restore the accepted value before ordinary field exit can publish
            // a draft whose source is no longer a live editing target.
            match self.text_session_field(window) {
                Some(TextFieldKind::Number) => self.reset_number_draft(),
                Some(TextFieldKind::Vector(axis)) => self.reset_vector_draft(axis),
                Some(TextFieldKind::TableNote) => self.reset_table_note_draft(),
                _ => {}
            }
        }
    }

    pub(super) fn reset_table_note_draft(&mut self) {
        let _ = self
            .table_note_state
            .reset(&mut self.text_context, &self.demo.table_note);
    }

    pub(super) fn commit_table_note_draft(&mut self) -> bool {
        let accepted =
            match resolve_value_draft(self.table_note_state.text(), &self.demo.table_note, |raw| {
                (raw.chars().count() <= 128).then(|| raw.to_owned())
            }) {
                DraftResolution::Accepted { value, .. } => {
                    self.demo.table_note = value;
                    true
                }
                DraftResolution::Rejected { .. } => false,
            };
        self.reset_table_note_draft();
        accepted
    }

    /// End this thumb's capture without retiring a replacement owner or button.
    pub(super) fn cancel_window_scrollbar_drag(&mut self, window: UiWindowId) {
        if window == STANDALONE_WINDOW {
            cancel_scrollbar_drag(window, &mut self.scrollbar_drag, &mut self.input_ownership);
        } else if let Some(host) = self
            .native_windows
            .values_mut()
            .find(|host| host.ui_window == window)
        {
            cancel_scrollbar_drag(window, &mut host.scrollbar_drag, &mut host.input);
        }
    }

    /// Apply shared cancellation intents before a field transition can commit an old preview.
    pub(super) fn cancel_window_value_edits(&mut self, window: UiWindowId) {
        let mut shared_changed = self.finish_workspace_resize_for(window, false);
        shared_changed |= self.can_edit_color(window);
        self.cancel_color_for(window);
        if window == STANDALONE_WINDOW {
            shared_changed |= self.cancel_number_scrub();
            shared_changed |= self.cancel_vector_scrub();
            shared_changed |= self.cancel_drawer_resize();
            shared_changed |= self.cancel_table_resize();
            shared_changed |= self.cancel_ramp_drag();
            shared_changed |= self.cancel_slider_edit();
            self.cancel_window_scrollbar_drag(window);
        } else {
            let mut number_dirty = false;
            let mut vector_axis = None;
            if let Some(id) = self
                .native_windows
                .iter()
                .find(|(_, host)| host.ui_window == window)
                .map(|(id, _)| *id)
            {
                shared_changed |= self.cancel_native_ramp_drag(id);
            }
            if let Some(host) = self
                .native_windows
                .values_mut()
                .find(|host| host.ui_window == window)
            {
                if let Some(mut scrub) = host.number_scrub.take()
                    && let Some(PointerEditOutcome::Cancel { restore }) =
                        scrub.finish(window, &self.demo.number, false)
                {
                    self.demo.number = restore;
                    number_dirty = true;
                    shared_changed = true;
                }
                if let Some(mut scrub) = host.vector_scrub.take() {
                    let index = usize::from(scrub.axis);
                    if let Some(PointerEditOutcome::Cancel { restore }) =
                        scrub.finish(window, &self.demo.vector[index], false)
                    {
                        self.demo.vector[index] = restore;
                        vector_axis = Some(scrub.axis);
                        shared_changed = true;
                    }
                }
                if let Some(mut drag) = host.drawer_resize.take()
                    && let Some(PointerEditOutcome::Cancel { restore }) =
                        drag.finish(window, &self.demo.drawer_width, false)
                {
                    self.demo.drawer_width = restore;
                    shared_changed = true;
                }
                shared_changed |= cancel_table_resize_session(
                    window,
                    &mut host.table_resize,
                    &mut host.input,
                    &mut self.demo.table_column_fractions,
                );
                if let Some(mut edit) = host.slider_drag.take()
                    && let Some(PointerEditOutcome::Cancel { restore }) =
                        edit.finish(window, &self.demo.slider, false)
                {
                    self.demo.slider = restore;
                    shared_changed = true;
                }
                host.scrollbar_drag = None;
                host.input.cancel_capture();
            }
            if number_dirty {
                self.reset_number_draft();
            }
            if let Some(axis) = vector_axis {
                self.reset_vector_draft(axis);
            }
        }
        if shared_changed {
            self.ui_query.invalidate_shared();
            self.request_all_redraw();
        }
    }

    pub(super) fn finish_slider_edit(&mut self) -> bool {
        self.slider_drag.take().is_some_and(|mut edit| {
            edit.finish(STANDALONE_WINDOW, &self.demo.slider, true)
                .is_some()
        })
    }

    pub(super) fn cancel_slider_edit(&mut self) -> bool {
        let Some(mut edit) = self.slider_drag.take() else {
            return false;
        };
        if let Some(PointerEditOutcome::Cancel { restore }) =
            edit.finish(STANDALONE_WINDOW, &self.demo.slider, false)
        {
            self.demo.slider = restore;
        }
        self.input_ownership.cancel_capture();
        true
    }
}

pub(super) fn drawer_state(demo: &DemoState) -> SidebarDrawerState<'static> {
    SidebarDrawerState {
        title: "Sample sidebar",
        open: demo.drawer_open,
        width: demo.drawer_width,
        min_width: 100.0,
        max_width: 320.0,
        keyboard_step: 8.0,
    }
}

pub(super) struct ScalarControlFrame<'a> {
    pub(super) gallery: ElementScope,
    pub(super) actions: &'a ActionRoute<DemoAction>,
    pub(super) focus: ElementId,
    pub(super) focus_root: Option<ElementId>,
    pub(super) table: Option<Rect>,
}

#[derive(Debug, PartialEq)]
pub(super) enum ScalarKeyUpdate {
    Slider(f32),
    Drawer(f32),
    Table([f32; 3]),
}

impl ScalarControlFrame<'_> {
    fn accepts_focus(&self) -> bool {
        self.actions.focus_order().contains(&self.focus)
            && self
                .focus_root
                .is_none_or(|root| self.actions.is_descendant_of(self.focus, root))
    }
}

pub(super) fn table_resize_spec_for_geometry(
    gallery: ElementScope,
    table: Rect,
    fractions: &[f32; 3],
    column: usize,
) -> Option<TableColumnResize> {
    let (min_width, max_width) =
        table_resize_width_bounds(fractions, table.width.max(1.0), column)?;
    Some(TableColumnResize {
        handle: WidgetIdentity::new(
            table_resize_element(gallery, column),
            format!("gallery.table.column.{column}.resize"),
        ),
        min_width,
        max_width,
        keyboard_step: 8.0,
    })
}

pub(super) fn scalar_key_update(
    demo: &DemoState,
    frame: &ScalarControlFrame<'_>,
    key: &Key,
) -> Option<ScalarKeyUpdate> {
    if !frame.accepts_focus() {
        return None;
    }
    let direction = match key {
        Key::Named(NamedKey::ArrowLeft | NamedKey::ArrowDown) => 0,
        Key::Named(NamedKey::ArrowRight | NamedKey::ArrowUp) => 1,
        Key::Named(NamedKey::PageDown) => 2,
        Key::Named(NamedKey::PageUp) => 3,
        Key::Named(NamedKey::Home) => 4,
        Key::Named(NamedKey::End) => 5,
        _ => return None,
    };
    if frame.actions.action_at(frame.focus) == Some(&DemoAction::SetSlider) {
        let key = [
            SliderKey::Decrease,
            SliderKey::Increase,
            SliderKey::PageDecrease,
            SliderKey::PageIncrease,
            SliderKey::Start,
            SliderKey::End,
        ][direction];
        return Some(ScalarKeyUpdate::Slider(slider_value_for_key(
            demo.slider,
            key,
            0.01,
        )));
    }
    if demo.drawer_open && frame.focus == frame.gallery.element(DRAWER_RESIZE) {
        let key = [
            SidebarDrawerResizeKey::Decrease,
            SidebarDrawerResizeKey::Increase,
            SidebarDrawerResizeKey::PageDecrease,
            SidebarDrawerResizeKey::PageIncrease,
            SidebarDrawerResizeKey::Minimum,
            SidebarDrawerResizeKey::Maximum,
        ][direction];
        return Some(ScalarKeyUpdate::Drawer(sidebar_drawer_width_for_key(
            demo.drawer_width,
            key,
            drawer_state(demo),
        )));
    }
    let column =
        (0..2).find(|column| frame.focus == table_resize_element(frame.gallery, *column))?;
    let table = frame.table?;
    let spec =
        table_resize_spec_for_geometry(frame.gallery, table, &demo.table_column_fractions, column)?;
    let key = [
        TableResizeKey::Decrease,
        TableResizeKey::Increase,
        TableResizeKey::PageDecrease,
        TableResizeKey::PageIncrease,
        TableResizeKey::Minimum,
        TableResizeKey::Maximum,
    ][direction];
    let table_width = table.width.max(1.0);
    let widths = table_column_widths(&demo.table_column_fractions, table_width)?;
    let width = table_column_width_for_key(widths[column], key, &spec);
    table_column_fractions_after(&demo.table_column_fractions, table_width, column, width)
        .map(ScalarKeyUpdate::Table)
}

impl App {
    pub(super) fn window_actions(&self, window: UiWindowId) -> Option<&ActionRoute<DemoAction>> {
        if window == STANDALONE_WINDOW {
            Some(&self.ui_actions)
        } else {
            self.native_windows
                .values()
                .find(|host| host.ui_window == window)
                .map(|host| &host.actions)
        }
    }

    pub(super) fn handle_scalar_control_key(&mut self, window: UiWindowId, key: &Key) -> bool {
        let menu_open = if window == STANDALONE_WINDOW {
            self.demo.menu.is_open()
        } else {
            self.native_windows
                .values()
                .any(|host| host.ui_window == window && host.pane_menu.is_some())
        };
        if menu_open {
            return false;
        }
        let table = if window == STANDALONE_WINDOW {
            self.table_rect
        } else {
            self.native_windows
                .values()
                .find(|host| host.ui_window == window)
                .and_then(|host| host.table_rect)
        };
        let Some((scopes, actions, focus)) = self
            .window_scopes(window)
            .zip(self.window_actions(window))
            .zip(self.window_focus(window))
            .map(|((scopes, actions), focus)| (scopes, actions, focus))
        else {
            return false;
        };
        let frame = ScalarControlFrame {
            gallery: scopes.gallery,
            actions,
            focus,
            focus_root: self.focus_root_for_window(window),
            table,
        };
        if !frame.accepts_focus() {
            return false;
        }
        let update = scalar_key_update(&self.demo, &frame, key);
        self.action_window = window;
        if self.handle_number_step_key_for(window, key) {
            return true;
        }
        match update {
            Some(ScalarKeyUpdate::Slider(value)) => {
                self.apply_demo_action(DemoAction::SetSliderValue(value));
            }
            Some(ScalarKeyUpdate::Drawer(value)) => self.demo.drawer_width = value,
            Some(ScalarKeyUpdate::Table(fractions)) => {
                self.demo.table_column_fractions = fractions;
            }
            None => return false,
        }
        true
    }

    fn handle_number_step_key_for(&mut self, window: UiWindowId, key: &Key) -> bool {
        let field = self.text_session_field(window);
        match field {
            Some(TextFieldKind::Number) if !self.number_state.is_composing() => {}
            Some(TextFieldKind::Vector(axis))
                if !self.vector_states[usize::from(axis)].is_composing() => {}
            _ => return false,
        }
        let delta = match key {
            Key::Named(NamedKey::ArrowUp) => 1,
            Key::Named(NamedKey::ArrowDown) => -1,
            Key::Named(NamedKey::PageUp) => 10,
            Key::Named(NamedKey::PageDown) => -10,
            _ => return false,
        };
        match field {
            Some(TextFieldKind::Number) => {
                self.commit_number_draft();
                self.apply_demo_action(DemoAction::StepNumber(delta));
            }
            Some(TextFieldKind::Vector(axis)) => {
                self.commit_vector_draft(axis);
                self.apply_demo_action(DemoAction::StepVector(axis, delta as i8));
            }
            _ => unreachable!(),
        }
        true
    }
}
