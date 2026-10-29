use super::*;

fn input(value: &str) -> (TextInputContext, TextInputState) {
    let mut context = TextInputContext::default();
    let bytes = include_bytes!("../tests/fixtures/MaterialIcons-Regular.ttf");
    assert!(context.register_font(Arc::new(bytes.to_vec())));
    let mut state = TextInputState::new(13.0, "Material Icons");
    state.reset(&mut context, value).expect("initial text");
    (context, state)
}

#[test]
fn marked_references_follow_utf8_edits_ime_and_atomic_undo_without_parsing_labels() {
    let (mut context, mut state) = input("中😀");
    state
        .insert_mark(&mut context, "image-one", "[图1]")
        .expect("reference");
    state
        .insert(&mut context, "还有", TextEditKind::Typing)
        .expect("text");
    state
        .insert_mark(&mut context, "image-two", "[图2]")
        .expect("second");
    assert_eq!(state.text(), "中😀[图1]还有[图2]");
    assert_eq!(state.marks()[0].range, "中😀".len().."中😀[图1]".len());
    let original = state.clone();
    state.act(&mut context, TextInputAction::TextStart { extend: false });
    state.preedit(&mut context, "候选", None).expect("preedit");
    assert_eq!(state.text(), original.text());
    assert_eq!(state.marks(), original.marks());
    state.cancel_compose(&mut context);
    state.commit_ime(&mut context, "前").expect("commit");
    assert_eq!(state.marks()[0].range.start, "前中😀".len());
    state.act(&mut context, TextInputAction::Undo);
    assert_eq!(state.text(), original.text());
    assert_eq!(state.marks(), original.marks());
    state.act(&mut context, TextInputAction::TextEnd { extend: false });
    state.act(&mut context, TextInputAction::Backspace { word: false });
    assert_eq!(state.text(), "中😀[图1]还有");
    assert_eq!(
        state.marks().len(),
        1,
        "one Backspace removes the entire reference"
    );
    state.act(&mut context, TextInputAction::Undo);
    assert_eq!(state.marks(), original.marks());
    state.act(&mut context, TextInputAction::Redo);
    assert_eq!(state.marks().len(), 1);
    state
        .insert(&mut context, "[图2]", TextEditKind::Paste)
        .expect("ordinary text");
    assert_eq!(
        state.marks().len(),
        1,
        "typing a label does not attach an image"
    );
    state.remove_mark(&mut context, "image-one");
    assert!(state.marks().is_empty());
    state.act(&mut context, TextInputAction::Undo);
    assert_eq!(state.marks()[0].id, "image-one");
    state.act(&mut context, TextInputAction::SelectAll);
    state
        .insert(&mut context, "replacement", TextEditKind::Paste)
        .expect("replace");
    assert!(state.marks().is_empty());
    state.act(&mut context, TextInputAction::Undo);
    assert_eq!(state.marks()[0].id, "image-one");
    state.reset(&mut context, "[图1]").expect("owner reset");
    assert!(state.marks().is_empty());
    assert_eq!(state.retained_mark_ids().count(), 0);
}

#[test]
fn read_only_source_selection_rejects_every_content_edit_path() {
    let (mut context, _) = input("");
    let source = "Ae\u{301}👨‍👩‍👧‍👦\t中文\r\nمرحبا";
    let mut state = TextInputState::new_read_only(13.0, "Material Icons", Some(140.0));
    state.reset(&mut context, source).expect("owner source");
    assert_eq!(state.text(), source, "copy keeps tabs and original CRLF");
    state.act(&mut context, TextInputAction::SelectAll);
    assert_eq!(state.selected_text(), Some(source));
    assert!(!state.snapshot().0.selection.is_empty());
    let before = state.clone();
    for action in [
        TextInputAction::Backspace { word: false },
        TextInputAction::Backspace { word: true },
        TextInputAction::Delete { word: false },
        TextInputAction::Delete { word: true },
        TextInputAction::Undo,
        TextInputAction::Redo,
    ] {
        assert!(!state.act(&mut context, action));
        assert_eq!(state, before);
    }
    for kind in [TextEditKind::Typing, TextEditKind::Paste, TextEditKind::Ime] {
        assert_eq!(
            state.insert(&mut context, "replace", kind),
            Err(TextInputError::ReadOnly)
        );
    }
    assert_eq!(
        state.replace_accessibility(&mut context, "replace"),
        Err(TextInputError::ReadOnly)
    );
    assert_eq!(
        state.preedit(&mut context, "候选", None),
        Err(TextInputError::ReadOnly)
    );
    assert_eq!(
        state.commit_ime(&mut context, "提交"),
        Err(TextInputError::ReadOnly)
    );
    assert_eq!(
        state, before,
        "rejected edit leaves the selected source intact"
    );
    assert!(state.undo.is_empty() && state.redo.is_empty() && !state.is_composing());

    state.act(&mut context, TextInputAction::TextStart { extend: false });
    state.act(
        &mut context,
        TextInputAction::Right {
            extend: false,
            word: false,
        },
    );
    state.act(
        &mut context,
        TextInputAction::Right {
            extend: true,
            word: false,
        },
    );
    assert_eq!(state.selected_text(), Some("e\u{301}"));
    let caret = state.snapshot().0.caret.expect("selection geometry");
    state.point(
        &mut context,
        Point::new(caret.x, caret.y + caret.height / 2.0),
        false,
        false,
    );
    assert_eq!(state.text(), source);
    assert!(
        state
            .apply_accessibility_selection(
                &mut context,
                TextInputSelection {
                    anchor: 1,
                    focus: 4
                }
            )
            .is_some()
    );
    assert_eq!(state.selected_text(), Some("e\u{301}"));
    assert_eq!(
        state.apply_accessibility_selection(
            &mut context,
            TextInputSelection {
                anchor: 1,
                focus: 2
            }
        ),
        None,
        "assistive selection cannot split the combining grapheme"
    );
    for action in [
        TextInputAction::Right {
            extend: false,
            word: false,
        },
        TextInputAction::Left {
            extend: true,
            word: false,
        },
    ] {
        for _ in 0..source.graphemes(true).count() * 2 {
            state.act(&mut context, action);
            assert!(state.read_only_boundary(state.editor.raw_selection().anchor().index()));
            assert!(state.read_only_boundary(state.editor.raw_selection().focus().index()));
            assert_eq!(state.text(), source);
        }
    }

    let mut node = accesskit::Node::new(accesskit::Role::MultilineTextInput);
    node.add_action(accesskit::Action::SetValue);
    let mut update = accesskit::TreeUpdate {
        nodes: Vec::new(),
        tree: None,
        focus: accesskit::NodeId(1),
    };
    let mut next = 2;
    state.accessibility(
        &mut context,
        &mut update,
        &mut node,
        || {
            let id = accesskit::NodeId(next);
            next += 1;
            id
        },
        Point::default(),
    );
    assert!(node.is_read_only());
    assert!(!node.supports_action(accesskit::Action::SetValue));
    assert_eq!(node.value(), Some(source));
    assert_eq!(
        state.reset(&mut context, &"x".repeat(TEXT_INPUT_MAX_BYTES + 1)),
        Err(TextInputError::TooLong)
    );
    assert_eq!(state.text(), source);
    state
        .reset(&mut context, "new owner source")
        .expect("explicit owner update");
    assert_eq!(state.text(), "new owner source");
    assert!(state.is_read_only());
}

