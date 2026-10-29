//! Public palette consumer; its first assertion preserves the short-viewport regression.
use super::*;
use vesta_gpui::{Rect, Size};
use vesta_gpui_widgets::{
    CommandPaletteEntry, CommandPaletteIds, CommandPaletteScrollIds, CommandPaletteState,
    command_palette, command_palette_geometry, command_palette_matches, command_palette_scrolled,
};

pub(super) fn qualify(context: &mut TextInputContext) -> Result<(), String> {
    let scope = ElementIdAllocator.allocate_scope().map_err(debug_error)?;
    let id = |part| WidgetIdentity::new(scope.element(part), format!("palette.{part}"));
    let mut editor = TextInputState::new(13.0, "sans-serif");
    editor.reset(context, "").map_err(debug_error)?;
    let size = Size::new(380.0, 240.0);
    let widget = command_palette(
        CommandPaletteIds {
            backdrop: id(0),
            panel: id(1),
            title: id(2),
            list: id(3),
            empty: id(4),
        },
        size,
        text_field(id(5), editor.snapshot(), true, WidgetTheme::dark()),
        (0..40)
            .map(|value| CommandPaletteEntry {
                root: id(100 + value),
                label: format!("Command {value}"),
                shortcut: String::new(),
                highlighted: value == 39,
                action: Some(Action::Select(FieldId(value))),
            })
            .collect(),
        WidgetTheme::dark(),
    );
    let layout = RetainedLayoutEngine::default()
        .compute_with_text_context(
            &widget.element,
            Viewport::new(size.width, size.height, 1.25),
            context,
        )
        .map_err(debug_error)?;
    let panel = layout.node(scope.element(1)).ok_or("missing panel")?.rect;
    let viewport = Rect::new(0.0, 0.0, size.width, size.height);
    require(
        panel.x >= viewport.x
            && panel.right() <= viewport.right()
            && panel.y >= viewport.y
            && panel.bottom() <= viewport.bottom(),
        "command palette extends beyond the short viewport",
    )?;
    qualify_center(context)
}

