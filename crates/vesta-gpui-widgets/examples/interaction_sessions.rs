//! Portable interaction consumer: caller data, two sources, public control decisions.
//! Run with an explicit host font. No Gallery, OS window or renderer is required.

use std::{collections::BTreeMap, sync::Arc};
use vesta_gpui::{
    ActionRoute, Axis, Dimension, Element, ElementId, ElementIdAllocator, ElementKey, ElementKind,
    ElementScope, FocusScope, HitTestTree, InputOwnership, InputTarget, KeyChord, KeyModifiers,
    KeyStroke, LayoutTree, Menu, MenuId, MenuItem, MenuItemId, Point, PointerButton, PointerClick,
    RetainedLayoutEngine, TextEditKind, TextInputContext, TextInputState, TextInputViews,
    TextSessionTransition, UiWindowId, Viewport, WindowTextSession,
};
use vesta_gpui_widgets::{
    ChoiceBoundary, ChoiceKeyAction, ControlState, DraftResolution, HorizontalEditSession,
    KeyRouteContext, KeyRouteLayer, KeyRouteOutcome, MenuController, MenuKeyAction,
    PointerEditOutcome, PopoverState, PopupCloseReason, ScrollAreaState, ScrollRevealViewport,
    Widget, WidgetIdentity, WidgetTheme, button, choice_key_action, popup_dismiss_for_hit,
    resolve_value_draft, route_key, scroll_reveal_path, scroll_wheel_path, text_field,
};

#[path = "interaction_sessions/breadcrumbs.rs"]
mod breadcrumbs;
#[path = "interaction_sessions/documents.rs"]
mod documents;
#[path = "interaction_sessions/icons.rs"]
mod icons;
#[path = "interaction_sessions/numeric.rs"]
mod numeric;
#[path = "interaction_sessions/palette.rs"]
mod palette;
#[path = "interaction_sessions/suggestions.rs"]
mod suggestions;
#[path = "interaction_sessions/temporary_editors.rs"]
mod temporary_editors;
#[path = "interaction_sessions/toolbars.rs"]
mod toolbars;
#[path = "interaction_sessions/trees.rs"]
mod trees;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct FieldId(u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Select(FieldId),
    Open(FieldId),
    Apply,
}

struct Frame {
    root: Element,
    route: ActionRoute<Action>,
    layout: LayoutTree,
    views: TextInputViews<FieldId>,
}

struct Source {
    window: UiWindowId,
    scope: ElementScope,
    layout: RetainedLayoutEngine,
    input: InputOwnership,
    menu: MenuController,
    popup: PopoverState,
    scroll: BTreeMap<ElementId, ScrollAreaState>,
}

impl Source {
    fn new(window: UiWindowId) -> Result<Self, String> {
        Ok(Self {
            window,
            scope: ElementIdAllocator.allocate_scope().map_err(debug_error)?,
            layout: Default::default(),
            input: Default::default(),
            menu: Default::default(),
            popup: Default::default(),
            scroll: Default::default(),
        })
    }

    fn id(&self, part: u32) -> ElementId {
        self.scope.element(part)
    }

    fn target(&self, part: u32) -> InputTarget {
        InputTarget {
            window: self.window,
            element: self.id(part),
        }
    }

