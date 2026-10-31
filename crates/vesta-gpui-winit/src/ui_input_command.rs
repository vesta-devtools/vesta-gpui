//! Bounded, target-relative commands. Host authorization and dispatch remain in the Host.
use crate::{UiInputEvent, UiKeyEvent};
use serde_json::{Value, json};
use vesta_gpui::{ActionRoute, ElementId, Point, UiCommand, UiSnapshot, UiTargetRef, UiWindowId};
use winit::{
    dpi::PhysicalPosition,
    event::{DeviceId, ElementState, Modifiers, MouseButton, MouseScrollDelta, TouchPhase},
    keyboard::{Key, KeyCode, KeyLocation, ModifiersState, NamedKey, NativeKeyCode, PhysicalKey},
};

const MAX_SOURCE_STEPS: usize = 64;
const MAX_EXPANDED_STEPS: usize = 256;
#[cfg(test)]
mod tests;
#[derive(Clone, Debug, PartialEq)]
pub enum UiWindowCommand {
    Minimize,
    Maximize,
    Restore,
    Close,
    RetryRender,
    Resize { width: f32, height: f32 },
    Move { x: i32, y: i32 },
}
#[derive(Clone, Debug, PartialEq)]
pub enum UiInputStep {
    Move(Point),
    Button {
        button: MouseButton,
        pressed: bool,
    },
    Wheel {
        x: f32,
        y: f32,
    },
    Key {
        key: String,
        modifiers: Option<ModifiersState>,
    },
    Text(String),
    Cancel,
    Window(UiWindowCommand),
}
#[derive(Clone, Debug, PartialEq)]
pub struct UiInputRequest {
    pub target: UiTargetRef,
    pub revision: u64,
    pub modifiers: ModifiersState,
    pub steps: Vec<UiInputStep>,
}

fn keys(value: &Value, allowed: &[&str]) -> Result<(), &'static str> {
    if value
        .as_object()
        .is_none_or(|o| o.keys().any(|k| !allowed.contains(&k.as_str())))
    {
        Err("invalid_input")
    } else {
        Ok(())
    }
}
fn number(value: &Value, key: &str) -> Result<u64, &'static str> {
    value[key]
        .as_str()
        .and_then(|s| s.parse().ok())
        .ok_or("invalid_input")
}
fn scalar(value: &Value, key: &str) -> Result<f32, &'static str> {
    value[key]
        .as_f64()
        .filter(|v| v.is_finite() && v.abs() <= 32768.0)
        .map(|v| v as f32)
        .ok_or("invalid_input")
}
fn point(value: &Value) -> Result<Point, &'static str> {
    keys(value, &["x", "y"])?;
    Ok(Point::new(scalar(value, "x")?, scalar(value, "y")?))
}
fn button(value: &Value) -> Result<MouseButton, &'static str> {
    match if value.is_null() {
        Some("primary")
    } else {
        value.as_str()
    } {
        Some("primary") => Ok(MouseButton::Left),
        Some("secondary") => Ok(MouseButton::Right),
        Some("middle") => Ok(MouseButton::Middle),
        _ => Err("invalid_input"),
    }
}
fn modifiers(value: &Value) -> Result<ModifiersState, &'static str> {
    if value.is_null() {
        return Ok(ModifiersState::empty());
    }
    let mut result = ModifiersState::empty();
    for name in value
        .as_array()
        .filter(|a| a.len() <= 4)
        .ok_or("invalid_input")?
    {
        result |= match name.as_str() {
            Some("control") => ModifiersState::CONTROL,
            Some("shift") => ModifiersState::SHIFT,
            Some("alt") => ModifiersState::ALT,
            Some("super") => ModifiersState::SUPER,
            _ => return Err("invalid_input"),
        };
    }
    Ok(result)
}