#[test]
fn password_projection_preserves_unicode_selection_history_and_private_values() {
    let (mut context, _) = input("");
    let mut state = TextInputState::new_password(13.0, "Material Icons", 32);
    state.reset(&mut context, "Ae\u{301}😀ב").expect("password");
    let masked = state.snapshot();
    state.refresh(&mut context);
    assert_eq!(state.snapshot(), masked);
    assert_eq!(state.masked_value(), "••••");
    assert!(!format!("{state:?}").contains(state.text()));
    state.act(&mut context, TextInputAction::TextStart { extend: false });
    state.act(
        &mut context,
        TextInputAction::Right {
            extend: false,
            word: false,
        },
    );
    state.act(
        &mut context,
        TextInputAction::Right {
            extend: true,
            word: false,
        },
    );
    assert_eq!(state.selected_text(), Some("e\u{301}"));
    assert!(!state.snapshot().0.selection.is_empty());
    state
        .insert(&mut context, "中", TextEditKind::Paste)
        .expect("selection replacement");
    assert_eq!(state.text(), "A中😀ב");
    state.act(&mut context, TextInputAction::Undo);
    assert_eq!(state.text(), "Ae\u{301}😀ב");
    assert_eq!(state.selected_text(), Some("e\u{301}"));
    state.act(&mut context, TextInputAction::Redo);
    assert_eq!(state.text(), "A中😀ב");
    let before = state.clone();
    assert_eq!(
        state.insert(&mut context, &"x".repeat(33), TextEditKind::Paste),
        Err(TextInputError::TooLong)
    );
    assert_eq!(state, before);
    assert_eq!(
        state.preedit(&mut context, &"x".repeat(33), None),
        Err(TextInputError::TooLong)
    );
    assert_eq!(state, before);
    assert_eq!(
        state.reset(&mut context, &"x".repeat(33)),
        Err(TextInputError::TooLong)
    );
    assert_eq!(state, before);
    assert_eq!(
        state.replace_accessibility(&mut context, &"x".repeat(33)),
        Err(TextInputError::TooLong)
    );
    assert_eq!(
        state, before,
        "rejected platform replacement preserves selection and history"
    );
    state.act(&mut context, TextInputAction::SelectAll);
    let mut node = accesskit::Node::new(accesskit::Role::PasswordInput);
    node.set_value(state.masked_value());
    let mut update = accesskit::TreeUpdate {
        nodes: Vec::new(),
        tree: None,
        focus: accesskit::NodeId(1),
    };
    state.accessibility(
        &mut context,
        &mut update,
        &mut node,
        || panic!("no secret text runs"),
        Point::default(),
    );
    assert!(update.nodes.is_empty());
    assert_eq!(node.value(), Some("••••"));
    let mask_end = state.snapshot().0.caret.expect("masked caret");
    state.point(
        &mut context,
        Point::new(mask_end.x, mask_end.y + mask_end.height / 2.0),
        false,
        false,
    );
    state.act(&mut context, TextInputAction::Backspace { word: false });
    assert_eq!(
        state.text(),
        "A中😀",
        "masked pointer maps to the secret's grapheme boundary"
    );
    state.act(&mut context, TextInputAction::Undo);
    state.act(&mut context, TextInputAction::TextStart { extend: false });
    state
        .preedit(&mut context, "中文", Some((0, 6)))
        .expect("password preedit");
    let composed = state.snapshot();
    state.refresh(&mut context);
    assert_eq!(state.snapshot(), composed);
    assert!(!state.snapshot().0.compose.is_empty());
    state.cancel_compose(&mut context);
    assert_ne!(state.snapshot(), composed);
    assert_eq!(state.text(), "A中😀ב");
}