    fn frame(
        &mut self,
        fields: &BTreeMap<FieldId, TextInputState>,
        context: &mut TextInputContext,
    ) -> Result<Frame, String> {
        let identity = |part| WidgetIdentity::new(self.id(part), format!("form.part.{part}"));
        let panel = |part, width, height| {
            Element::container(self.id(part), ElementKey::new(format!("panel.{part}"))).styled(
                vesta_gpui::Style::default().size(Dimension::Px(width), Dimension::Px(height)),
            )
        };
        let mut rows = Widget::from_element(panel(4, 700.0, 300.0));
        for (&field, state) in fields {
            let base = field_base(field);
            let mut editor = text_field(
                identity(base + 3),
                state.snapshot(),
                false,
                WidgetTheme::default(),
            );
            editor.style.width = Dimension::Px(140.0);
            let row = Widget::from_element(
                panel(base, 650.0, 32.0).styled(
                    vesta_gpui::Style::default()
                        .size(Dimension::Px(650.0), Dimension::Px(32.0))
                        .axis(Axis::Row)
                        .focusable(true),
                ),
            )
            .on_click(Action::Select(field))
            .child(button(
                identity(base + 1),
                identity(base + 2),
                "Choose",
                Action::Open(field),
                ControlState::Idle,
                WidgetTheme::default(),
            ))
            .child(Widget::from_element(editor));
            rows = rows.child(row);
        }
        let scroll_box = |part, width, height| {
            Element::scroll_box(
                self.id(part),
                ElementKey::new(format!("scroll.{part}")),
                self.scroll
                    .get(&self.id(part))
                    .copied()
                    .unwrap_or_default()
                    .offset(),
            )
            .styled(vesta_gpui::Style::default().size(Dimension::Px(width), Dimension::Px(height)))
        };
        let inner = Widget::from_element(scroll_box(3, 500.0, 140.0)).child(rows);
        let outer = Widget::from_element(scroll_box(2, 350.0, 180.0))
            .child(inner)
            .child(Widget::from_element(panel(5, 500.0, 260.0)));
        let mut root = Widget::from_element(panel(1, 640.0, 360.0)).child(outer);
        if self.popup.is_open() {
            root = root.child(Widget::from_element(panel(50, 200.0, 32.0)).child(button(
                identity(51),
                identity(52),
                "Apply choice",
                Action::Apply,
                ControlState::Idle,
                WidgetTheme::default(),
            )));
        }
        let layout = self
            .layout
            .compute_with_text_context(&root.element, Viewport::new(640.0, 360.0, 1.0), context)
            .map_err(debug_error)?;
        for part in [2, 3] {
            let id = self.id(part);
            self.scroll
                .entry(id)
                .or_default()
                .sync_layout(layout.node(id).ok_or("missing scroll geometry")?);
        }
        let geometry = layout
            .nodes()
            .iter()
            .filter_map(|node| match &root.element.find(node.id)?.kind {
                ElementKind::TextInput { snapshot, .. } => Some((
                    node.id,
                    snapshot.viewport(node.rect, node.inherited_clip, Point::default()),
                )),
                _ => None,
            })
            .collect::<Vec<_>>();
        let views = TextInputViews::from_layout(self.window, &geometry, |element| {
            fields
                .keys()
                .find(|&&field| self.id(field_base(field) + 3) == element)
                .copied()
        })
        .map_err(debug_error)?;
        let route = ActionRoute::build(&root.element, root.actions).map_err(debug_error)?;
        FocusScope::new(self.window, &route, None).reconcile(&mut self.input);
        Ok(Frame {
            root: root.element,
            route,
            layout,
            views,
        })
    }
}

// Stable caller identity, independent of display labels and row positions.
fn field_base(field: FieldId) -> u32 {
    100 + field.0 * 8
}
fn key(stroke: KeyStroke) -> KeyChord {
    KeyChord::new(stroke, KeyModifiers::default())
}
fn debug_error(error: impl std::fmt::Debug) -> String {
    format!("{error:?}")
}
fn require(condition: bool, failure: &str) -> Result<(), String> {
    condition.then_some(()).ok_or_else(|| failure.to_owned())
}

fn hit(frame: &Frame, element: ElementId) -> Result<ElementId, String> {
    let rect = frame
        .layout
        .node(element)
        .ok_or("missing control geometry")?
        .rect;
    HitTestTree::build(&frame.root, &frame.layout)
        .hit(Point::new(
            rect.x + rect.width / 2.0,
            rect.y + rect.height / 2.0,
        ))
        .ok_or_else(|| "control is not visible/hittable".into())
}

