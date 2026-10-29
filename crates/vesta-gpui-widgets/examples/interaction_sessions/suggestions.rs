//! Real public composition and delayed-provider ordering over two independent sources.
use super::*;
use vesta_gpui_widgets::{
    EditableComboIds, SuggestionError, SuggestionOption, SuggestionPhase, SuggestionRequestId,
    SuggestionState, SuggestionViewIds, editable_combo_controlled,
};

pub(super) fn qualify(context: &mut TextInputContext) -> Result<(), String> {
    let mut frames = 0;
    for window in [UiWindowId(81), UiWindowId(83)] {
        let scope = ElementIdAllocator.allocate_scope().map_err(debug_error)?;
        let id = |part| WidgetIdentity::new(scope.element(part), format!("suggestion.{part}"));
        let owner = InputTarget {
            window,
            element: scope.element(5),
        };
        let foreign = InputTarget {
            window: UiWindowId(99),
            element: owner.element,
        };
        let mut editor = TextInputState::new(13.0, "sans-serif");
        editor.reset(context, "terrain 中文").map_err(debug_error)?;
        let original = editor.text().to_owned();
        let mut retained = RetainedLayoutEngine::default();
        for theme in [WidgetTheme::light(), WidgetTheme::dark()] {
            for width in [160.0, 300.0, 640.0] {
                for count in [0_u32, 1, 40, 10_000] {
                    let mut state = SuggestionState::default();
                    let first = state
                        .begin(owner, SuggestionRequestId(1), "old".into(), false)
                        .map_err(debug_error)?;
                    let current = state
                        .begin(owner, SuggestionRequestId(2), original.clone(), false)
                        .map_err(debug_error)?;
                    require(
                        !state.resolve(&first, Ok(Vec::new())).map_err(debug_error)?,
                        "stale result replaced current query",
                    )?;
                    let items = (0..count)
                        .map(|i| SuggestionOption {
                            id: i * 17 + 11,
                            label: format!("Terrain 中文 {i}"),
                            enabled: i % 7 != 1,
                        })
                        .collect::<Vec<_>>();
                    state.resolve(&current, Ok(items)).map_err(debug_error)?;
                    state.sync_viewport(112.0);
                    for key in [
                        KeyStroke::ArrowDown,
                        KeyStroke::PageDown,
                        KeyStroke::End,
                        KeyStroke::PageUp,
                        KeyStroke::Home,
                        KeyStroke::ArrowUp,
                    ] {
                        let chord = KeyChord::new(
                            key,
                            KeyModifiers {
                                alt: matches!(key, KeyStroke::Home | KeyStroke::End),
                                ..Default::default()
                            },
                        );
                        require(
                            state.key_action(owner, chord, false).is_some(),
                            "list navigation leaked to editor",
                        )?;
                        let mut input = text_field(id(5), editor.snapshot(), true, theme);
                        input.style.width = Dimension::Percent(1.0);
                        let view = editable_combo_controlled(
                            SuggestionViewIds {
                                combo: EditableComboIds {
                                    root: id(0),
                                    list: id(1),
                                    empty: id(2),
                                },
                                content: id(3),
                            },
                            Widget::from_element(input),
                            &state,
                            112.0,
                            theme,
                            |option| {
                                (
                                    id(100 + option.id),
                                    Some(Action::Select(FieldId(option.id))),
                                )
                            },
                        )
                        .map_err(debug_error)?;
                        let widget = Widget::from_element(
                            Element::container(
                                scope.element(900_000),
                                ElementKey::new("suggestion.host"),
                            )
                            .styled(
                                vesta_gpui::Style::default()
                                    .size(Dimension::Px(width), Dimension::Px(240.0)),
                            ),
                        )
                        .on_click(Action::Apply)
                        .child(view);
                        let layout = retained
                            .compute_with_text_context(
                                &widget.element,
                                Viewport::new(width, 240.0, 1.25),
                                context,
                            )
                            .map_err(debug_error)?;
                        let route = ActionRoute::build(&widget.element, widget.actions)
                            .map_err(debug_error)?;
                        require(
                            route.focus_order() == [owner.element],
                            "candidate stole editor tab stop",
                        )?;
                        let hits = HitTestTree::build(&widget.element, &layout);
                        let list = layout
                            .node(scope.element(1))
                            .ok_or("suggestion viewport missing")?
                            .scroll_viewport();
                        let body = widget
                            .element
                            .find(scope.element(3))
                            .ok_or("suggestion extent missing")?;
                        require(body.children.len() <= 8, "unbounded suggestion rows")?;
                        require(
                            list.right() <= width + 0.01,
                            "suggestion list overflowed narrow caller",
                        )?;
                        for child in &body.children {
                            let Some(option) = state
                                .options()
                                .iter()
                                .find(|o| scope.element(100 + o.id) == child.id)
                            else {
                                return Err("foreign suggestion row".into());
                            };
                            let expected =
                                option.enabled.then_some(Action::Select(FieldId(option.id)));
                            require(
                                route.action_for_hit(child.id) == expected.as_ref(),
                                "disabled candidate escaped to compound parent",
                            )?;
                            let rect = layout.node(child.id).ok_or("row layout missing")?.rect;
                            if rect.y >= list.y && rect.bottom() <= list.bottom() {
                                let hit = hits
                                    .hit(Point::new(rect.x + 8.0, rect.y + 14.0))
                                    .ok_or("row hit missing")?;
                                require(
                                    route.action_for_hit(hit) == expected.as_ref(),
                                    "actual candidate hit lost stable action",
                                )?;
                            }
                        }
                        if let Some(selected) = state.highlighted() {
                            let rect = layout
                                .node(scope.element(100 + *selected))
                                .ok_or("selected row virtualized away")?
                                .rect;
                            require(
                                rect.y >= list.y - 0.01 && rect.bottom() <= list.bottom() + 0.01,
                                "selection was not revealed",
                            )?;
                        }
                        let before = state.clone();
                        require(
                            state.key_action(foreign, chord, false).is_none(),
                            "foreign editor navigated candidates",
                        )?;
                        require(state == before, "foreign key changed source state")?;
                        frames += 1;
                    }
                    require(
                        editor.text() == original,
                        "candidate navigation changed text/history",
                    )?;
                    require(
                        state
                            .key_action(
                                owner,
                                KeyChord::new(KeyStroke::Space, KeyModifiers::default()),
                                false,
                            )
                            .is_none(),
                        "Space accepted candidate",
                    )?;
                    require(
                        state
                            .key_action(
                                owner,
                                KeyChord::new(KeyStroke::Enter, KeyModifiers::default()),
                                true,
                            )
                            .is_none(),
                        "preedit accepted candidate",
                    )?;
                    let selected = state.highlighted().copied();
                    if let Some(selected) = selected {
                        require(
                            state.accept(foreign, &current, &selected, false).is_none(),
                            "foreign candidate accepted",
                        )?;
                        require(
                            state.accept(owner, &first, &selected, false).is_none(),
                            "old ticket accepted",
                        )?;
                        require(
                            state.accept(owner, &current, &selected, true).is_none(),
                            "composing value accepted",
                        )?;
                        require(
                            state.accept(owner, &current, &selected, false).is_some(),
                            "current candidate rejected",
                        )?;
                        require(
                            state.accept(owner, &current, &selected, false).is_none(),
                            "candidate accepted twice",
                        )?;
                    }
                }
            }
        }
        let mut state = SuggestionState::default();
        let ticket = state
            .begin(owner, SuggestionRequestId(10), "load".into(), false)
            .map_err(debug_error)?;
        require(
            state.phase() == SuggestionPhase::Loading,
            "loading phase missing",
        )?;
        let loading = state.clone();
        let options = vec![
            SuggestionOption {
                id: 1_u32,
                label: "a".into(),
                enabled: true,
            },
            SuggestionOption {
                id: 1,
                label: "duplicate".into(),
                enabled: true,
            },
        ];
        require(
            state.resolve(&ticket, Ok(options)) == Err(SuggestionError::DuplicateCandidate),
            "duplicate stable IDs accepted",
        )?;
        require(
            state.phase() == SuggestionPhase::Loading && state.options().is_empty(),
            "invalid response partially committed",
        )?;
        state
            .resolve(&ticket, Err("Provider unavailable".into()))
            .map_err(debug_error)?;
        require(
            state.phase() == SuggestionPhase::Failed,
            "failure was shown as empty/ready",
        )?;
        let failed = state.clone();
        require(
            !state
                .resolve(&ticket, Ok(Vec::new()))
                .map_err(debug_error)?,
            "late success overwrote failure",
        )?;
        require(state.cancel(owner), "owner cancel failed")?;
        let closed = state.clone();
        require(
            !state
                .resolve(&ticket, Ok(Vec::new()))
                .map_err(debug_error)?,
            "cancelled request revived",
        )?;
        require(
            state.begin(owner, SuggestionRequestId(10), "reuse".into(), false)
                == Err(SuggestionError::ReusedRequestId),
            "retired ID reused",
        )?;
        require(
            state.begin(owner, SuggestionRequestId(11), "preedit".into(), true)
                == Err(SuggestionError::Composing),
            "preedit started request",
        )?;
        let request = state
            .begin(owner, SuggestionRequestId(12), "stable".into(), false)
            .map_err(debug_error)?;
        let data = |disabled: bool| {
            [50_u32, 20, 90]
                .into_iter()
                .map(|id| SuggestionOption {
                    id,
                    label: format!("Choice {id}"),
                    enabled: !disabled || id != 20,
                })
                .collect()
        };
        state
            .resolve(&request, Ok(data(false)))
            .map_err(debug_error)?;
        state.key_action(
            owner,
            KeyChord::new(KeyStroke::ArrowDown, KeyModifiers::default()),
            false,
        );
        require(
            state.highlighted() == Some(&20),
            "initial stable highlight missing",
        )?;
        let next = state
            .begin(owner, SuggestionRequestId(13), "reordered".into(), false)
            .map_err(debug_error)?;
        let mut ordered: Vec<_> = data(false);
        ordered.reverse();
        state.resolve(&next, Ok(ordered)).map_err(debug_error)?;
        require(
            state.highlighted() == Some(&20),
            "reorder followed an array slot",
        )?;
        let next = state
            .begin(owner, SuggestionRequestId(14), "availability".into(), false)
            .map_err(debug_error)?;
        state.resolve(&next, Ok(data(true))).map_err(debug_error)?;
        require(
            state.highlighted() == Some(&50),
            "disabled highlighted identity was retained",
        )?;
        let mut foreign_response = next.clone();
        foreign_response.owner = foreign;
        require(
            !state
                .resolve(&foreign_response, Ok(Vec::new()))
                .map_err(debug_error)?,
            "foreign response applied",
        )?;
        let mut disabled = SuggestionState::default();
        let request = disabled
            .begin(owner, SuggestionRequestId(1), "disabled".into(), false)
            .map_err(debug_error)?;
        disabled
            .resolve(
                &request,
                Ok(vec![SuggestionOption {
                    id: 1_u32,
                    label: "Unavailable".into(),
                    enabled: false,
                }]),
            )
            .map_err(debug_error)?;
        for state in [loading, failed, closed, disabled] {
            let mut editor_element =
                text_field(id(5), editor.snapshot(), true, WidgetTheme::dark());
            editor_element.style.width = Dimension::Percent(1.0);
            let view = editable_combo_controlled(
                SuggestionViewIds {
                    combo: EditableComboIds {
                        root: id(0),
                        list: id(1),
                        empty: id(2),
                    },
                    content: id(3),
                },
                Widget::from_element(editor_element),
                &state,
                56.0,
                WidgetTheme::dark(),
                |option| {
                    (
                        id(100 + option.id),
                        Some(Action::Select(FieldId(option.id))),
                    )
                },
            )
            .map_err(debug_error)?;
            let layout = retained
                .compute_with_text_context(
                    &view.element,
                    Viewport::new(240.0, 160.0, 1.25),
                    context,
                )
                .map_err(debug_error)?;
            let route = ActionRoute::build(&view.element, view.actions).map_err(debug_error)?;
            require(
                route.focus_order() == [owner.element],
                "status projection changed editor focus contract",
            )?;
            require(
                view.element
                    .semantic
                    .as_ref()
                    .is_some_and(|s| s.busy == (state.phase() == SuggestionPhase::Loading)),
                "loading semantics missing",
            )?;
            require(
                layout.node(scope.element(1)).is_some()
                    == (state.phase() != SuggestionPhase::Closed),
                "closed projection retained list",
            )?;
            if let Some(message) = state.failure() {
                require(
                    view.element
                        .find(scope.element(2))
                        .is_some_and(|e| e.semantic.as_ref().is_some_and(|s| s.label == message)),
                    "provider failure message disappeared",
                )?;
            }
            frames += 1;
        }
    }
    println!(
        "suggestions: {frames} actual public retained frames; two source lifetimes and delayed-response refusal qualified"
    );
    Ok(())
}