#[test]
fn unicode_selection_and_history_preserve_text_contract() {
    let (mut context, mut state) = input("Vesta e\u{301}😀");
    let original_visual = state.snapshot();
    state.refresh(&mut context);
    assert_eq!(state.snapshot(), original_visual);
    state.act(
        &mut context,
        TextInputAction::Left {
            extend: true,
            word: false,
        },
    );
    assert_ne!(
        state.snapshot(),
        original_visual,
        "selection changes retire input evidence"
    );
    state.act(&mut context, TextInputAction::TextEnd { extend: false });
    let before_fonts = state.snapshot();
    context.set_font_stack(FontRole::Ui, "Material Icons");
    state.refresh(&mut context);
    assert_ne!(
        state.snapshot(),
        before_fonts,
        "font revision invalidates the layout"
    );
    let before_context = state.snapshot();
    let (mut replacement_context, _) = input("");
    replacement_context.set_font_stack(FontRole::Ui, "Material Icons");
    state.refresh(&mut replacement_context);
    assert_ne!(
        state.snapshot(),
        before_context,
        "equal font counters in different contexts are distinct"
    );
    state.act(&mut context, TextInputAction::Backspace { word: false });
    assert_eq!(state.text(), "Vesta e\u{301}");
    state.act(&mut context, TextInputAction::Backspace { word: false });
    assert_eq!(state.text(), "Vesta ");
    state.act(&mut context, TextInputAction::Undo);
    assert_eq!(state.text(), "Vesta e\u{301}😀");
    state.act(&mut context, TextInputAction::SelectAll);
    state
        .insert(&mut context, "地形\n道路", TextEditKind::Paste)
        .expect("paste");
    assert_eq!(state.text(), "地形 道路");
    state.act(&mut context, TextInputAction::Undo);
    assert_eq!(state.selected_text(), Some("Vesta e\u{301}😀"));
    state.act(&mut context, TextInputAction::Redo);
    assert_eq!(state.text(), "地形 道路");
    let before = state.text().to_owned();
    assert_eq!(
        state.insert(
            &mut context,
            &"x".repeat(TEXT_INPUT_MAX_BYTES + 1),
            TextEditKind::Paste
        ),
        Err(TextInputError::TooLong)
    );
    assert_eq!(state.text(), before);
    state
        .reset(&mut context, "xe\u{301}")
        .expect("exact range source");
    assert!(
        state
            .apply_accessibility_selection(
                &mut context,
                TextInputSelection {
                    anchor: 2,
                    focus: 2
                },
            )
            .is_some()
    );
    let rejected_before = state.clone();
    assert_eq!(
        state.insert(
            &mut context,
            &"x".repeat(TEXT_INPUT_MAX_BYTES),
            TextEditKind::Paste,
        ),
        Err(TextInputError::TooLong)
    );
    assert_eq!(
        state, rejected_before,
        "rejected paste preserves the exact selection"
    );
    // A shaped cluster edge may split an extended Unicode grapheme. Native
    // Shift+Left copied only U+0301 from xe\u{301}, although Backspace was safe.
    // Keyboard movement and its resulting edit must retain the whole grapheme.
    for multiline in [false, true] {
        for grapheme in ["e\u{301}", "👨‍👩‍👧‍👦", "🇨🇳", "👍🏽"] {
            let source = format!("x{grapheme}");
            let mut editor = if multiline {
                TextInputState::new_multiline(13.0, "Material Icons", 200.0)
            } else {
                TextInputState::new(13.0, "Material Icons")
            };
            editor
                .reset(&mut context, &source)
                .expect("grapheme source");
            editor.act(
                &mut context,
                TextInputAction::Left {
                    extend: false,
                    word: false,
                },
            );
            editor
                .insert(&mut context, "Q", TextEditKind::Typing)
                .expect("insert at preceding grapheme edge");
            assert_eq!(editor.text(), format!("xQ{grapheme}"));
            editor.act(&mut context, TextInputAction::Undo);
            editor.act(&mut context, TextInputAction::TextEnd { extend: false });
            editor.act(
                &mut context,
                TextInputAction::Left {
                    extend: true,
                    word: false,
                },
            );
            assert_eq!(editor.selected_text(), Some(grapheme));
            editor.act(&mut context, TextInputAction::Backspace { word: false });
            assert_eq!(editor.text(), "x");
            editor.act(&mut context, TextInputAction::Undo);
            assert_eq!(editor.text(), source);
            assert_eq!(editor.selected_text(), Some(grapheme));
            editor.act(&mut context, TextInputAction::TextStart { extend: false });
            editor.act(
                &mut context,
                TextInputAction::Right {
                    extend: false,
                    word: false,
                },
            );
            editor.act(
                &mut context,
                TextInputAction::Right {
                    extend: true,
                    word: false,
                },
            );
            assert_eq!(editor.selected_text(), Some(grapheme));
            editor.act(&mut context, TextInputAction::Delete { word: false });
            assert_eq!(editor.text(), "x");
            editor.act(&mut context, TextInputAction::Undo);
            editor.act(&mut context, TextInputAction::TextStart { extend: false });
            for _ in 0..2 {
                editor.act(
                    &mut context,
                    TextInputAction::Right {
                        extend: false,
                        word: false,
                    },
                );
            }
            editor
                .insert(&mut context, "Q", TextEditKind::Typing)
                .expect("insert at following grapheme edge");
            assert_eq!(editor.text(), format!("{source}Q"));

            editor.reset(&mut context, &source).expect("pointer source");
            let snapshot = editor.snapshot();
            for (byte, _) in source.char_indices() {
                let cursor = snapshot.layout_cursor(byte).expect("shaped source cursor");
                let bounds = cursor.bounds();
                let point = Point::new(bounds.x, bounds.y + bounds.height * 0.5);
                for (extend, word, already_positioned) in [
                    (false, false, false),
                    (true, false, false),
                    (false, true, false),
                    (false, false, true),
                ] {
                    let mut pointed = editor.clone();
                    if already_positioned {
                        assert!(
                            pointed
                                .apply_accessibility_selection(
                                    &mut context,
                                    TextInputSelection {
                                        anchor: byte,
                                        focus: byte,
                                    },
                                )
                                .is_some()
                        );
                    }
                    pointed.point(&mut context, point, extend, word);
                    let selection = pointed.editor.raw_selection();
                    for endpoint in [selection.anchor().index(), selection.focus().index()] {
                        assert!(
                            endpoint == source.len()
                                || source.grapheme_indices(true).any(|(i, _)| i == endpoint),
                            "pointer split {grapheme:?}: byte={byte}, endpoint={endpoint}, extend={extend}, word={word}, multiline={multiline}, already_positioned={already_positioned}"
                        );
                    }
                    let before = pointed.text().to_owned();
                    let selected = selection.text_range();
                    let mut expected = before.clone();
                    expected.replace_range(selected, "Q");
                    pointed
                        .insert(&mut context, "Q", TextEditKind::Typing)
                        .expect("insert at pointed grapheme edge");
                    assert_eq!(pointed.text(), expected);
                    pointed.act(&mut context, TextInputAction::Undo);
                    assert_eq!(pointed.text(), before);
                }
            }
        }
    }
    // History remains bounded even when large snapshots move to the redo stack.
    state.reset(&mut context, "").expect("reset");
    for index in 0..40 {
        state.act(&mut context, TextInputAction::SelectAll);
        state
            .insert(
                &mut context,
                &format!("{}{index}", "x".repeat(TEXT_INPUT_MAX_BYTES - 2)),
                TextEditKind::Paste,
            )
            .expect("bounded paste");
    }
    for action in [TextInputAction::Undo, TextInputAction::Redo] {
        while state.act(&mut context, action) {
            assert!(state.undo.len() + state.redo.len() <= HISTORY_STEPS);
            assert!(
                state
                    .undo
                    .iter()
                    .chain(&state.redo)
                    .map(|s| s.editor.raw_text().len())
                    .sum::<usize>()
                    <= HISTORY_TEXT_BYTES
            );
        }
    }
}