fn popup_and_keys(first: &mut Source, second: &Source, frame: &Frame) -> Result<(), String> {
    let field = FieldId(17);
    let opener = first.target(field_base(field) + 1);
    let label_hit = hit(frame, first.id(field_base(field) + 2))?;
    let editor_hit = hit(frame, first.id(field_base(field) + 3))?;
    require(
        frame.route.action_for_hit(editor_hit).is_none(),
        "editor gesture activated its parent row",
    )?;
    let mut click = PointerClick::default();
    require(
        click.press(
            first.window,
            PointerButton::Primary,
            &frame.route,
            Some(label_hit),
        ),
        "open press lost its child action",
    )?;
    require(
        click
            .release(
                second.window,
                PointerButton::Primary,
                &frame.route,
                Some(label_hit),
            )
            .is_none(),
        "foreign release activated a child",
    )?;
    require(
        click.release(
            first.window,
            PointerButton::Primary,
            &frame.route,
            Some(label_hit),
        ) == Some(&Action::Open(field)),
        "child click activated the row instead",
    )?;
    first.popup.open_for(opener);
    let choices = [
        FieldId(17),
        FieldId(71),
        FieldId(29),
        FieldId(53),
        FieldId(83),
    ];
    let mut chosen = None;
    let mut shortcut_count = 0;
    let context = KeyRouteContext {
        window: first.window,
        text_owner: None,
        composing: false,
        modal: false,
    };
    let outcome = route_key(context, Some(key(KeyStroke::End)), |layer| {
        if layer == KeyRouteLayer::Popup && first.popup.is_open() {
            if let Some(ChoiceKeyAction::Highlight(id)) =
                choice_key_action(&choices, None, key(KeyStroke::End), ChoiceBoundary::Clamp)
            {
                chosen = Some(id);
            }
            true
        } else if layer == KeyRouteLayer::Shortcut {
            shortcut_count += 1;
            true
        } else {
            false
        }
    });
    require(
        outcome == KeyRouteOutcome::Consumed(KeyRouteLayer::Popup)
            && chosen == Some(FieldId(83))
            && shortcut_count == 0,
        "popup navigation leaked into a shortcut",
    )?;
    require(!second.popup.is_open(), "popup state crossed sources")?;
    let text_owner = first.target(field_base(field) + 3);
    let mut callbacks = 0;
    let outcome = route_key(
        KeyRouteContext {
            text_owner: Some(text_owner),
            composing: true,
            ..context
        },
        Some(key(KeyStroke::Escape)),
        |_| {
            callbacks += 1;
            true
        },
    );
    require(
        outcome == KeyRouteOutcome::TextInput && callbacks == 0 && first.popup.is_open(),
        "composition dismissed a non-text control",
    )?;
    let outcome = route_key(
        KeyRouteContext {
            window: second.window,
            text_owner: Some(text_owner),
            ..context
        },
        None,
        |_| {
            callbacks += 1;
            true
        },
    );
    require(
        outcome == KeyRouteOutcome::InvalidTextOwner && callbacks == 0,
        "foreign editor owner reached dispatch",
    )?;
    println!(
        "child click -> five-choice popup -> End; composition and foreign source leave other controls untouched"
    );
    Ok(())
}

