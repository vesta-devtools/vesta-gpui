//! Application commands share one worker generation and immutable source session.
use super::*;
impl App {
    pub(super) fn cancel_drag(&mut self) {
        let preview = self.drag.is_some();
        if let Some(drag) = self.drag.take()
            && let Some(timeline) = &mut self.timeline
        {
            timeline.view = drag.original;
            timeline.selection = drag.selection;
        }
        self.ownership
            .release_pointer(WINDOW, PointerButton::Primary);
        self.ownership
            .release_pointer(WINDOW, PointerButton::Secondary);
        self.click.cancel();
        if preview && !self.busy() {
            self.checkpoint = None;
        }
    }
    pub(super) fn suspend_geometry(&mut self) {
        self.cancel_drag();
        self.route = None;
        self.hit = None;
        self.layout_tree = None;
        self.search.clear_geometry();
        if let Some(window) = &self.window {
            self.search.bridge.sync_ime(window, false, None);
        }
    }
    pub(super) fn reset_pages(&mut self) {
        self.page = None;
        self.previous_pages.clear();
        self.table_scroll = 0;
    }
    pub(super) fn command(&mut self, command: Command) -> std::result::Result<(), TraceError> {
        if matches!(command, Command::Cancel) {
            return self.cancel_work();
        }
        let global = matches!(
            command,
            Command::FocusSearch | Command::ClearSearch | Command::Timeline | Command::Table
        );
        if self.importing() && !global {
            return Ok(());
        }
        if !global {
            self.cancel_drag();
            self.begin_change();
        }
        match command {
            Command::Timeline | Command::Table => return Ok(()),
            Command::Gpu(key) => {
                self.gpu = Some(key);
                self.gpu_scroll = Default::default();
                return self.reanalyze();
            }
            Command::FocusSearch => {
                self.set_focus(Some(InputTarget {
                    window: WINDOW,
                    element: view::SEARCH,
                }));
                self.search
                    .input
                    .act(&mut self.context, vesta_gpui::TextInputAction::SelectAll);
                self.search.refresh_view();
                self.redraw();
                return Ok(());
            }
            Command::ClearSearch => {
                self.search.input.cancel_compose(&mut self.context);
                self.search.input.end_group();
                self.search
                    .input
                    .act(&mut self.context, vesta_gpui::TextInputAction::SelectAll);
                self.search.input.act(
                    &mut self.context,
                    vesta_gpui::TextInputAction::Delete { word: false },
                );
                self.search.input.end_group();
                self.search.refresh_view();
                self.set_focus(Some(InputTarget {
                    window: WINDOW,
                    element: view::SEARCH,
                }));
                return self.apply_search();
            }
            Command::Panel(panel) => {
                self.panel = panel;
                self.reset_pages();
                return self.reanalyze();
            }
            Command::PageNext(cursor) => {
                self.previous_pages.push(self.page.take());
                self.page = Some(cursor);
                self.table_scroll = 0;
                return self.reanalyze();
            }
            Command::PagePrevious => {
                if let Some(cursor) = self.previous_pages.pop() {
                    self.page = cursor;
                    self.table_scroll = 0;
                    self.reanalyze()?;
                }
                return Ok(());
            }
            _ => {}
        }
        let Some((session, timeline)) = self.session.as_ref().zip(self.timeline.as_mut()) else {
            return Ok(());
        };
        match command {
            Command::Fit => {
                timeline.fit();
                timeline.selection = None;
            }
            Command::ZoomIn => timeline.zoom(0.5, 0.5),
            Command::ZoomOut => timeline.zoom(0.5, 2.0),
            Command::Scope(key) | Command::Frame(key) => {
                if let Some(index) = session.indices.get(&key) {
                    let event = &session.events[*index];
                    timeline.selected = Some(key);
                    timeline.select(event.start, event.end);
                    let range = TimeRange::new(event.start, event.end.max(event.start + 1))
                        .unwrap_or(timeline.view);
                    if event.is_frame || !event.visible_in(timeline.view) {
                        timeline.view = range;
                    }
                    if event.is_span() {
                        self.timer = Some(TimerKey {
                            category: event.category.clone(),
                            name: event.name.clone(),
                        });
                    }
                    let mut y = 0.0;
                    if self.panel != Panel::Counters {
                        for lane in &session.lanes {
                            if lane.key == event.lane {
                                let row = lane
                                    .rows
                                    .iter()
                                    .position(|r| r.depth == event.depth)
                                    .unwrap_or(0);
                                timeline.scroll = (y + 27.0 + row as f32 * 23.0 - 25.0).max(0.0);
                                break;
                            }
                            y += 37.0 + lane.rows.len() as f32 * 23.0;
                        }
                    }
                }
            }
            Command::Worst => {
                if let Some((key, _)) = self.report.as_ref().and_then(|r| r.analysis.max_frame) {
                    return self.command(Command::Frame(key));
                }
            }
            Command::Previous | Command::Next => {
                let frame = timeline
                    .selected
                    .and_then(|key| session.indices.get(&key))
                    .and_then(|i| session.events[*i].frame);
                let current = frame
                    .and_then(|key| {
                        session
                            .frames
                            .iter()
                            .position(|i| session.events[*i].key == key)
                    })
                    .unwrap_or(0);
                let next = if matches!(command, Command::Previous) {
                    current.saturating_sub(1)
                } else {
                    (current + 1).min(session.frames.len().saturating_sub(1))
                };
                if let Some(index) = session.frames.get(next) {
                    return self.command(Command::Frame(session.events[*index].key));
                }
            }
            Command::Timer(category, name) => self.timer = Some(TimerKey { category, name }),
            _ => {}
        }
        self.reset_pages();
        self.reanalyze()?;
        self.redraw();
        Ok(())
    }
}
