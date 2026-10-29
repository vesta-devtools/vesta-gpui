//! Public, non-windowed qualification of complete borrowed editor state.
use super::*;
use vesta_gpui::TextInputAction;
use vesta_gpui_widgets::{
    InlineEditIds, InlineEditKeyAction, TemporaryEditorState, inline_edit_key_action,
    inline_editable_label,
};

pub(super) fn qualify(context: &mut TextInputContext) -> Result<(), String> {
    let mut frames = 0;
    for window in [UiWindowId(101), UiWindowId(103)] {
        let scope = ElementIdAllocator.allocate_scope().map_err(debug_error)?;
        let opener = InputTarget {
            window,
            element: scope.element(2),
        };
        let foreign = UiWindowId(777);
        for theme in [WidgetTheme::light(), WidgetTheme::dark()] {
            for width in [160.0, 300.0, 640.0] {
                for reason in [
                    PopupCloseReason::Explicit,
                    PopupCloseReason::Confirm,
                    PopupCloseReason::TraverseFocus,
                    PopupCloseReason::OutsidePress,
                    PopupCloseReason::FocusTransfer,
                    PopupCloseReason::OwnerRemoved,
                    PopupCloseReason::WindowUnavailable,
                ] {
                    for redo_branch in [false, true] {
                        let mut editor = TextInputState::new_multiline(17.0, "sans-serif", 80.0);
                        editor
                            .reset(context, "Search 中文 e\u{301}😀\nline2")
                            .map_err(debug_error)?;
                        editor.act(context, TextInputAction::SelectAll);
                        editor
                            .insert(context, "replacement", TextEditKind::Paste)
                            .map_err(debug_error)?;
                        editor.act(context, TextInputAction::Undo);
                        if !redo_branch {
                            editor.act(context, TextInputAction::Redo);
                            editor.act(context, TextInputAction::SelectAll);
                        }
                        let original = editor.snapshot();
                        let original_text = editor.text().to_owned();
                        let selection = editor.selected_text().map(str::to_owned);
                        let mut session = TemporaryEditorState::default();
                        require(
                            !session.exchange_in(
                                window,
                                Some(InputTarget {
                                    window: foreign,
                                    ..opener
                                }),
                                &mut editor,
                                TextInputState::default(),
                            ),
                            "foreign opener changed slot",
                        )?;
                        require(editor.snapshot() == original, "refused open changed editor")?;
                        require(
                            session.exchange_in(
                                window,
                                Some(opener),
                                &mut editor,
                                TextInputState::default(),
                            ),
                            "open failed",
                        )?;
                        editor.reset(context, "Draft 标签").map_err(debug_error)?;
                        editor.act(context, TextInputAction::SelectAll);
                        editor
                            .insert(context, "草稿😀", TextEditKind::Paste)
                            .map_err(debug_error)?;
                        if reason != PopupCloseReason::Confirm {
                            editor
                                .preedit(context, "未提交", None)
                                .map_err(debug_error)?;
                        }
                        let draft = editor.snapshot();
                        require(
                            !session.exchange_in(
                                window,
                                Some(opener),
                                &mut editor,
                                TextInputState::default(),
                            ),
                            "live session replaced suspended state",
                        )?;
                        require(editor.snapshot() == draft, "live refusal replaced draft")?;
                        require(
                            session.finish_into(foreign, reason, &mut editor).is_none(),
                            "foreign source restored slot",
                        )?;
                        require(
                            editor.snapshot() == draft && session.is_open(),
                            "foreign exit mutated session",
                        )?;
                        present(context, scope, theme, width, &editor, true)?;
                        frames += 1;
                        let exit = session
                            .finish_into(window, reason, &mut editor)
                            .ok_or("matching exit missing")?;
                        require(
                            exit.commit == (reason == PopupCloseReason::Confirm),
                            "terminal commit policy changed",
                        )?;
                        require(
                            exit.draft.snapshot() == draft,
                            "returned draft was reconstructed",
                        )?;
                        require(
                            exit.draft.is_composing() == (reason != PopupCloseReason::Confirm),
                            "draft composition leaked",
                        )?;
                        require(
                            editor.snapshot() == original && editor.is_multiline(),
                            "full original editor/configuration not restored",
                        )?;
                        require(
                            editor.selected_text() == selection.as_deref(),
                            "original selection lost",
                        )?;
                        require(
                            !editor.is_composing(),
                            "suspended editor inherited draft preedit",
                        )?;
                        require(
                            session.finish_into(window, reason, &mut editor).is_none(),
                            "duplicate exit consumed twice",
                        )?;
                        let action = if redo_branch {
                            TextInputAction::Redo
                        } else {
                            TextInputAction::Undo
                        };
                        editor.act(context, action);
                        require(
                            editor.text()
                                == if redo_branch {
                                    "replacement"
                                } else {
                                    "Search 中文 e\u{301}😀\nline2"
                                },
                            "original undo/redo chain lost",
                        )?;
                        editor.act(
                            context,
                            if redo_branch {
                                TextInputAction::Undo
                            } else {
                                TextInputAction::Redo
                            },
                        );
                        require(
                            editor.text() == original_text,
                            "original history could not return",
                        )?;
                        present(context, scope, theme, width, &editor, false)?;
                        frames += 1;
                    }
                }
            }
        }
        for modifiers in [
            KeyModifiers {
                control: true,
                ..Default::default()
            },
            KeyModifiers {
                alt: true,
                ..Default::default()
            },
            KeyModifiers {
                shift: true,
                ..Default::default()
            },
            KeyModifiers {
                super_key: true,
                ..Default::default()
            },
        ] {
            for key in [KeyStroke::Enter, KeyStroke::Escape] {
                require(
                    inline_edit_key_action(
                        opener,
                        window,
                        true,
                        false,
                        KeyChord::new(key, modifiers),
                    )
                    .is_none(),
                    "modified editing key consumed",
                )?;
            }
        }
        for (key, expected) in [
            (KeyStroke::Enter, InlineEditKeyAction::Commit),
            (KeyStroke::Escape, InlineEditKeyAction::Cancel),
        ] {
            let chord = KeyChord::new(key, KeyModifiers::default());
            require(
                inline_edit_key_action(opener, window, true, false, chord) == Some(expected),
                "plain terminal key missing",
            )?;
            require(
                inline_edit_key_action(opener, foreign, true, false, chord).is_none(),
                "foreign key accepted",
            )?;
            require(
                inline_edit_key_action(opener, window, true, true, chord).is_none(),
                "preedit key accepted",
            )?;
            require(
                inline_edit_key_action(opener, window, false, false, chord).is_none(),
                "nonediting key accepted",
            )?;
        }
    }
    // Existing open/finish consumers retain their original ownership and result shape.
    let mut legacy = TemporaryEditorState::default();
    require(
        legacy.open_in(UiWindowId(1), None, 42),
        "legacy open changed",
    )?;
    let exit = legacy
        .finish(UiWindowId(1), PopupCloseReason::Explicit)
        .ok_or("legacy finish changed")?;
    require(exit.saved == 42 && !exit.commit, "legacy result changed")?;
    println!(
        "temporary editors: {frames} retained frames; two owners/themes, three widths, seven exits, undo/redo and Unicode selection preserved"
    );
    Ok(())
}