fn menu_and_outside_click(
    source: &mut Source,
    other: &Source,
    frame: &Frame,
) -> Result<(), String> {
    let opener = source.popup.opener().ok_or("missing popup opener")?;
    require(
        source
            .popup
            .dismiss(other.window, PopupCloseReason::Explicit)
            .is_none()
            && source.popup.is_open(),
        "foreign source dismissed popup",
    )?;
    let mut explicit = source.popup;
    let scope = FocusScope::new(source.window, &frame.route, None);
    require(
        explicit
            .dismiss(source.window, PopupCloseReason::Explicit)
            .and_then(|dismissal| dismissal.restore_target(&scope))
            == Some(opener),
        "explicit dismissal lost live opener",
    )?;
    let outside = hit(frame, source.id(field_base(FieldId(29)) + 2))?;
    require(
        popup_dismiss_for_hit(
            opener,
            source.id(50),
            source.window,
            Some(outside),
            &frame.route,
        ),
        "outside click did not dismiss popup",
    )?;
    let dismissal = source
        .popup
        .dismiss(source.window, PopupCloseReason::OutsidePress)
        .ok_or("own popup dismissal rejected")?;
    require(
        dismissal.restore_target(&scope).is_none()
            && source
                .popup
                .dismiss(source.window, PopupCloseReason::Explicit)
                .is_none(),
        "outside dismissal stole focus or repeated its close",
    )?;
    let restored = scope.restore_target(source.target(field_base(FieldId(29)) + 1));
    require(restored.is_some(), "outside action's control cannot focus")?;
    source.input.set_focus(restored);
    require(
        frame.route.action_for_hit(outside) == Some(&Action::Open(FieldId(29))),
        "dismissal lost the original outside action",
    )?;
    let mut menu = Menu::new(MenuId(1), "Actions")
        .item(MenuItem::new(MenuItemId(10), "Apply", Action::Apply))
        .item(MenuItem::new(MenuItemId(20), "Unavailable", Action::Apply).enabled(false));
    source.menu.open_in(source.window, menu.id, restored);
    require(
        source
            .menu
            .dismiss(other.window, PopupCloseReason::Explicit)
            .is_none()
            && source.menu.is_open(),
        "foreign source dismissed menu",
    )?;
    let _ = source.menu.key_action(&menu, key(KeyStroke::End));
    require(
        source.menu.key_action(&menu, key(KeyStroke::Enter))
            == MenuKeyAction::Activate(MenuItemId(10)),
        "menu activated a disabled descriptor",
    )?;
    menu.items[0].enabled = false;
    source.menu.reconcile(std::slice::from_ref(&menu));
    require(
        source.menu.key_action(&menu, key(KeyStroke::Enter)) == MenuKeyAction::Consumed,
        "stale menu highlight activated replacement data",
    )?;
    let menu_return = source
        .menu
        .dismiss(source.window, PopupCloseReason::Confirm)
        .and_then(|dismissal| dismissal.restore_target(&scope));
    source.input.set_focus(menu_return);
    require(
        source.input.focus() == restored,
        "menu dismissal lost live focus",
    )?;
    println!(
        "outside dismissal preserves original child action; disabling the highlighted menu item prevents activation"
    );
    Ok(())
}

fn scrolling(source: &mut Source, other: &Source, frame: &Frame) -> Result<(), String> {
    let target = source.target(field_base(FieldId(17)) + 3);
    let scope = FocusScope::new(source.window, &frame.route, None);
    let movement = Point::new(400.0, 450.0);
    let mut visited = Vec::new();
    let remaining = scroll_wheel_path(&scope, target, movement, |id, delta| {
        if let Some(state) = source.scroll.get_mut(&id) {
            visited.push(id);
            state.scroll_by_point(delta)
        } else {
            delta
        }
    });
    require(
        visited == [source.id(3), source.id(2)],
        "nested wheel did not visit inner then outer",
    )?;
    for part in [2, 3] {
        let state = source
            .scroll
            .get(&source.id(part))
            .ok_or("missing scroll state")?;
        require(
            state.offset() == state.max() && state.max().x > 0.0 && state.max().y > 0.0,
            "dual-axis scroll did not reach actual layout limits",
        )?;
    }
    require(
        remaining.x > 0.0 && remaining.y > 0.0,
        "excess wheel movement was silently lost",
    )?;
    let before = source.scroll.clone();
    let foreign = InputTarget {
        window: other.window,
        ..target
    };
    let mut touched = false;
    let returned = scroll_wheel_path(&scope, foreign, movement, |_, delta| {
        touched = true;
        delta
    });
    require(
        returned == movement && !touched && source.scroll == before,
        "foreign wheel changed this window",
    )?;
    require(
        other
            .scroll
            .values()
            .all(|state| state.offset() == Point::default()),
        "source scrolling changed the other projection",
    )?;
    println!(
        "real nested layout consumes both axes inner-to-outer and returns excess; other source remains at origin"
    );
    Ok(())
}