impl UiInputRequest {
    pub fn parse(value: &Value) -> Result<Self, &'static str> {
        if value["schema_version"] != 1
            || value["op"] != "input"
            || value["request_id"]
                .as_str()
                .is_none_or(|id| id.is_empty() || id.len() > 128)
        {
            return Err("invalid_input");
        }
        keys(
            value,
            &[
                "schema_version",
                "request_id",
                "op",
                "instance",
                "window",
                "element",
                "owner",
                "revision",
                "steps",
                "modifiers",
            ],
        )?;
        let target = UiTargetRef {
            instance: value["instance"]
                .as_str()
                .and_then(|s| s.parse().ok())
                .ok_or("invalid_input")?,
            window: UiWindowId(number(value, "window")?),
            element: ElementId::new(number(value, "element")?),
            owner: number(value, "owner")?,
        };
        let input = value["steps"]
            .as_array()
            .filter(|s| !s.is_empty() && s.len() <= MAX_SOURCE_STEPS)
            .ok_or("invalid_input")?;
        let mut steps = Vec::new();
        let mut held = None;
        let mut text_bytes = 0;
        for step in input {
            match step["kind"].as_str().ok_or("invalid_input")? {
                "cancel" => {
                    keys(step, &["kind"])?;
                    if input.len() != 1 {
                        return Err("cancel_must_be_alone");
                    }
                    steps.push(UiInputStep::Cancel);
                }
                "move" => {
                    keys(step, &["kind", "x", "y"])?;
                    steps.push(UiInputStep::Move(Point::new(
                        scalar(step, "x")?,
                        scalar(step, "y")?,
                    )));
                }
                "press" | "release" => {
                    keys(step, &["kind", "button"])?;
                    let button = button(&step["button"])?;
                    let pressed = step["kind"] == "press";
                    if pressed {
                        if held.replace(button).is_some() {
                            return Err("unbalanced_gesture");
                        }
                    } else if held.take() != Some(button) {
                        return Err("unbalanced_gesture");
                    }
                    steps.push(UiInputStep::Button { button, pressed });
                }
                "click" => {
                    keys(step, &["kind", "button", "count"])?;
                    if held.is_some() {
                        return Err("unbalanced_gesture");
                    }
                    let button = button(&step["button"])?;
                    let count = step
                        .get("count")
                        .map_or(Some(1), Value::as_u64)
                        .filter(|n| (1..=3).contains(n))
                        .ok_or("invalid_input")?;
                    for _ in 0..count {
                        steps.push(UiInputStep::Button {
                            button,
                            pressed: true,
                        });
                        steps.push(UiInputStep::Button {
                            button,
                            pressed: false,
                        });
                    }
                }
                "drag" => {
                    keys(step, &["kind", "from", "to", "button", "samples"])?;
                    if held.is_some() {
                        return Err("unbalanced_gesture");
                    }
                    let from = point(&step["from"])?;
                    let to = point(&step["to"])?;
                    let button = button(&step["button"])?;
                    let samples = step
                        .get("samples")
                        .map_or(Some(8), Value::as_u64)
                        .filter(|n| (1..=64).contains(n))
                        .ok_or("invalid_input")?;
                    steps.push(UiInputStep::Move(from));
                    steps.push(UiInputStep::Button {
                        button,
                        pressed: true,
                    });
                    for n in 1..=samples {
                        let t = n as f32 / samples as f32;
                        steps.push(UiInputStep::Move(Point::new(
                            from.x + (to.x - from.x) * t,
                            from.y + (to.y - from.y) * t,
                        )));
                    }
                    steps.push(UiInputStep::Button {
                        button,
                        pressed: false,
                    });
                }
                "wheel" => {
                    keys(step, &["kind", "x", "y"])?;
                    steps.push(UiInputStep::Wheel {
                        x: scalar(step, "x")?,
                        y: scalar(step, "y")?,
                    });
                }
                "key" => {
                    keys(step, &["kind", "key", "modifiers"])?;
                    let key = step["key"]
                        .as_str()
                        .filter(|k| !k.is_empty() && k.len() <= 32)
                        .ok_or("invalid_input")?;
                    command_key(key)?;
                    steps.push(UiInputStep::Key {
                        key: key.into(),
                        modifiers: step.get("modifiers").map(modifiers).transpose()?,
                    });
                }
                "text" => {
                    keys(step, &["kind", "text"])?;
                    let text = step["text"].as_str().ok_or("invalid_input")?;
                    text_bytes += text.len();
                    if text_bytes > vesta_gpui::TEXT_INPUT_MAX_BYTES {
                        return Err("input_too_large");
                    }
                    steps.push(UiInputStep::Text(text.into()));
                }
                "window" => {
                    keys(step, &["kind", "action", "width", "height", "x", "y"])?;
                    let action = match step["action"].as_str() {
                        Some("minimize") => UiWindowCommand::Minimize,
                        Some("maximize") => UiWindowCommand::Maximize,
                        Some("restore") => UiWindowCommand::Restore,
                        Some("close") => UiWindowCommand::Close,
                        Some("retry_render") => UiWindowCommand::RetryRender,
                        Some("resize") => {
                            let width = scalar(step, "width")?;
                            let height = scalar(step, "height")?;
                            if !(64.0..=8192.0).contains(&width)
                                || !(64.0..=8192.0).contains(&height)
                            {
                                return Err("invalid_input");
                            }
                            UiWindowCommand::Resize { width, height }
                        }
                        Some("move") => UiWindowCommand::Move {
                            x: scalar(step, "x")? as i32,
                            y: scalar(step, "y")? as i32,
                        },
                        _ => return Err("invalid_input"),
                    };
                    if input.len() != 1 {
                        return Err("window_command_must_be_alone");
                    }
                    steps.push(UiInputStep::Window(action));
                }
                _ => return Err("unknown_input"),
            }
            if steps.len() > MAX_EXPANDED_STEPS {
                return Err("input_too_large");
            }
        }
        if held.is_some() {
            return Err("unbalanced_gesture");
        }
        Ok(Self {
            target,
            revision: number(value, "revision")?,
            modifiers: modifiers(&value["modifiers"])?,
            steps,
        })
    }

    pub fn validate<A>(
        &self,
        snapshot: &UiSnapshot,
        route: &ActionRoute<A>,
        modal: Option<ElementId>,
    ) -> Result<(), &'static str> {
        self.validate_with_pointer_boundary(snapshot, route, modal, None)
    }
    /// The Host may admit pointer-only input to its existing custom canvas handler.
    /// Disabled widgets outside this explicitly named subtree remain disabled.
    pub fn validate_with_pointer_boundary<A>(
        &self,
        snapshot: &UiSnapshot,
        route: &ActionRoute<A>,
        modal: Option<ElementId>,
        boundary: Option<ElementId>,
    ) -> Result<(), &'static str> {
        if matches!(
            self.steps.as_slice(),
            [UiInputStep::Window(_) | UiInputStep::Cancel]
        ) {
            return self.validate_window(snapshot);
        }
        let custom = boundary.is_some_and(|id| route.is_descendant_of(self.target.element, id))
            && self.steps.iter().all(|step| {
                matches!(
                    step,
                    UiInputStep::Move(_) | UiInputStep::Button { .. } | UiInputStep::Wheel { .. }
                )
            });
        if custom {
            if self.target.instance != snapshot.id.instance {
                return Err("foreign_instance");
            }
            if self.target.window != snapshot.id.window {
                return Err("foreign_window");
            }
            if modal.is_some_and(|id| !route.is_descendant_of(self.target.element, id)) {
                return Err("modal_blocked");
            }
        } else {
            UiCommand::Reveal.preflight(self.target, snapshot, route, modal)?;
        }
        let node = snapshot
            .nodes()
            .iter()
            .find(|n| n.id == self.target.element)
            .ok_or("target_not_captured")?;
        let mut ancestor = Some(node.id);
        while let Some(id) = ancestor {
            let parent = snapshot
                .nodes()
                .iter()
                .find(|n| n.id == id)
                .ok_or("target_not_captured")?;
            if parent.value.redacted || parent.name.redacted {
                return Err("sensitive_target");
            }
            ancestor = parent.parent;
        }
        if node.composing {
            return Err("composing");
        }
        if self
            .steps
            .iter()
            .any(|step| matches!(step, UiInputStep::Text(_)))
        {
            if node.read_only || node.value.value.is_none() || node.selection.is_none() {
                return Err("not_editor");
            }
            UiCommand::Focus.preflight(self.target, snapshot, route, modal)?;
        }
        let bounds = node.visible_bounds.ok_or("target_not_visible")?;
        if bounds.width <= 0.0 || bounds.height <= 0.0 {
            return Err("target_not_visible");
        }
        if self
            .steps
            .iter()
            .any(|step| matches!(step, UiInputStep::Key { .. } | UiInputStep::Text(_)))
            && let Some(focus) = snapshot
                .nodes()
                .iter()
                .find(|node| Some(node.id) == snapshot.focus)
        {
            if focus.value.redacted || focus.name.redacted {
                return Err("sensitive_target");
            }
            if focus.composing {
                return Err("composing");
            }
        }
        let origin = node.bounds.ok_or("target_not_visible")?;
        if !matches!(self.steps.first(), Some(UiInputStep::Move(_)))
            && self
                .steps
                .iter()
                .any(|s| matches!(s, UiInputStep::Button { .. } | UiInputStep::Wheel { .. }))
        {
            let p = Point::new(
                bounds.x + bounds.width / 2.0,
                bounds.y + bounds.height / 2.0,
            );
            if snapshot.nodes().iter().any(|n| {
                (n.value.redacted || n.name.redacted)
                    && n.visible_bounds.is_some_and(|r| r.contains(p))
            }) {
                return Err("sensitive_target");
            }
        }
        let mut held = false;
        for step in &self.steps {
            if let UiInputStep::Move(offset) = step {
                let p = Point::new(origin.x + offset.x, origin.y + offset.y);
                if !held && !snapshot.viewport.logical_rect().contains(p) {
                    return Err("input_out_of_bounds");
                }
                if snapshot.nodes().iter().any(|n| {
                    (n.value.redacted || n.name.redacted)
                        && n.visible_bounds.is_some_and(|r| r.contains(p))
                }) {
                    return Err("sensitive_target");
                }
            } else if let UiInputStep::Button { pressed, .. } = step {
                held = *pressed;
            }
        }
        Ok(())
    }

    pub fn validate_window(&self, snapshot: &UiSnapshot) -> Result<(), &'static str> {
        if self.target.instance != snapshot.id.instance {
            return Err("foreign_instance");
        }
        if self.target.window != snapshot.id.window {
            return Err("foreign_window");
        }
        if snapshot
            .nodes()
            .first()
            .is_none_or(|root| root.id != self.target.element)
        {
            return Err("window_root_required");
        }
        Ok(())
    }
    /// Validate pointer admission against the Host's actual z-ordered hit tree.
    pub fn validate_hits<A>(
        &self,
        snapshot: &UiSnapshot,
        route: &ActionRoute<A>,
        mut hit: impl FnMut(Point) -> Option<ElementId>,
    ) -> Result<(), &'static str> {
        let node = snapshot
            .nodes()
            .iter()
            .find(|node| node.id == self.target.element)
            .ok_or("target_not_captured")?;
        let bounds = node.bounds.ok_or("target_not_visible")?;
        let visible = node.visible_bounds.ok_or("target_not_visible")?;
        let mut cursor = Point::new(
            visible.x + visible.width / 2.0,
            visible.y + visible.height / 2.0,
        );
        for step in &self.steps {
            match step {
                UiInputStep::Move(p) => cursor = Point::new(bounds.x + p.x, bounds.y + p.y),
                UiInputStep::Button { pressed: true, .. } | UiInputStep::Wheel { .. } => {
                    if hit(cursor).map_or(node.parent.is_some(), |id| {
                        !route.is_descendant_of(id, self.target.element)
                    }) {
                        return Err("target_occluded");
                    }
                }
                _ => (),
            }
        }
        Ok(())
    }

    pub fn events(
        &self,
        snapshot: &UiSnapshot,
        modifiers: ModifiersState,
    ) -> Result<Vec<UiInputEvent>, &'static str> {
        let bounds = snapshot
            .nodes()
            .iter()
            .find(|n| n.id == self.target.element)
            .and_then(|n| n.bounds)
            .ok_or("target_not_captured")?;
        let scale = f64::from(snapshot.viewport.scale_factor);
        let device_id = DeviceId::dummy();
        let motion = |p: Point| UiInputEvent::CursorMoved {
            device_id,
            position: PhysicalPosition::new(
                f64::from(bounds.x + p.x) * scale,
                f64::from(bounds.y + p.y) * scale,
            ),
        };
        let mut events = vec![UiInputEvent::ModifiersChanged(Modifiers::from(
            self.modifiers,
        ))];
        if !matches!(self.steps.first(), Some(UiInputStep::Move(_)))
            && self.steps.iter().any(|s| {
                matches!(
                    s,
                    UiInputStep::Move(_) | UiInputStep::Button { .. } | UiInputStep::Wheel { .. }
                )
            })
        {
            let visible = snapshot
                .nodes()
                .iter()
                .find(|n| n.id == self.target.element)
                .and_then(|n| n.visible_bounds)
                .ok_or("target_not_visible")?;
            events.push(motion(Point::new(
                visible.x + visible.width / 2.0 - bounds.x,
                visible.y + visible.height / 2.0 - bounds.y,
            )));
        }
        for step in &self.steps {
            match step {
                UiInputStep::Move(p) => events.push(motion(*p)),
                UiInputStep::Button { button, pressed } => events.push(UiInputEvent::MouseInput {
                    device_id,
                    state: if *pressed {
                        ElementState::Pressed
                    } else {
                        ElementState::Released
                    },
                    button: *button,
                }),
                UiInputStep::Wheel { x, y } => events.push(UiInputEvent::MouseWheel {
                    device_id,
                    delta: MouseScrollDelta::PixelDelta(PhysicalPosition::new(
                        f64::from(*x) * scale,
                        f64::from(*y) * scale,
                    )),
                    phase: TouchPhase::Moved,
                }),
                UiInputStep::Text(text) => events.push(UiInputEvent::Text(text.clone())),
                UiInputStep::Cancel => {
                    for state in [ElementState::Pressed, ElementState::Released] {
                        events.push(UiInputEvent::KeyboardInput {
                            device_id,
                            event: UiKeyEvent {
                                logical_key: Key::Named(NamedKey::Escape),
                                physical_key: PhysicalKey::Code(KeyCode::Escape),
                                text: None,
                                location: KeyLocation::Standard,
                                state,
                                repeat: false,
                            },
                            is_synthetic: false,
                        });
                    }
                }
                UiInputStep::Key {
                    key,
                    modifiers: desired,
                } => {
                    let desired = desired.unwrap_or(self.modifiers);
                    let (logical_key, physical_key) = command_key(key)?;
                    events.push(UiInputEvent::ModifiersChanged(Modifiers::from(desired)));
                    for state in [ElementState::Pressed, ElementState::Released] {
                        events.push(UiInputEvent::KeyboardInput {
                            device_id,
                            event: UiKeyEvent {
                                logical_key: logical_key.clone(),
                                physical_key,
                                text: if desired == ModifiersState::empty() {
                                    logical_key.to_text().map(ToString::to_string)
                                } else {
                                    None
                                },
                                location: KeyLocation::Standard,
                                state,
                                repeat: false,
                            },
                            is_synthetic: false,
                        });
                    }
                    events.push(UiInputEvent::ModifiersChanged(Modifiers::from(
                        self.modifiers,
                    )));
                }
                UiInputStep::Window(_) => return Err("window_dispatch_required"),
            }
        }
        events.push(UiInputEvent::ModifiersChanged(Modifiers::from(modifiers)));
        Ok(events)
    }
}

