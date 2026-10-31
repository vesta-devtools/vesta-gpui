//! Opt-in observations of successfully presented native windows. Never dispatches actions.
use super::*;
use std::{collections::VecDeque, sync::mpsc::SyncSender};
use vesta_gpui::{UiFieldContent, UiQueryLimits, UiSnapshot, UiSnapshotFrame, UiSnapshotId};

pub(super) mod admission;
mod capture;
mod control;
mod input;
mod wire;
pub(super) use wire::start;

#[derive(Clone, Debug)]
pub(super) enum Operation {
    Input {
        request_id: String,
        request: vesta_gpui_winit::UiInputRequest,
    },
    WindowState {
        instance: u128,
        window: UiWindowId,
    },
    InputState {
        instance: u128,
        window: UiWindowId,
    },
    Capture(capture::Query),
    Windows,
    Changes {
        instance: u128,
        window: UiWindowId,
        since: u64,
    },
    Commands {
        instance: u128,
        window: UiWindowId,
    },
    Invoke {
        request_id: String,
        target: vesta_gpui::UiTargetRef,
        command: vesta_gpui::UiCommand,
    },
    Snapshot {
        instance: u128,
        window: UiWindowId,
        revision: Option<u64>,
        root: Option<ElementId>,
        after: Option<ElementId>,
        limit: usize,
    },
}

#[derive(Debug)]
pub(super) struct Request {
    pub operation: Operation,
    pub reply: SyncSender<Result<(Operation, Reply), &'static str>>,
}

#[derive(Debug)]
pub(super) enum Reply {
    Changes(Arc<UiSnapshot>, Arc<UiSnapshot>),
    Capture(capture::Reply),
    Control(serde_json::Value),
    Windows {
        instance: u128,
        read_only: bool,
        windows: Vec<(UiWindowId, Option<UiSnapshotId>)>,
        truncated: bool,
    },
    Snapshot(Arc<UiSnapshot>),
}

#[derive(Clone, Debug, PartialEq)]
enum ControlIntent {
    Semantic(vesta_gpui::UiTargetRef, vesta_gpui::UiCommand),
    Input(vesta_gpui_winit::UiInputRequest),
}
type CommandReceipt = (
    String,
    ControlIntent,
    Result<serde_json::Value, &'static str>,
);
struct InputRoot {
    root: Element,
    input: vesta_gpui_winit::UiInputObservation,
}
impl std::ops::Deref for InputRoot {
    type Target = Element;
    fn deref(&self) -> &Element {
        &self.root
    }
}
pub(super) struct Store {
    pub enabled: bool,
    pub control_enabled: bool,
    pub(super) input_origin: vesta_gpui_winit::UiInputOrigin,
    pub(super) input_error: Option<&'static str>,
    owners: vesta_gpui::UiOwnerRegistry<DemoAction>,
    instance: u128,
    latest: HashMap<UiWindowId, Arc<UiSnapshot>>,
    roots: HashMap<UiWindowId, Arc<InputRoot>>,
    pinned_roots: HashMap<(UiWindowId, u64), Arc<InputRoot>>,
    captures: HashMap<UiWindowId, capture::Binding>,
    pinned: VecDeque<Arc<UiSnapshot>>,
    dirty: HashSet<UiWindowId>,
    receipts: VecDeque<CommandReceipt>,
}

impl Store {
    pub(super) fn invalidate_window(&mut self, window: UiWindowId) {
        if self.enabled {
            self.dirty.insert(window);
        }
    }

    pub(super) fn invalidate_shared(&mut self) {
        if self.enabled {
            // Gallery editors, theme and workspace are shared across native windows.
            self.dirty.extend(self.latest.keys().copied());
        }
    }
    pub fn new(enabled: bool) -> Self {
        // Lifetime identity for a local diagnostic process, not an authentication secret.
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        Self {
            enabled,
            control_enabled: false,
            input_origin: vesta_gpui_winit::UiInputOrigin::Native,
            input_error: None,
            owners: Default::default(),
            instance: (now << 32) | u128::from(std::process::id()),
            latest: HashMap::new(),
            roots: HashMap::new(),
            pinned_roots: HashMap::new(),
            captures: HashMap::new(),
            pinned: VecDeque::new(),
            dirty: HashSet::new(),
            receipts: VecDeque::new(),
        }
    }

    pub fn next(&self, window: UiWindowId) -> Option<UiSnapshotId> {
        if !self.enabled || (!self.latest.contains_key(&window) && self.latest.len() >= 16) {
            return None;
        }
        let revision = self
            .latest
            .get(&window)
            .map_or(Some(1), |old| old.id.revision.checked_add(1))?;
        Some(UiSnapshotId {
            instance: self.instance,
            window,
            revision,
        })
    }

    pub fn publish(&mut self, snapshot: UiSnapshot) {
        let snapshot = Arc::new(snapshot);
        self.dirty.remove(&snapshot.id.window);
        self.latest.insert(snapshot.id.window, snapshot.clone());
        if let Some(binding) = self.captures.remove(&snapshot.id.window) {
            self.pin(snapshot.clone());
            let _ = binding.presented.set(snapshot.id);
        }
    }