fn reveal_editor(
    source: &mut Source,
    fields: &BTreeMap<FieldId, TextInputState>,
    context: &mut TextInputContext,
) -> Result<(), String> {
    let scrolled = source.frame(fields, context)?;
    let target = source.target(field_base(FieldId(17)) + 3);
    let scope = FocusScope::new(source.window, &scrolled.route, None);
    let path = scope.path_to(target).ok_or("missing current editor path")?;
    let target_rect = scrolled
        .layout
        .node(target.element)
        .ok_or("missing editor")?
        .rect;
    let steps = scroll_reveal_path(&path, target_rect, |element| {
        source
            .scroll
            .contains_key(&element)
            .then(|| {
                scrolled
                    .layout
                    .node(element)
                    .map(|node| ScrollRevealViewport {
                        rect: node.scroll_viewport(),
                        margin: 0.0,
                    })
            })
            .flatten()
    });
    require(steps.len() == 2, "reveal lost a nested viewport")?;
    for step in steps {
        source
            .scroll
            .get_mut(&step.element)
            .ok_or("missing reveal state")?
            .reveal(step.target, step.viewport.rect, step.viewport.margin);
    }
    source.input.request_focus(target);
    let revealed = source.frame(fields, context)?;
    let resolved = FocusScope::new(source.window, &revealed.route, None)
        .resolve_request(&mut source.input)
        .ok_or("missing deferred focus")?;
    source.input.set_focus(resolved);
    require(
        source.input.focus() == Some(target) && hit(&revealed, target.element)? == target.element,
        "revealed editor is not the actual focused hit",
    )?;
    println!(
        "deferred focus rebuilds and reveals the editor through both live scroll viewports; actual hit reaches the editor"
    );
    Ok(())
}

fn transfer_and_edit(
    first: &Source,
    second: &Source,
    a: &Frame,
    b: &Frame,
    fields: &mut BTreeMap<FieldId, TextInputState>,
    context: &mut TextInputContext,
) -> Result<(), String> {
    let field = FieldId(17);
    let old = WindowTextSession {
        window: first.window,
        field,
    };
    let next = WindowTextSession {
        window: second.window,
        field,
    };
    let transfer = TextSessionTransition::new(Some(old), Some(next));
    require(
        !transfer.fields_changed() && transfer.ime_owner_to_end() == Some(first.window),
        "same field transfer reinitialized its draft or left old IME ownership",
    )?;
    require(
        a.views.field_at(first.target(field_base(field) + 3)) == Some(field)
            && b.views.field_at(second.target(field_base(field) + 3)) == Some(field),
        "current source geometry lost typed editor identity",
    )?;
    require(
        a.views
            .field_at(InputTarget {
                window: second.window,
                ..first.target(field_base(field) + 3)
            })
            .is_none(),
        "editor geometry accepted a foreign source",
    )?;
    let editor = fields.get_mut(&field).ok_or("missing caller editor")?;
    editor.end_group();
    editor
        .insert(context, " / transferred", TextEditKind::Typing)
        .map_err(debug_error)?;
    require(
        editor.text() == "Ready / transferred",
        "transfer lost shared content",
    )?;
    let owner = second.target(field_base(field) + 3);
    let mut gesture = HorizontalEditSession::new(owner, 0.0, 10_i32, 4.0).map_err(debug_error)?;
    require(
        gesture.offset(first.window, 100.0).is_none(),
        "foreign pointer changed edit preview",
    )?;
    let delta = gesture
        .offset(second.window, 20.0)
        .ok_or("missing edit preview")?;
    let preview = 10 + delta as i32;
    let mut model_value = preview;
    require(
        gesture.finish(first.window, &preview, true).is_none(),
        "foreign release committed edit",
    )?;
    let intent = gesture
        .finish(second.window, &preview, false)
        .ok_or("missing cancel intent")?;
    require(
        intent == PointerEditOutcome::Cancel { restore: 10 }
            && gesture.finish(second.window, &preview, true).is_none(),
        "cancel did not restore once",
    )?;
    if let PointerEditOutcome::Cancel { restore } = intent {
        model_value = restore;
    }
    require(model_value == 10, "caller did not apply cancellation")?;
    require(
        resolve_value_draft("invalid", &10_i32, |raw| raw.parse().ok())
            == DraftResolution::Rejected { restore: 10 },
        "invalid draft overwrote model",
    )?;
    let mut committed = HorizontalEditSession::new(owner, 0.0, 10_i32, 0.0).map_err(debug_error)?;
    let _ = committed.offset(second.window, 20.0);
    let commit = committed
        .finish(second.window, &30, true)
        .ok_or("missing commit")?;
    let mut transactions = Vec::new();
    if let PointerEditOutcome::Commit { before, after } = commit {
        require(model_value == before, "typed commit lost original snapshot")?;
        model_value = after;
        transactions.push((before, after));
    }
    require(
        model_value == 30
            && transactions == [(10, 30)]
            && committed
                .finish(second.window, &model_value, true)
                .is_none(),
        "caller committed more than once",
    )?;
    println!(
        "same typed editor transfers without losing content; foreign edits reject, cancellation restores, one commit emits original/new values"
    );
    Ok(())
}