pub fn ui_window_state(window: &winit::window::Window) -> Value {
    let size = window.inner_size();
    let position = window.outer_position().ok();
    json!({"physical_size":{"width":size.width,"height":size.height},"outer_position":position.map(|p|json!({"x":p.x,"y":p.y})),"scale_factor":window.scale_factor(),"minimized":window.is_minimized(),"maximized":window.is_maximized(),"visible":window.is_visible(),"focused":window.has_focus()})
}
pub fn ui_input_state(
    input: &vesta_gpui::InputOwnership,
    pointer: Point,
    modifiers: ModifiersState,
) -> Value {
    use vesta_gpui::PointerButton;
    let target = |target: vesta_gpui::InputTarget| json!({"window":target.window.0.to_string(),"element":target.element.get().to_string()});
    json!({"observation":"live_logical_host_input","pointer":{"x":pointer.x,"y":pointer.y},"focus":input.focus().map(target),"capture":input.capture().map(|capture|json!({"target":target(capture.target),"button":match capture.button{PointerButton::Primary=>"primary",PointerButton::Secondary=>"secondary",PointerButton::Middle=>"middle"}})),"modifiers":{"control":modifiers.control_key(),"shift":modifiers.shift_key(),"alt":modifiers.alt_key(),"super":modifiers.super_key()}})
}
/// Close returns the existing close event; RetryRender returns the legacy F5
/// intent, which only an opted-in Host may interpret as renderer recovery.
/// Other variants use the supplied winit Window. Authorization/admission is Host-owned.
pub fn apply_ui_window_command(
    window: &winit::window::Window,
    command: &UiWindowCommand,
) -> Option<UiInputEvent> {
    match command {
        UiWindowCommand::Close => return Some(UiInputEvent::CloseRequested),
        UiWindowCommand::RetryRender => {
            return Some(UiInputEvent::KeyboardInput {
                device_id: DeviceId::dummy(),
                event: UiKeyEvent {
                    logical_key: Key::Named(NamedKey::F5),
                    physical_key: PhysicalKey::Code(KeyCode::F5),
                    text: None,
                    location: KeyLocation::Standard,
                    state: ElementState::Pressed,
                    repeat: false,
                },
                is_synthetic: false,
            });
        }
        UiWindowCommand::Minimize => window.set_minimized(true),
        UiWindowCommand::Maximize => window.set_maximized(true),
        UiWindowCommand::Restore => {
            window.set_minimized(false);
            window.set_maximized(false);
        }
        UiWindowCommand::Resize { width, height } => {
            let _ = window.request_inner_size(winit::dpi::LogicalSize::new(*width, *height));
        }
        UiWindowCommand::Move { x, y } => {
            window.set_outer_position(winit::dpi::PhysicalPosition::new(*x, *y))
        }
    }
    None
}

