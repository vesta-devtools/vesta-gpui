use super::*;
use crate::tests::{activate_gallery_page, install_action_route};
use vesta_gpui::{TextEditKind, TextInputAction};
use winit::keyboard::ModifiersState;

#[test]
fn inline_borrow_preserves_history_and_rejects_invalid_or_foreign_commits() {
    let mut app = App::new();
    activate_gallery_page(&mut app, "gallery.inputs");
    install_action_route(&mut app);
    app.search_state
        .reset(&mut app.text_context, "Search 中文")
        .expect("search");
    app.search_state
        .act(&mut app.text_context, TextInputAction::SelectAll);
    app.search_state
        .insert(&mut app.text_context, "redo", TextEditKind::Paste)
        .expect("edit");
    app.search_state
        .act(&mut app.text_context, TextInputAction::Undo);
    let label = app.demo.inline_label.clone();
    app.begin_inline_edit();
    assert!(app.demo.inline_session.is_open());
    assert_eq!(app.search_state.text(), label);
    let borrowed = app.search_state.snapshot();
    app.begin_inline_edit();
    assert_eq!(
        app.search_state.snapshot(),
        borrowed,
        "reopen replaced live draft"
    );
    app.action_window = UiWindowId(999);
    app.finish_inline_edit(true);
    assert!(app.demo.inline_session.is_open());
    app.action_window = STANDALONE_WINDOW;
    app.search_state
        .reset(&mut app.text_context, "  ")
        .expect("blank");
    app.finish_inline_edit(true);
    assert!(
        app.demo.inline_session.is_open(),
        "invalid draft was discarded"
    );
    assert_eq!(app.demo.inline_label, label);
    app.search_state
        .reset(&mut app.text_context, "new 中文")
        .expect("valid");
    app.search_state
        .preedit(&mut app.text_context, "未确认", None)
        .expect("preedit");
    app.finish_inline_edit(true);
    assert!(app.demo.inline_session.is_open());
    assert!(!app.handle_inline_edit_key_for(STANDALONE_WINDOW, &Key::Named(NamedKey::Escape)));
    app.search_state
        .reset(&mut app.text_context, "  new 中文  ")
        .expect("committed");
    app.text_bridge
        .update_modifiers(&ModifiersState::CONTROL.into());
    assert!(!app.handle_inline_edit_key_for(STANDALONE_WINDOW, &Key::Named(NamedKey::Enter)));
    assert!(app.demo.inline_session.is_open());
    app.text_bridge
        .update_modifiers(&ModifiersState::empty().into());
    assert!(app.handle_inline_edit_key_for(STANDALONE_WINDOW, &Key::Named(NamedKey::Enter)));
    app.finish_inline_edit(true);
    assert_eq!(app.demo.inline_label, "new 中文");
    assert!(!app.demo.inline_session.is_open());
    assert_eq!(app.search_state.text(), "Search 中文");
    assert_eq!(app.search_state.selected_text(), Some("Search 中文"));
    app.search_state
        .act(&mut app.text_context, TextInputAction::Redo);
    assert_eq!(app.search_state.text(), "redo");
    app.search_state
        .act(&mut app.text_context, TextInputAction::Undo);
    install_action_route(&mut app);
    app.begin_inline_edit();
    app.search_state
        .reset(&mut app.text_context, "discarded")
        .expect("cancel draft");
    app.cancel_window_editors_for(UiWindowId(999), PopupCloseReason::WindowUnavailable);
    assert!(
        app.demo.inline_session.is_open(),
        "foreign cleanup consumed session"
    );
    app.cancel_window_editors_for(STANDALONE_WINDOW, PopupCloseReason::WindowUnavailable);
    assert!(!app.demo.inline_session.is_open());
    assert_eq!(app.demo.inline_label, "new 中文");
    assert_eq!(app.search_state.selected_text(), Some("Search 中文"));
    app.search_state
        .act(&mut app.text_context, TextInputAction::Redo);
    assert_eq!(app.search_state.text(), "redo");
    assert!(app.input_ownership.capture().is_none());

    install_action_route(&mut app);
    app.begin_inline_edit();
    install_action_route(&mut app);
    app.set_focus_target(Some(app.scopes.gallery.element(INLINE_ROOT + 4)));
    assert!(
        app.demo.inline_session.is_open(),
        "Save focus cancelled draft"
    );
    app.set_focus_target(Some(app.scopes.gallery.element(INLINE_ROOT + 6)));
    assert!(
        app.demo.inline_session.is_open(),
        "Cancel focus cancelled draft before action"
    );
    app.open_demo_dialog(1);
    assert!(
        !app.demo.inline_session.is_open(),
        "modal focus retained inline draft"
    );
    assert_eq!(app.search_state.text(), "redo");
    app.close_demo_dialog(1, false);
    install_action_route(&mut app);
    app.begin_inline_edit();
    install_action_route(&mut app);
    app.begin_text_selection(STANDALONE_WINDOW, TextFieldKind::Area, None);
    assert!(
        !app.demo.inline_session.is_open(),
        "another field retained inline draft"
    );
    assert_eq!(app.search_state.text(), "redo");
}