    fn pin(&mut self, snapshot: Arc<UiSnapshot>) {
        if self.pinned.iter().any(|pinned| pinned.id == snapshot.id) {
            return;
        }
        if self.pinned.len() == 8
            && let Some(old) = self.pinned.pop_front()
        {
            self.pinned_roots.remove(&(old.id.window, old.id.revision));
        }
        if self
            .latest
            .get(&snapshot.id.window)
            .is_some_and(|latest| latest.id == snapshot.id)
            && let Some(root) = self.roots.get(&snapshot.id.window)
        {
            self.pinned_roots
                .insert((snapshot.id.window, snapshot.id.revision), root.clone());
        }
        self.pinned.push_back(snapshot);
    }

    pub fn publish_root(
        &mut self,
        snapshot: UiSnapshot,
        root: Element,
        route: &ActionRoute<DemoAction>,
        layout: &LayoutTree,
        context: String,
    ) {
        if self.owners.update(&snapshot, route).is_err() {
            return;
        }
        let input = vesta_gpui_winit::UiInputObservation::capture(
            &snapshot,
            &self.owners,
            route,
            &root,
            layout,
            context,
        );
        self.roots
            .insert(snapshot.id.window, Arc::new(InputRoot { root, input }));
        self.publish(snapshot);
    }
    pub fn cancel_capture_binding(&mut self, window: UiWindowId) {
        self.captures.remove(&window);
    }

    pub fn forget(&mut self, window: UiWindowId) {
        self.owners.forget(window);
        self.roots.remove(&window);
        self.pinned_roots.retain(|(w, _), _| *w != window);
        self.captures.remove(&window);
        self.dirty.remove(&window);
        self.latest.remove(&window);
        self.pinned.retain(|snapshot| snapshot.id.window != window);
    }

    fn answer(
        &mut self,
        operation: &Operation,
        live: &[UiWindowId],
    ) -> Result<Reply, &'static str> {
        self.latest.retain(|window, _| live.contains(window));
        self.roots.retain(|window, _| live.contains(window));
        self.pinned_roots
            .retain(|(window, _), _| live.contains(window));
        self.pinned
            .retain(|snapshot| live.contains(&snapshot.id.window));
        match operation {
            Operation::Changes {
                instance,
                window,
                since,
            } => {
                if *instance != self.instance {
                    return Err("foreign_instance");
                }
                if !live.contains(window) {
                    return Err("window_not_found");
                }
                let latest = self.latest.get(window).cloned().ok_or("not_ready")?;
                let previous = if latest.id.revision == *since {
                    latest.clone()
                } else {
                    self.pinned
                        .iter()
                        .find(|s| s.id.window == *window && s.id.revision == *since)
                        .cloned()
                        .ok_or("stale_snapshot")?
                };
                self.pin(latest.clone());
                Ok(Reply::Changes(latest, previous))
            }
            Operation::Commands { .. }
            | Operation::Invoke { .. }
            | Operation::Capture(_)
            | Operation::Input { .. }
            | Operation::WindowState { .. }
            | Operation::InputState { .. } => Err("control_disabled"),
            Operation::Windows => Ok(Reply::Windows {
                instance: self.instance,
                read_only: !self.control_enabled,
                windows: live
                    .iter()
                    .take(16)
                    .map(|window| (*window, self.latest.get(window).map(|snapshot| snapshot.id)))
                    .collect(),
                truncated: live.len() > 16,
            }),
            Operation::Snapshot {
                instance,
                window,
                revision,
                ..
            } => {
                if *instance != self.instance {
                    return Err("foreign_instance");
                }
                if !live.contains(window) {
                    return Err("window_not_found");
                }
                let latest = self.latest.get(window).ok_or("not_ready")?;
                let snapshot = match revision {
                    Some(revision) if *revision != latest.id.revision => self
                        .pinned
                        .iter()
                        .find(|snapshot| {
                            snapshot.id.window == *window && snapshot.id.revision == *revision
                        })
                        .cloned()
                        .ok_or("stale_snapshot")?,
                    _ => latest.clone(),
                };
                // Pin only queried revisions. Repaints do not invalidate a client's next page.
                self.pin(snapshot.clone());
                Ok(Reply::Snapshot(snapshot))
            }
        }
    }
}