fn command_key(key: &str) -> Result<(Key, PhysicalKey), &'static str> {
    if let Ok(named) = serde_json::from_value::<NamedKey>(Value::String(key.into())) {
        let physical = serde_json::from_value::<KeyCode>(Value::String(key.into())).ok();
        return Ok((
            Key::Named(named),
            physical.map_or(
                PhysicalKey::Unidentified(NativeKeyCode::Unidentified),
                PhysicalKey::Code,
            ),
        ));
    }
    if key.chars().count() != 1 || key.chars().any(char::is_control) {
        return Err("unknown_key");
    }
    let code = if key.len() == 1 && key.as_bytes()[0].is_ascii_alphabetic() {
        Some(format!("Key{}", key.to_ascii_uppercase()))
    } else if key.len() == 1 && key.as_bytes()[0].is_ascii_digit() {
        Some(format!("Digit{key}"))
    } else {
        None
    };
    let physical =
        code.and_then(|code| serde_json::from_value::<KeyCode>(Value::String(code)).ok());
    Ok((
        Key::Character(key.into()),
        physical.map_or(
            PhysicalKey::Unidentified(NativeKeyCode::Unidentified),
            PhysicalKey::Code,
        ),
    ))
}

pub fn ui_input_schema() -> Value {
    let request_example = json!({"schema_version":1,"request_id":"<fresh request ID>","op":"input","instance":"<commands instance>","window":"<commands window>","element":"<commands element>","owner":"<commands owner>","revision":"<commands revision>","steps":[{"kind":"move","x":10.0,"y":10.0},{"kind":"click","button":"primary","count":1}]});
    json!({"op":"input","request_example":request_example,"target":"top-level instance/window/element/owner from commands; no nested target or input object; steps is an array tagged by kind","revision":"required decimal string; pinned source snapshot revision","source_checks":["owner","context","geometry","hit","focus","editor state"],"origin":"command","coordinates":"logical pixels relative to target bounds; wheel is logical pixel delta","modifiers":["control","shift","alt","super"],"steps":{"move":{"x":"number","y":"number"},"click":{"button":"primary|secondary|middle","count":"1..3"},"press/release":{"button":"balanced within request"},"drag":{"from":{"x":"number","y":"number"},"to":{"x":"number","y":"number"},"samples":"1..64"},"wheel":{"x":"number","y":"number"},"key":{"key":"one Unicode scalar or a pinned winit NamedKey name (including F1..F35)","modifiers":"optional override of request modifiers"},"text":{"text":"UTF-8, editable target only"},"cancel":{"target":"window root; must be the only step"},"window":{"action":"minimize|maximize|restore|close|resize|move|retry_render (Host capability)","target":"window root; must be the only step","resize":"logical width/height 64..8192","move":"physical outer x/y"}},"max_steps":MAX_SOURCE_STEPS,"max_expanded_steps":MAX_EXPANDED_STEPS,"max_text_bytes":vesta_gpui::TEXT_INPUT_MAX_BYTES,"native_child_surfaces":false,"os_dialogs":false})
}
