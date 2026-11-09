//! Native input routing over current GPUI hit geometry.
use super::*;
use vesta_gpui_winit::{TextPointerAction, UiInputEvent, UiInputOrigin};

impl App {
    pub(super) fn input(
        &mut self,
        event: &UiInputEvent,
        origin: UiInputOrigin,
    ) -> std::result::Result<(), TraceError> {
        if let UiInputEvent::Focused(active) = event {
            self.active = *active;
        }
        if let UiInputEvent::ModifiersChanged(mods) = event {
            self.modifiers = mods.state();
        }
        if let UiInputEvent::CursorMoved { position, .. } = event {
            let scale = self.window.as_ref().map_or(1.0, |w| w.scale_factor());
            self.pointer = vesta_gpui_winit::logical_point(*position, scale);
        }
        let text_hit = self
            .search
            .view
            .is_some_and(|v| v.clip.contains(self.pointer));
        self.search
            .bridge
            .observe_text_pointer_event(event, text_hit);
        if let UiInputEvent::KeyboardInput {
            event: key,
            is_synthetic: false,
            ..
        } = event
            && key.state == ElementState::Pressed
        {
            self.ownership
                .observe_focus_input(vesta_gpui::FocusInput::Keyboard);
            if key.logical_key == Key::Named(NamedKey::Escape)
                && self.busy()
                && !self.search.input.is_composing()
            {
                return self.command(Command::Cancel);
            }
            if self.modifiers.control_key()
                && matches!(&key.logical_key,Key::Character(k)if k.eq_ignore_ascii_case("f"))
                && !self.search.input.is_composing()
            {
                return self.command(Command::FocusSearch);
            }
            if key.logical_key == Key::Named(NamedKey::Tab) && !self.search.input.is_composing() {
                let old = self.ownership.focus();
                if let Some(route) = &self.route {
                    FocusScope::new(WINDOW, route, None)
                        .traverse(&mut self.ownership, self.modifiers.shift_key());
                }
                self.focus_changed(old);
                return Ok(());
            }
            if self.gpu_scroll_key(&key.logical_key) || self.content_scroll_key(&key.logical_key) {
                return Ok(());
            }
            if self.search_focused()
                && key.logical_key == Key::Named(NamedKey::Enter)
                && !self.search.input.is_composing()
            {
                return self.apply_search();
            }
            if self.search_focused()
                && key.logical_key == Key::Named(NamedKey::Escape)
                && !self.search.input.is_composing()
            {
                self.set_focus(None);
                return self.apply_search();
            }
        }
        let search_active = self.search_focused()
            && (self.active || origin == UiInputOrigin::Command)
            && self.search.view.is_some();
        let handled = if search_active
            || matches!(
                event,
                UiInputEvent::ModifiersChanged(_)
                    | UiInputEvent::Focused(false)
                    | UiInputEvent::Ime(winit::event::Ime::Disabled)
            ) {
            match self.search.edit(event, &mut self.context) {
                Ok(handled) => handled,
                Err(error) => {
                    self.status = error.to_string();
                    self.redraw();
                    true
                }
            }
        } else {
            false
        };
        if handled {
            self.redraw();
        }
        if let UiInputEvent::MouseInput {
            state: ElementState::Pressed,
            button: MouseButton::Left,
            ..
        } = event
        {
            self.ownership
                .observe_focus_input(vesta_gpui::FocusInput::Pointer);
            let target = if text_hit {
                Some(InputTarget {
                    window: WINDOW,
                    element: view::SEARCH,
                })
            } else {
                self.hit
                    .as_ref()
                    .and_then(|h| h.hit(self.pointer))
                    .and_then(|hit| {
                        self.route.as_ref().and_then(|r| {
                            r.target_for_hit(hit).map(|(element, _)| InputTarget {
                                window: WINDOW,
                                element,
                            })
                        })
                    })
            };
            self.set_focus(target);
            if text_hit && let Some(view) = self.search.view {
                let target = InputTarget {
                    window: WINDOW,
                    element: view::SEARCH,
                };
                if let Some(action) = self.search.bridge.text_pointer_press(
                    target,
                    self.pointer,
                    Instant::now(),
                    self.search.input.is_composing(),
                ) {
                    self.ownership
                        .capture_pointer(target, PointerButton::Primary);
                    self.search.input.point(
                        &mut self.context,
                        view.to_layout(self.pointer),
                        action == TextPointerAction::Extend,
                        action == TextPointerAction::Word,
                    );
                    self.search.refresh_view();
                    self.redraw();
                }
                self.click.cancel();
                return Ok(());
            }
        }
        if let UiInputEvent::CursorMoved { .. } = event
            && self
                .ownership
                .capture()
                .is_some_and(|c| c.target.element == view::SEARCH)
        {
            if let Some(view) = self.search.view
                && self.search.bridge.text_pointer_drag(
                    InputTarget {
                        window: WINDOW,
                        element: view::SEARCH,
                    },
                    self.pointer,
                )
            {
                self.search.input.point(
                    &mut self.context,
                    view.to_layout(self.pointer),
                    true,
                    false,
                );
                self.search.refresh_view();
                self.redraw();
            }
            return Ok(());
        }
        if matches!(
            event,
            UiInputEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            }
        ) {
            self.ownership
                .release_pointer(WINDOW, PointerButton::Primary);
        }
        if handled
            && matches!(
                event,
                UiInputEvent::KeyboardInput { .. } | UiInputEvent::Ime(_) | UiInputEvent::Text(_)
            )
        {
            return Ok(());
        }
        match event {
            UiInputEvent::ModifiersChanged(mods) => self.modifiers = mods.state(),
            UiInputEvent::CursorMoved { position, .. } => {
                let scale = self.window.as_ref().map_or(1.0, |w| w.scale_factor());
                self.pointer = vesta_gpui_winit::logical_point(*position, scale);
                if let Some(drag) = &mut self.drag {
                    let delta = (self.pointer.x - drag.anchor) / self.area.width;
                    drag.moved |= (self.pointer.x - drag.anchor).abs() > 3.0;
                    if drag.moved {
                        self.click.cancel();
                    }
                    if let Some(timeline) = &mut self.timeline {
                        if drag.pan {
                            timeline.view = drag.original;
                            timeline.pan(-f64::from(delta));
                        } else if drag.moved {
                            timeline.select(
                                timeline
                                    .at(f64::from((drag.anchor - self.area.x) / self.area.width)),
                                timeline.at(f64::from(
                                    (self.pointer.x - self.area.x) / self.area.width,
                                )),
                            );
                        }
                    }
                    self.redraw();
                }
            }
            UiInputEvent::MouseInput { state, button, .. } => {
                let hit = self.hit.as_ref().and_then(|h| h.hit(self.pointer));
                if *state == ElementState::Pressed {
                    if *button == MouseButton::Left
                        && let Some(route) = &self.route
                    {
                        self.click.press(WINDOW, PointerButton::Primary, route, hit);
                    }
                    if matches!(button, MouseButton::Left | MouseButton::Right)
                        && !self.busy()
                        && self.area.contains(self.pointer)
                        && self.timeline.is_some()
                    {
                        self.begin_change();
                        let timeline = self.timeline.as_ref().expect("existing timeline");
                        self.drag = Some(Drag {
                            anchor: self.pointer.x,
                            original: timeline.view,
                            selection: timeline.selection,
                            pan: *button == MouseButton::Right,
                            moved: false,
                        });
                        self.ownership.capture_pointer(
                            InputTarget {
                                window: WINDOW,
                                element: view::TRACK,
                            },
                            if *button == MouseButton::Right {
                                PointerButton::Secondary
                            } else {
                                PointerButton::Primary
                            },
                        );
                    }
                } else {
                    let finished = self.drag.as_ref().is_some_and(|d| {
                        if d.pan {
                            *button == MouseButton::Right
                        } else {
                            *button == MouseButton::Left
                        }
                    });
                    let changed = finished && self.drag.take().is_some_and(|d| d.moved);
                    if finished && !changed && !self.busy() {
                        self.checkpoint = None;
                    }
                    if finished {
                        self.ownership.release_pointer(
                            WINDOW,
                            if *button == MouseButton::Right {
                                PointerButton::Secondary
                            } else {
                                PointerButton::Primary
                            },
                        );
                    }
                    if changed {
                        self.click.cancel();
                        self.reset_pages();
                        self.reanalyze()?;
                    } else if *button == MouseButton::Left {
                        let command = self
                            .route
                            .as_ref()
                            .and_then(|route| {
                                self.click
                                    .release(WINDOW, PointerButton::Primary, route, hit)
                            })
                            .cloned();
                        if let Some(command) = command {
                            self.command(command)?;
                        }
                    }
                }
            }
            UiInputEvent::MouseWheel { delta, .. } => {
                self.cancel_drag();
                if self.importing() {
                    return Ok(());
                }
                let scale = self.window.as_ref().map_or(1.0, |w| w.scale_factor());
                let delta = vesta_gpui_winit::wheel_delta(*delta, scale, 32.0);
                if matches!(self.panel, Panel::Gpu | Panel::Memory)
                    && self
                        .layout_tree
                        .as_ref()
                        .and_then(|l| l.node(view::GPU_DETAIL))
                        .is_some_and(|n| n.scroll_viewport().contains(self.pointer))
                {
                    self.gpu_scroll.scroll_by(-delta.y);
                    self.redraw();
                    return Ok(());
                }
                if self.table_area.contains(self.pointer) {
                    let rows = (delta.y.abs() / 25.0).ceil().max(1.0) as usize;
                    let requested = if delta.y > 0.0 {
                        self.table_scroll.saturating_sub(rows)
                    } else {
                        self.table_scroll.saturating_add(rows)
                    };
                    self.set_table_scroll(requested);
                    return Ok(());
                }
                if !self.area.contains(self.pointer) {
                    return Ok(());
                }
                if self.timeline.is_some() {
                    if self.modifiers.control_key() {
                        self.begin_change();
                        self.reset_pages();
                        let timeline = self.timeline.as_mut().expect("existing timeline");
                        let anchor = f64::from((self.pointer.x - self.area.x) / self.area.width);
                        timeline.zoom(anchor, if delta.y > 0.0 { 0.8 } else { 1.25 });
                        self.reanalyze()?;
                    } else if self.modifiers.shift_key() {
                        self.begin_change();
                        self.reset_pages();
                        let timeline = self.timeline.as_mut().expect("existing timeline");
                        timeline.pan(-f64::from(delta.y) / 600.0);
                        self.reanalyze()?;
                    } else {
                        let timeline = self.timeline.as_mut().expect("existing timeline");
                        timeline.scroll = (timeline.scroll - delta.y).max(0.0);
                    }
                }
                self.redraw();
            }
            UiInputEvent::KeyboardInput {
                event,
                is_synthetic: false,
                ..
            } if event.state == ElementState::Pressed && !self.search_focused() => {
                if matches!(
                    event.logical_key,
                    Key::Named(NamedKey::Enter | NamedKey::Space)
                ) {
                    let action = self
                        .ownership
                        .focus()
                        .and_then(|target| {
                            self.route
                                .as_ref()
                                .and_then(|route| route.action_at(target.element))
                        })
                        .cloned();
                    if let Some(action) = action {
                        self.command(action)?;
                        return Ok(());
                    }
                }
                let command = match &event.logical_key {
                    Key::Named(NamedKey::Home) => Some(Command::Fit),
                    Key::Named(NamedKey::ArrowLeft) => Some(Command::Previous),
                    Key::Named(NamedKey::ArrowRight) => Some(Command::Next),
                    Key::Character(key) if key == "+" || key == "=" => Some(Command::ZoomIn),
                    Key::Character(key) if key == "-" => Some(Command::ZoomOut),
                    Key::Character(key) if key.eq_ignore_ascii_case("w") => Some(Command::Worst),
                    _ => None,
                };
                if event.logical_key == Key::Named(NamedKey::Escape) {
                    if self.drag.is_some() {
                        self.cancel_drag();
                        self.redraw();
                    } else {
                        self.click.cancel();
                        self.begin_change();
                        if let Some(timeline) = &mut self.timeline {
                            timeline.selection = None;
                        }
                        self.reset_pages();
                        self.reanalyze()?;
                    }
                }
                if let Some(command) = command {
                    self.command(command)?;
                }
            }
            UiInputEvent::Focused(false) => {
                self.search.end_session(&mut self.context);
                self.cancel_drag();
                self.ownership.cancel_focus_request();
                self.modifiers = Default::default();
                self.redraw();
            }
            UiInputEvent::DroppedFile(path) => {
                self.open_source(path.clone())?;
            }
            _ => {}
        }
        Ok(())
    }

    fn gpu_scroll_key(&mut self, key: &Key) -> bool {
        if !matches!(self.panel, Panel::Gpu | Panel::Memory)
            || self
                .ownership
                .focus()
                .is_none_or(|t| t.element != view::GPU_DETAIL)
        {
            return false;
        }
        let Some(node) = self
            .layout_tree
            .as_ref()
            .and_then(|l| l.node(view::GPU_DETAIL))
        else {
            return false;
        };
        let page = node.scroll_viewport().height.max(1.0);
        let delta = match key {
            Key::Named(NamedKey::PageDown) => page,
            Key::Named(NamedKey::PageUp) => -page,
            Key::Named(NamedKey::ArrowDown) => 25.0,
            Key::Named(NamedKey::ArrowUp) => -25.0,
            Key::Named(NamedKey::End) => self.gpu_scroll.max_y(),
            Key::Named(NamedKey::Home) => -self.gpu_scroll.max_y(),
            _ => return false,
        };
        self.cancel_drag();
        self.gpu_scroll.scroll_by(delta);
        self.redraw();
        true
    }
}