#[test]
fn decorated_editor_insets_share_hit_caret_selection_and_composition_geometry() {
    let (mut context, mut state) = input("12.5");
    let bounds = Rect::new(25.0, 30.0, 120.0, 32.0);
    let original = state.snapshot();
    let default_view = original.viewport(bounds, bounds, Point::default());
    assert_eq!(default_view.clip.x, bounds.x + 8.0);
    let decorated = original.clone().with_horizontal_insets(18.0, 8.0);
    assert_ne!(
        original, decorated,
        "retained views must observe inset changes"
    );
    assert!(
        Arc::ptr_eq(&original.0, &decorated.0),
        "layout stays shared"
    );
    let view = decorated.viewport(bounds, bounds, Point::default());
    assert_eq!(view.origin.x - default_view.origin.x, 10.0);
    assert_eq!(view.clip.x, bounds.x + 18.0);
    assert_eq!(
        original.viewport(bounds, bounds, Point::default()),
        default_view
    );
    state.point(
        &mut context,
        view.to_layout(Point::new(view.origin.x, view.origin.y + 3.0)),
        false,
        false,
    );
    assert_eq!(state.editor.raw_selection().text_range().start, 0);
    state.act(&mut context, TextInputAction::SelectAll);
    let selected = state.snapshot().with_horizontal_insets(18.0, 8.0);
    let view = selected.viewport(bounds, bounds, Point::default());
    let marks = selected.decorations(view, true, true, 1.25);
    assert!(!marks.selection.is_empty());
    assert!(marks.selection.iter().all(|r| r.x >= view.clip.x));
    assert!(view.clip.contains_rect(view.caret.expect("selected caret")));
    state
        .preedit(&mut context, "ni", Some((2, 2)))
        .expect("composition");
    let composed = state.snapshot().with_horizontal_insets(18.0, 8.0);
    let view = composed.viewport(bounds, bounds, Point::default());
    let marks = composed.decorations(view, true, true, 1.25);
    assert!(!marks.compose.is_empty());
    assert!(marks.compose.iter().all(|r| r.x >= view.clip.x));
    assert!(
        view.clip
            .contains_rect(view.caret.expect("composition caret"))
    );
    state.cancel_compose(&mut context);
    state
        .reset(&mut context, &"12345".repeat(60))
        .expect("long text");
    let snapshot = state.snapshot().with_horizontal_insets(18.0, 8.0);
    let view = snapshot.viewport(bounds, bounds, Point::default());
    assert!(view.scroll_x > 0.0);
    let caret = view.caret.expect("scrolled caret");
    assert!(caret.x >= view.clip.x && caret.right() <= view.clip.right() + 0.01);
    for (left, right) in [(18.0, 8.0), (f32::MAX, f32::MAX), (f32::NAN, -1.0)] {
        let narrow = Rect::new(0.0, 0.0, 10.0, 32.0);
        let view = original
            .clone()
            .with_horizontal_insets(left, right)
            .viewport(narrow, narrow, Point::default());
        assert!(view.origin.x.is_finite() && view.scroll_x.is_finite());
        assert!(view.clip.width >= 1.0 && view.clip.right() <= narrow.right());
    }
}