impl App {
    pub(super) fn ui_input_context(&self, window: UiWindowId, current: &Window) -> String {
        let geometry = |id: UiWindowId, native: &Window| {
            (
                id,
                native.inner_position().ok(),
                (id != window).then(|| (native.inner_size(), native.scale_factor())),
            )
        };
        let own = geometry(window, current);
        let main = self
            .window
            .as_ref()
            .filter(|_| window != STANDALONE_WINDOW)
            .map(|native| geometry(STANDALONE_WINDOW, native));
        let floating = self
            .native_window_order
            .iter()
            .filter_map(|id| self.native_windows.get(id))
            .filter(|host| host.ui_window != window)
            .map(|host| geometry(host.ui_window, &host.window))
            .collect::<Vec<_>>();
        let shared = format!(
            "{:?}:{:?}:{:?}:{own:?}:{main:?}:{floating:?}",
            self.mode, self.theme.mode, self.demo.workspace
        );
        if self.mode == AcceptanceMode::Viewport3d {
            format!("{shared}:{:?}:{:?}", self.camera, self.scene)
        } else {
            shared
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn capture_query_snapshot(
        &self,
        window: UiWindowId,
        gallery: ElementScope,
        root: &Element,
        layout: &LayoutTree,
        route: &ActionRoute<DemoAction>,
        focus: Option<InputTarget>,
        modal_root: Option<ElementId>,
    ) -> Option<UiSnapshot> {
        let id = self.ui_query.next(window)?;
        let fields = |element| {
            let kind = TextFieldKind::ALL
                .into_iter()
                .find(|kind| kind.element(gallery) == element)?;
            let state = match kind {
                TextFieldKind::Main => &self.text_state,
                TextFieldKind::Search => &self.search_state,
                TextFieldKind::Area => &self.area_state,
                TextFieldKind::Number => &self.number_state,
                TextFieldKind::Vector(axis) => &self.vector_states[usize::from(axis)],
                TextFieldKind::ColorHex => &self.color_hex_state,
                TextFieldKind::Password => &self.password_state,
                TextFieldKind::TableNote => &self.table_note_state,
                TextFieldKind::Label => &self.readonly_label_state,
            };
            Some(UiFieldContent {
                text: state.text(),
                selection: state.inspection_selection(),
                sensitive: state.is_password(),
                read_only: state.is_read_only(),
                composing: state.is_composing(),
            })
        };
        match UiSnapshot::capture(
            UiSnapshotFrame {
                id,
                root,
                layout,
                route,
                focus,
                modal_root,
            },
            UiQueryLimits::default(),
            fields,
            |_| false,
        ) {
            Ok(snapshot) => Some(snapshot),
            Err(error) => {
                eprintln!("UI query capture rejected: {error:?}");
                None
            }
        }
    }

    pub(super) fn answer_ui_query(&mut self, event_loop: &ActiveEventLoop, request: Request) {
        if let Operation::InputState { instance, window } = &request.operation {
            let result = self
                .ui_native_window(*instance, *window)
                .and_then(|_| {
                    if *window == STANDALONE_WINDOW {
                        Ok(vesta_gpui_winit::ui_input_state(
                            &self.input_ownership,
                            self.cursor,
                            self.window_modifiers(*window),
                        ))
                    } else {
                        let host = self
                            .native_windows
                            .values()
                            .find(|host| host.ui_window == *window)
                            .ok_or("window_not_found")?;
                        Ok(vesta_gpui_winit::ui_input_state(
                            &host.input,
                            host.cursor,
                            self.window_modifiers(*window),
                        ))
                    }
                })
                .map(Reply::Control);
            let _ = request
                .reply
                .try_send(result.map(|reply| (request.operation, reply)));
            return;
        }
        if let Operation::Input {
            request_id,
            request: input,
        } = &request.operation
        {
            let reply = self
                .answer_ui_input(event_loop, request_id, input)
                .map(Reply::Control);
            let _ = request
                .reply
                .try_send(reply.map(|reply| (request.operation, reply)));
            return;
        }
        if let Operation::WindowState { instance, window } = &request.operation {
            let reply = self.ui_native_window(*instance, *window).map(|native| {
                let mut value = vesta_gpui_winit::ui_window_state(&native);
                if let Some(profile) = self.profile_window_state(*window) {
                    value["profiling"] = profile;
                }
                Reply::Control(value)
            });
            let _ = request
                .reply
                .try_send(reply.map(|reply| (request.operation, reply)));
            return;
        }
        if let Operation::Capture(query) = &request.operation {
            let reply = self.answer_ui_capture(query).map(Reply::Capture);
            let _ = request
                .reply
                .try_send(reply.map(|reply| (request.operation, reply)));
            return;
        }
        if matches!(
            request.operation,
            Operation::Commands { .. } | Operation::Invoke { .. }
        ) {
            let reply = self
                .answer_ui_control(&request.operation)
                .map(Reply::Control);
            let _ = request
                .reply
                .try_send(reply.map(|reply| (request.operation, reply)));
            return;
        }
        let mut live = self
            .native_windows
            .values()
            .map(|host| host.ui_window)
            .collect::<Vec<_>>();
        if self.window.is_some() {
            live.push(STANDALONE_WINDOW);
        }
        live.sort_by_key(|window| window.0);
        let reply = self.ui_query.answer(&request.operation, &live);
        // One bounded response per request; serialization and stdout run in the IO worker.
        let _ = request
            .reply
            .try_send(reply.map(|reply| (request.operation, reply)));
    }
}

#[cfg(test)]
mod tests;