fn present(
    context: &mut TextInputContext,
    scope: ElementScope,
    theme: WidgetTheme,
    width: f32,
    editor: &TextInputState,
    editing: bool,
) -> Result<(), String> {
    let id = |part| WidgetIdentity::new(scope.element(part), format!("temporary.{part}"));
    let view = inline_editable_label(
        InlineEditIds {
            root: id(1),
            display: id(2),
            display_label: id(3),
            controls: id(4),
            commit: id(5),
            commit_label: id(6),
            cancel: id(7),
            cancel_label: id(8),
        },
        "Label 中文",
        editing.then(|| text_field(id(9), editor.snapshot(), true, theme)),
        Action::Open(FieldId(1)),
        Action::Apply,
        Action::Select(FieldId(1)),
        theme,
    );
    let mut retained = RetainedLayoutEngine::default();
    let layout = retained
        .compute_with_text_context(&view.element, Viewport::new(width, 400.0, 1.25), context)
        .map_err(debug_error)?;
    let route = ActionRoute::build(&view.element, view.actions).map_err(debug_error)?;
    require(
        layout.node(scope.element(9)).is_some() == editing,
        "temporary input identity duplicated or missing",
    )?;
    require(
        route.action_at(scope.element(2)).is_some() != editing,
        "hidden display action remained live",
    )?;
    require(
        route.action_at(scope.element(5)).is_some() == editing,
        "commit action lifetime incorrect",
    )?;
    Ok(())
}