#[test]
fn ime_preedit_is_reversible_and_commits_as_one_edit() {
    let (mut context, mut state) = input("replace me");
    state.act(&mut context, TextInputAction::SelectAll);
    state
        .preedit(&mut context, "nihao", Some((5, 5)))
        .expect("compose");
    assert_eq!(state.text(), "replace me");
    assert!(state.is_composing());
    let bounds = Rect::new(0.0, 0.0, 200.0, 32.0);
    let snapshot = state.snapshot();
    let view = snapshot.viewport(bounds, bounds, Point::default());
    let decorations = snapshot.decorations(view, true, true, 2.0);
    assert!(!decorations.compose.is_empty());
    assert!(
        decorations
            .compose
            .iter()
            .all(|underline| underline.height == 1.0)
    );
    assert!(
        snapshot
            .decorations(view, false, true, 2.0)
            .compose
            .is_empty()
    );
    state.cancel_compose(&mut context);
    assert_eq!(state.selected_text(), Some("replace me"));
    state
        .preedit(&mut context, "nihao", Some((5, 5)))
        .expect("compose");
    state.commit_ime(&mut context, "你好").expect("commit");
    assert_eq!(state.text(), "你好");
    assert!(!state.is_composing());
    state.act(&mut context, TextInputAction::Undo);
    assert_eq!(state.text(), "replace me");
    assert_eq!(state.selected_text(), Some("replace me"));
    state
        .preedit(&mut context, "ni", Some((2, 2)))
        .expect("compose again");
    state
        .commit_ime(&mut context, "")
        .expect("empty commit cancels");
    assert_eq!(state.text(), "replace me");
    assert_eq!(state.selected_text(), Some("replace me"));
}

#[test]
fn text_geometry_stays_aligned_after_scrolling_and_raster_clipping() {
    let (mut context, mut state) = input(&"vesta ".repeat(30));
    let bounds = Rect::new(25.0, 30.0, 120.0, 28.0);
    let view = state.snapshot().viewport(bounds, bounds, Point::default());
    assert!(view.scroll_x > 0.0);
    let caret = view.caret.expect("end caret");
    assert!(caret.x >= view.clip.x && caret.right() <= view.clip.right() + 0.01);
    state.point(
        &mut context,
        view.to_layout(Point::new(caret.x, caret.y + 3.0)),
        false,
        false,
    );
    assert_eq!(
        state.editor.raw_selection().text_range().start,
        state.text().len()
    );
    state.act(&mut context, TextInputAction::Start { extend: false });
    let view = state
        .snapshot()
        .viewport(bounds, bounds, Point::new(view.scroll_x, view.scroll_y));
    assert_eq!(
        view.scroll_x,
        0.0,
        "selection {:?}, view {:?}, snapshot {:?}",
        state.editor.raw_selection(),
        view,
        state.snapshot()
    );
    state.act(&mut context, TextInputAction::SelectAll);
    let snapshot = state.snapshot();
    let clipped = Rect::new(40.0, 34.0, 70.0, 18.0);
    let view = snapshot.viewport(bounds, clipped, Point::new(view.scroll_x, view.scroll_y));
    let decorations = snapshot.decorations(view, true, true, 1.25);
    assert_eq!(decorations.selection.len(), snapshot.0.selection.len());
    let caret = decorations.caret.expect("GPU caret geometry");
    assert_eq!(
        caret.x,
        view.caret.expect("viewport caret").x + CARET_VISUAL_GAP
    );
    assert!(
        snapshot
            .decorations(view, false, false, 1.25)
            .caret
            .is_none()
    );
    let colors = crate::TextInputColors {
        text: crate::Color::srgba(1.0, 1.0, 1.0, 1.0),
        selection: crate::Color::srgba(0.2, 0.4, 0.8, 0.8),
        caret: crate::Color::srgba(1.0, 1.0, 1.0, 1.0),
    };
    let mut rasterizer = crate::TextRasterizer::default();
    let cached = rasterizer
        .render_shared(&snapshot, view, 1.0, true, true, colors)
        .expect("first composed bitmap");
    let reused = rasterizer
        .render_shared(&snapshot, view, 1.0, true, true, colors)
        .expect("same composed bitmap");
    assert!(Arc::ptr_eq(&cached, &reused));
    let changed_clip = rasterizer
        .render_shared(
            &snapshot,
            snapshot.viewport(bounds, bounds, Point::default()),
            1.0,
            true,
            true,
            colors,
        )
        .expect("different clip");
    assert!(!Arc::ptr_eq(&cached, &changed_clip));
    for scale in [1.0, 1.25, 2.0] {
        let image = rasterizer
            .render(&snapshot, view, scale, true, true, colors)
            .expect("raster");
        let repeated = rasterizer
            .render(&snapshot, view, scale, true, true, colors)
            .expect("cached raster");
        assert_eq!(image.rgba, repeated.rgba);
        if scale == 1.0 {
            let recolored = rasterizer
                .render(
                    &snapshot,
                    view,
                    scale,
                    true,
                    true,
                    crate::TextInputColors {
                        text: crate::Color::rgb8(255, 0, 0),
                        ..colors
                    },
                )
                .expect("cached glyphs with new color");
            assert_ne!(image.rgba, recolored.rgba);
        }
        assert!(image.rgba.chunks_exact(4).any(|p| p[3] != 0));
        for (index, pixel) in image.rgba.chunks_exact(4).enumerate() {
            let point = Point::new(
                bounds.x + (index as u32 % image.width) as f32 / scale,
                bounds.y + (index as u32 / image.width) as f32 / scale,
            );
            if !view.clip.contains(point) {
                assert_eq!(pixel[3], 0, "paint escaped ancestor clip");
            }
        }
    }
}

fn multiline(value: &str, wrap_width: f32) -> (TextInputContext, TextInputState) {
    let mut context = TextInputContext::default();
    let bytes = include_bytes!("../tests/fixtures/MaterialIcons-Regular.ttf");
    assert!(context.register_font(Arc::new(bytes.to_vec())));
    let mut state = TextInputState::new_multiline(13.0, "Material Icons", wrap_width);
    state.reset(&mut context, value).expect("initial text");
    (context, state)
}