fn main() -> Result<(), String> {
    let usage = "usage: interaction_sessions <host-font.ttf> [--documents-only]";
    let mut args = std::env::args_os().skip(1);
    let font = args.next().ok_or(usage)?;
    let documents_only = match args.next() {
        Some(argument) if argument == "--documents-only" => true,
        None => false,
        _ => return Err(usage.into()),
    };
    if args.next().is_some() {
        return Err(usage.into());
    }
    let mut context = TextInputContext::default();
    require(
        context.register_font(Arc::new(std::fs::read(font).map_err(debug_error)?)),
        "invalid host font",
    )?;
    if documents_only {
        return documents::qualify(&mut context);
    }
    let mut fields = BTreeMap::new();
    for id in [17, 29, 53] {
        let mut editor = TextInputState::new(13.0, "sans-serif");
        editor.reset(&mut context, "Ready").map_err(debug_error)?;
        fields.insert(FieldId(id), editor);
    }
    let mut first = Source::new(UiWindowId(11))?;
    let mut second = Source::new(UiWindowId(23))?;
    let a = first.frame(&fields, &mut context)?;
    let b = second.frame(&fields, &mut context)?;
    popup_and_keys(&mut first, &second, &a)?;
    let popup_frame = first.frame(&fields, &mut context)?;
    let modal = FocusScope::new(first.window, &popup_frame.route, Some(first.id(50)));
    modal.traverse(&mut first.input, false);
    require(
        first.input.focus() == Some(first.target(51))
            && modal
                .restore_target(first.target(field_base(FieldId(17)) + 1))
                .is_none(),
        "modal navigation/restoration escaped popup",
    )?;
    menu_and_outside_click(&mut first, &second, &popup_frame)?;
    scrolling(&mut first, &second, &a)?;
    reveal_editor(&mut first, &fields, &mut context)?;
    transfer_and_edit(&first, &second, &a, &b, &mut fields, &mut context)?;
    let removed = first.target(field_base(FieldId(17)) + 3);
    first.input.set_focus(Some(removed));
    first.input.capture_pointer(removed, PointerButton::Primary);
    second
        .input
        .set_focus(Some(second.target(field_base(FieldId(29)) + 3)));
    fields.remove(&FieldId(17));
    let removed_frame = first.frame(&fields, &mut context)?;
    let retained_frame = second.frame(&fields, &mut context)?;
    require(
        first.input.focus().is_none()
            && first.input.capture().is_none()
            && removed_frame.views.get(FieldId(17)).is_none(),
        "deleted editor retained focus/capture/geometry",
    )?;
    require(
        second.input.focus() == Some(second.target(field_base(FieldId(29)) + 3))
            && retained_frame.views.get(FieldId(29)).is_some(),
        "deletion cleared surviving foreign control",
    )?;
    println!(
        "removed editor releases focus/capture/geometry; surviving control in the other source stays focused"
    );
    palette::qualify(&mut context)?;
    suggestions::qualify(&mut context)?;
    temporary_editors::qualify(&mut context)?;
    trees::qualify(&mut context)?;
    breadcrumbs::qualify(&mut context)?;
    toolbars::qualify(&mut context)?;
    icons::qualify(&mut context)?;
    numeric::qualify(&mut context)?;
    documents::qualify(&mut context)?;
    println!(
        "Portable interaction sequence passed. OS IME, held gestures, DPI and reader speech are separate acceptance."
    );
    Ok(())
}