fn qualify_center(context: &mut TextInputContext) -> Result<(), String> {
    require(
        command_palette_matches(
            "  中文  INCREASE ",
            "Increase Sample",
            "Ctrl+I",
            "数值 中文",
        ) && command_palette_matches("ctrl+i", "Increase Sample", "Ctrl+I", "")
            && !command_palette_matches("increase missing", "Increase Sample", "", "中文"),
        "search lost Unicode/multiword/shortcut semantics",
    )?;
    let chord = |key| KeyChord::new(key, KeyModifiers::default());
    let mut frames = 0;
    for window in [UiWindowId(71), UiWindowId(73)] {
        let scope = ElementIdAllocator.allocate_scope().map_err(debug_error)?;
        let id = |part| WidgetIdentity::new(scope.element(part), format!("center.{part}"));
        let mut editor = TextInputState::new(13.0, "sans-serif");
        editor.reset(context, "").map_err(debug_error)?;
        let mut retained = RetainedLayoutEngine::default();
        for theme in [WidgetTheme::dark(), WidgetTheme::light()] {
            for size in [
                Size::new(1280.0, 900.0),
                Size::new(380.0, 240.0),
                Size::new(160.0, 140.0),
            ] {
                for count in [0_u32, 1, 40, 10_000] {
                    let keys = (0..count)
                        .map(|i| (i * 17 + 11, i % 7 != 1))
                        .collect::<Vec<_>>();
                    let geometry = command_palette_geometry(size, keys.len(), theme);
                    let mut state = CommandPaletteState::default();
                    state.reconcile(&keys, geometry.list_height);
                    for key in [
                        KeyStroke::Home,
                        KeyStroke::PageDown,
                        KeyStroke::End,
                        KeyStroke::PageUp,
                        KeyStroke::ArrowDown,
                        KeyStroke::Home,
                    ] {
                        let decision =
                            state.key_action(&keys, chord(key), false, geometry.list_height);
                        require(decision.is_some(), "palette did not own a navigation key")?;
                        let entries = keys
                            .iter()
                            .map(|(value, enabled)| CommandPaletteEntry {
                                root: id(100 + value),
                                label: format!("Command 中文 {value}"),
                                shortcut: "Ctrl+K".into(),
                                highlighted: state.highlighted() == Some(value),
                                action: enabled.then_some(Action::Select(FieldId(*value))),
                            })
                            .collect();
                        let widget = Widget::from_element(
                            Element::container(
                                scope.element(800_000),
                                ElementKey::new("center.host"),
                            )
                            .styled(
                                vesta_gpui::Style::default()
                                    .size(Dimension::Px(size.width), Dimension::Px(size.height)),
                            ),
                        )
                        .on_click(Action::Apply)
                        .child(command_palette_scrolled(
                            CommandPaletteScrollIds {
                                base: CommandPaletteIds {
                                    backdrop: id(0),
                                    panel: id(1),
                                    title: id(2),
                                    list: id(3),
                                    empty: id(4),
                                },
                                content: id(6),
                            },
                            size,
                            text_field(id(5), editor.snapshot(), true, theme),
                            entries,
                            state.scroll().offset_y(),
                            theme,
                        ));
                        let layout = retained
                            .compute_with_text_context(
                                &widget.element,
                                Viewport::new(size.width, size.height, 1.25),
                                context,
                            )
                            .map_err(debug_error)?;
                        let route = ActionRoute::build(&widget.element, widget.actions)
                            .map_err(debug_error)?;
                        let hits = HitTestTree::build(&widget.element, &layout);
                        let panel = layout.node(scope.element(1)).ok_or("panel missing")?.rect;
                        require(
                            panel.x >= 0.0
                                && panel.y >= 0.0
                                && panel.right() <= size.width
                                && panel.bottom() <= size.height,
                            "bounded palette exceeded viewport",
                        )?;
                        require(
                            route.focus_order() == [scope.element(5)],
                            "virtual options stole the editor's Tab stop",
                        )?;
                        let body = widget
                            .element
                            .find(scope.element(6))
                            .ok_or("result extent missing")?;
                        require(
                            body.children.len() <= 28,
                            "palette projected unbounded result nodes",
                        )?;
                        for &(value, enabled) in &keys {
                            let element = scope.element(100 + value);
                            if let Some(row) = widget.element.find(element) {
                                require(
                                    route.action_for_hit(element)
                                        == enabled.then_some(&Action::Select(FieldId(value))),
                                    "disabled command escaped to compound parent or stale action",
                                )?;
                                require(
                                    row.semantic.as_ref().is_some_and(|s| {
                                        s.selected == Some(state.highlighted() == Some(&value))
                                    }),
                                    "selected semantic disagrees with stable selection",
                                )?;
                            }
                        }
                        if let Some(value) =
                            state.highlighted().filter(|_| geometry.list_height >= 34.0)
                        {
                            let element = scope.element(100 + value);
                            let rect = layout
                                .node(element)
                                .ok_or("selected row was virtualized away")?
                                .rect;
                            let list = layout
                                .node(scope.element(3))
                                .ok_or("list missing")?
                                .scroll_viewport();
                            require(
                                rect.y >= list.y - 0.01 && rect.bottom() <= list.bottom() + 0.01,
                                "keyboard-selected row is outside viewport",
                            )?;
                            let hit = hits
                                .hit(Point::new(rect.x + 8.0, rect.y + rect.height / 2.0))
                                .ok_or("selected hit missing")?;
                            require(
                                route.action_for_hit(hit) == Some(&Action::Select(FieldId(*value))),
                                "visible row hit lost command identity",
                            )?;
                        }
                        let focus = FocusScope::new(window, &route, Some(scope.element(0)));
                        let mut input = InputOwnership::default();
                        focus.traverse(&mut input, false);
                        require(
                            input.focus()
                                == Some(InputTarget {
                                    window,
                                    element: scope.element(5),
                                }),
                            "palette focus escaped editor",
                        )?;
                        require(
                            focus
                                .restore_target(InputTarget {
                                    window: UiWindowId(99),
                                    element: scope.element(5),
                                })
                                .is_none(),
                            "foreign popup input accepted",
                        )?;
                        frames += 1;
                    }
                }
            }
        }
    }
    // Reordering is identity-preserving; disappearance and live disablement cannot activate
    // the old array slot. Composition and text Space must never become command execution.
    let mut state = CommandPaletteState::default();
    let a = [
        (11_u32, true),
        (29, false),
        (47, true),
        (83, true),
        (101, true),
        (119, true),
    ];
    let height = 68.0;
    state.key_action(&a, chord(KeyStroke::End), false, height);
    require(
        state.highlighted() == Some(&119) && state.scroll().offset_y() > 0.0,
        "long-list End failed",
    )?;
    let b = [(119, true), (47, true), (11, true)];
    state.reconcile(&b, height);
    require(
        state.highlighted() == Some(&119) && state.scroll().offset_y() == 0.0,
        "reordered selection did not follow stable key",
    )?;
    let c = [(119, false), (11, true), (47, true)];
    state.reconcile(&c, height);
    require(
        state.key_action(&c, chord(KeyStroke::Enter), false, height)
            == Some(ChoiceKeyAction::Confirm(11)),
        "disabled stale selection confirmed",
    )?;
    let before = state.clone();
    require(
        state
            .key_action(&c, chord(KeyStroke::Enter), true, height)
            .is_none()
            && state == before,
        "composition executed command",
    )?;
    require(
        state
            .key_action(&c, chord(KeyStroke::Space), false, height)
            .is_none(),
        "search Space executed command",
    )?;
    state.reconcile(&[], height);
    require(
        state.highlighted().is_none()
            && state.key_action(&[], chord(KeyStroke::Enter), false, height)
                == Some(ChoiceKeyAction::Consumed),
        "empty result activated old command",
    )?;
    println!(
        "Command center: {frames} retained frames; 10000-command bounds, stable updates, two sources/themes, complete navigation and empty/disabled/preedit ownership passed"
    );
    Ok(())
}