/// 多行编辑器保留硬换行、按值框宽度折行；单行字段保持原来的单行语义。
#[test]
fn multiline_input_keeps_hard_breaks_and_wraps_at_the_declared_width() {
    let (_context, single) = input(&"alpha beta gamma ".repeat(6));
    assert!(!single.is_multiline());
    let bounds = Rect::new(0.0, 0.0, 120.0, 20.0);
    assert!(
        single
            .snapshot()
            .viewport(bounds, bounds, Point::default())
            .scroll_x
            > 0.0
    );

    let (mut context, mut state) = multiline("first line\nsecond line", 60.0);
    assert!(state.is_multiline());
    assert_eq!(state.text(), "first line\nsecond line");
    // 多行粘贴不吞换行（单行字段会把换行折成空格）。
    state.reset(&mut context, "").expect("reset");
    state
        .insert(
            &mut context,
            "alpha beta gamma delta\nsecond paragraph alpha beta gamma delta",
            TextEditKind::Paste,
        )
        .expect("paste");
    assert!(state.text().contains('\n'));
    // 折行 + 硬换行合起来至少三行：布局高度明显超过单行。
    let tall = Rect::new(10.0, 20.0, 90.0, 200.0);
    let view = state.snapshot().viewport(tall, tall, Point::default());
    assert!(
        view.scroll_y == 0.0 && state.snapshot().layout_height() > 3.0 * 13.0,
        "layout {} view {:?}",
        state.snapshot().layout_height(),
        view
    );
    // 折行宽度就是 `text_input_wrap_width`：折窄行数只会更多。
    let before_width = state.snapshot();
    state.set_wrap_width(&mut context, 40.0);
    assert_ne!(state.snapshot(), before_width);
    let narrow = state.snapshot().layout_height();
    let narrow_visual = state.snapshot();
    state.refresh(&mut context);
    assert_eq!(state.snapshot(), narrow_visual);
    assert!(!state.set_wrap_width(&mut context, 40.0));
    assert_eq!(state.snapshot(), narrow_visual);
    state.set_wrap_width(&mut context, 300.0);
    let wide = state.snapshot().layout_height();
    assert!(narrow > wide, "narrow {narrow} wide {wide}");

    let bounds = Rect::new(0.0, 0.0, 120.0, 120.0);
    let snapshot = state.snapshot();
    let bitmap = crate::TextRasterizer::default()
        .render(
            &snapshot,
            snapshot.viewport(bounds, bounds, Point::default()),
            3.0,
            true,
            true,
            crate::TextInputColors {
                text: crate::Color::rgb8(255, 255, 255),
                selection: crate::Color::rgb8(60, 120, 240),
                caret: crate::Color::rgb8(255, 255, 255),
            },
        )
        .expect("3x multiline bitmap");
    assert_eq!((bitmap.width, bitmap.height), (360, 360));
}

/// 内容高于值框时纵向滚动跟着光标走，光标始终留在框内；装得下时保持单行字段的居中。
#[test]
fn multiline_viewport_keeps_the_caret_inside_a_short_box() {
    let (mut context, mut state) = multiline(
        "one two three four five six seven eight nine ten eleven twelve",
        90.0,
    );
    let bounds = Rect::new(5.0, 7.0, 100.0, 24.0);
    // reset 把光标放在文本末尾：纵向必须已经滚到末尾，光标仍在裁剪区内。
    let view = state.snapshot().viewport(bounds, bounds, Point::default());
    assert!(view.scroll_y > 0.0, "view {view:?}");
    let caret = view.caret.expect("caret");
    assert!(caret.y >= view.clip.y - 0.01 && caret.bottom() <= view.clip.bottom() + 0.01);
    // Wheel browsing can keep earlier lines visible even while the caret stays at the end.
    let manual = state
        .snapshot()
        .with_viewport_scroll(Point::default(), false)
        .viewport(bounds, bounds, Point::default());
    assert_eq!(manual.scroll_y, 0.0);
    assert!(manual.caret.expect("caret").y >= manual.clip.bottom());
    let followed = state
        .snapshot()
        .with_viewport_scroll(Point::default(), true)
        .viewport(bounds, bounds, Point::default());
    assert!(followed.scroll_y > 0.0);
    assert!(followed.clip.contains_rect(followed.caret.expect("caret")));
    // Ctrl+Home 回到文本开头：同一份 bounds 下滚回顶部。
    state.act(&mut context, TextInputAction::TextStart { extend: false });
    let view = state
        .snapshot()
        .viewport(bounds, bounds, Point::new(view.scroll_x, view.scroll_y));
    assert_eq!(view.scroll_y, 0.0, "view {view:?}");
    // 装得下时按单行字段的老行为垂直居中。
    let (_short_context, short) = multiline("one", 90.0);
    let tall = Rect::new(5.0, 7.0, 100.0, 40.0);
    let view = short.snapshot().viewport(tall, tall, Point::default());
    assert_eq!(view.scroll_y, 0.0);
    let caret = view.caret.expect("caret");
    assert!(caret.y > tall.y + 2.0, "单行内容垂直居中：{view:?}");
    // A tall composer can opt into top alignment without moving its hit/caret geometry apart.
    let snapshot = short.snapshot();
    let top = snapshot
        .clone()
        .with_vertical_alignment(crate::TextVAlign::Start);
    assert_ne!(top, snapshot);
    let top_view = top.viewport(tall, tall, Point::default());
    assert_eq!(top_view.origin.y, tall.y + 2.0);
    assert!(
        top_view
            .clip
            .contains_rect(top_view.caret.expect("top-aligned caret"))
    );
    let bottom = snapshot.with_vertical_alignment(crate::TextVAlign::End);
    let bottom_view = bottom.viewport(tall, tall, Point::default());
    assert!(bottom_view.origin.y > view.origin.y);
    assert!(
        bottom_view
            .clip
            .contains_rect(bottom_view.caret.expect("bottom-aligned caret"))
    );
    let snapshot = state.snapshot();
    assert_eq!(
        snapshot.viewport(bounds, bounds, Point::default()),
        snapshot
            .with_vertical_alignment(crate::TextVAlign::Start)
            .viewport(bounds, bounds, Point::default())
    );
}

#[test]
fn static_text_shape_and_layout_share_font_metrics_and_hard_lines() {
    use crate::{
        Color, Dimension, Element, ElementId, ElementKey, LayoutEngine, Size, Style, Viewport,
    };

    let (mut context, _) = input("");
    let exact_source = "源\t雪😀\r\n尾";
    let exact_snapshot = context
        .shape_static_text_wrapped(exact_source, 13.0, crate::FontRole::Ui, Some(80.0))
        .expect("static source shape");
    exact_snapshot
        .range_rectangles(0..exact_source.len())
        .expect("static paint must retain the exact read-only source byte range");
    let single = context
        .shape_static_text("Wide", 13.0, crate::FontRole::Ui)
        .expect("single line shape");
    let two_lines = context
        .shape_static_text("Wide\nI", 13.0, crate::FontRole::Ui)
        .expect("hard line shape");
    assert_eq!(
        two_lines,
        context
            .shape_static_text("Wide\nI", 13.0, crate::FontRole::Ui)
            .expect("cached shape")
    );
    assert!(two_lines.layout_width() > 0.0);
    assert_eq!(two_lines.layout_width(), single.layout_width());
    assert!(two_lines.layout_height() > single.layout_height());

    let text_id = ElementId::new(2);
    let root = Element::container(ElementId::new(1), ElementKey::new("root"))
        .styled(
            Style::default()
                .size(Dimension::Px(500.0), Dimension::Px(500.0))
                .align(crate::Align::Start),
        )
        .child(
            Element::text(text_id, ElementKey::new("text"), "Wide\nI")
                .styled(Style::default().text(13.0, Color::rgb8(255, 255, 255))),
        );
    let layout = LayoutEngine
        .compute_with_text_measure(
            &root,
            Viewport::new(500.0, 500.0, 1.0),
            |text, font_size, role| {
                let snapshot = context
                    .shape_static_text(text, font_size, role)
                    .expect("layout text shape");
                Size::new(snapshot.layout_width(), snapshot.layout_height())
            },
        )
        .expect("shaped text layout");
    let rect = layout.node(text_id).expect("text geometry").rect;
    assert!(
        (rect.width - two_lines.layout_width()).abs() < 1.0,
        "layout width {} shaped width {}",
        rect.width,
        two_lines.layout_width()
    );
    assert!(
        (rect.height - two_lines.layout_height()).abs() < 1.0,
        "layout height {} shaped height {}",
        rect.height,
        two_lines.layout_height()
    );
    let wrapped_text = "Wide Wide Wide Wide Wide";
    let wrapped = context
        .shape_static_text_wrapped(wrapped_text, 13.0, crate::FontRole::Ui, Some(80.0))
        .expect("soft-wrapped shape");
    assert_eq!(
        wrapped,
        context
            .shape_static_text_wrapped(wrapped_text, 13.0, crate::FontRole::Ui, Some(80.0))
            .expect("cached soft-wrapped shape")
    );
    let unwrapped = context
        .shape_static_text(wrapped_text, 13.0, crate::FontRole::Ui)
        .expect("unwrapped shape");
    assert!(wrapped.layout_height() > unwrapped.layout_height());
    assert!(
        wrapped.layout_width() <= 80.0,
        "wrapped width {} unwrapped {} height {}",
        wrapped.layout_width(),
        unwrapped.layout_width(),
        wrapped.layout_height()
    );
    let base_entries = context.static_text_cache_counts().0;
    for width in [72.0, 88.0, 104.0, 120.0] {
        context
            .shape_static_text_wrapped(wrapped_text, 13.0, crate::FontRole::Ui, Some(width))
            .expect("width-only static text reflow");
    }
    let (bases_after_reflow, wrapped_after_reflow) = context.static_text_cache_counts();
    assert_eq!(bases_after_reflow, base_entries);
    assert!(wrapped_after_reflow >= 5);
    let wrapped_id = ElementId::new(4);
    let wrapped_root = Element::container(ElementId::new(3), ElementKey::new("wrapped.root"))
        .styled(Style::default().size(Dimension::Px(200.0), Dimension::Px(200.0)))
        .child(
            Element::text(wrapped_id, ElementKey::new("wrapped.text"), wrapped_text).styled(
                Style::default()
                    .size(Dimension::Px(80.0), Dimension::Auto)
                    .text(13.0, Color::rgb8(255, 255, 255))
                    .text_wrap(true),
            ),
        );
    let wrapped_layout = LayoutEngine
        .compute_with_text_layout(
            &wrapped_root,
            Viewport::new(200.0, 200.0, 1.0),
            |text, size, role, width| {
                let shaped = context
                    .shape_static_text_wrapped(text, size, role, width)
                    .expect("measured wrapped label");
                Size::new(shaped.layout_width(), shaped.layout_height())
            },
        )
        .expect("wrapped layout");
    let wrapped_rect = wrapped_layout
        .node(wrapped_id)
        .expect("wrapped geometry")
        .rect;
    assert_eq!(wrapped_rect.width, 80.0);
    assert!((wrapped_rect.height - wrapped.layout_height()).abs() < 1.0);
    let stretch_id = ElementId::new(6);
    let stretch_root = Element::container(ElementId::new(5), ElementKey::new("stretch.root"))
        .styled(Style::default().size(Dimension::Px(100.0), Dimension::Px(200.0)))
        .child(
            Element::text(stretch_id, ElementKey::new("stretch.text"), wrapped_text)
                .styled(Style::default().text_wrap(true)),
        );
    let stretch_layout = LayoutEngine
        .compute_with_text_layout(
            &stretch_root,
            Viewport::new(100.0, 200.0, 1.0),
            |text, size, role, width| {
                let shaped = context
                    .shape_static_text_wrapped(text, size, role, width)
                    .expect("measured stretched label");
                Size::new(shaped.layout_width(), shaped.layout_height())
            },
        )
        .expect("stretched layout");
    let stretch_rect = stretch_layout
        .node(stretch_id)
        .expect("stretch geometry")
        .rect;
    let stretched = context
        .shape_static_text_wrapped(
            wrapped_text,
            13.0,
            crate::FontRole::Ui,
            Some(stretch_rect.width),
        )
        .expect("stretched shape");
    assert_eq!(stretch_rect.width, 100.0);
    assert!((stretch_rect.height - stretched.layout_height()).abs() < 1.0);
    assert!(context.static_text_cache_counts().0 > 0);
    assert!(context.static_text_cache_counts().1 > 0);
    // A row shares one baseline across content and centered 30/32px controls,
    // including glyphs with different ink bounds. Multiline text keeps its line box.
    let reference = context
        .shape_static_text("Hg", 13.0, crate::FontRole::Ui)
        .expect("baseline reference");
    let expected_baseline = (32.0 - reference.layout_height()) * 0.5
        + reference.single_line_baseline().expect("baseline");
    for (caption, y, height) in [
        ("Vesta", 0.0, 32.0),
        ("文件", 1.0, 30.0),
        ("English", 1.0, 30.0),
        ("\u{e145}", 1.0, 30.0),
    ] {
        let bounds = Rect::new(0.0, y, 200.0, height);
        let (snapshot, view) = crate::render::static_text_view(
            &mut context,
            bounds,
            bounds,
            caption,
            13.0,
            crate::FontRole::Ui,
            false,
            crate::TextHAlign::Center,
            crate::TextVAlign::Baseline,
        )
        .expect("baseline-aligned view");
        let baseline = view.origin.y + snapshot.single_line_baseline().expect("single line");
        assert!((baseline - expected_baseline).abs() < 0.0001);
    }
    let bounds = Rect::new(0.0, 0.0, 200.0, 80.0);
    let (snapshot, view) = crate::render::static_text_view(
        &mut context,
        bounds,
        bounds,
        "Wide\nI",
        13.0,
        crate::FontRole::Ui,
        false,
        crate::TextHAlign::Center,
        crate::TextVAlign::Baseline,
    )
    .expect("multiline fallback");
    assert!(snapshot.single_line_baseline().is_none());
    assert!((view.origin.y - (bounds.height - snapshot.layout_height()) * 0.5).abs() < 0.0001);
    // Static bidi labels must stay in their intrinsic frame. The editor's
    // generous no-wrap width must not right-align RTL glyphs a million pixels away,
    // and a wrapped cache entry must not apply a second directional alignment.
    for caption in ["مرحبا / North", "שלום / English", "مرحبا\nEnglish"] {
        for wrap in [false, true] {
            for h_align in [crate::TextHAlign::Start, crate::TextHAlign::End] {
                let bounds = Rect::new(20.0, 10.0, 180.0, 100.0);
                let (snapshot, view) = crate::render::static_text_view(
                    &mut context,
                    bounds,
                    bounds,
                    caption,
                    13.0,
                    crate::FontRole::Ui,
                    wrap,
                    h_align,
                    crate::TextVAlign::Start,
                )
                .expect("static bidi view");
                assert!(snapshot.layout_width() > 0.0);
                for line in snapshot.0.layout.lines() {
                    assert!(
                        line.metrics().offset.abs() < 0.01,
                        "static bidi line escaped its physical frame: {caption:?}, wrap={wrap}, {:?}",
                        line.metrics()
                    );
                }
                let expected_x = match h_align {
                    crate::TextHAlign::Start => bounds.x,
                    _ => bounds.right() - snapshot.layout_width().min(bounds.width),
                };
                assert!((view.origin.x - expected_x).abs() < 0.01);
                let mut visible_glyphs = 0;
                crate::TextRasterizer::default()
                    .visit_glyph_refs(&snapshot, view, 1.0, |glyph| {
                        if glyph.width > 0 && glyph.height > 0 {
                            visible_glyphs += 1;
                        }
                    })
                    .expect("static bidi glyphs");
                assert!(visible_glyphs > 0, "bidi glyphs clipped out: {caption:?}");
            }
        }
    }
    let font = include_bytes!("../tests/fixtures/MaterialIcons-Regular.ttf");
    assert!(context.register_font(Arc::new(font.to_vec())));
    assert_eq!(context.static_text_cache_counts(), (0, 0));
    assert_ne!(
        two_lines,
        context
            .shape_static_text("Wide\nI", 13.0, crate::FontRole::Ui)
            .expect("shape after font change")
    );
    context.set_font_stack(crate::FontRole::Icon, "Material Icons");
    let icon = context
        .shape_static_text("Wide\nI", 13.0, crate::FontRole::Icon)
        .expect("icon role shape");
    assert_ne!(
        icon,
        context
            .shape_static_text("Wide\nI", 13.0, crate::FontRole::Ui)
            .expect("UI role shape")
    );
}
