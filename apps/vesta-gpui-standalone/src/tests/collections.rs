use super::*;
use crate::window_collections::collection_item_focus;

#[test]
fn table_editor_preserves_cell_drafts_and_cancels_removed_source() {
    table_resize_survives_a_narrow_projection_after_wide_edits();
    table_query_preserves_live_cells_and_retires_missing_owners();
    use vesta_gpui::{TextEditKind, TextInputAction};
    let mut app = App::new();
    activate_gallery_page(&mut app, "gallery.collections");
    app.apply_demo_action(DemoAction::SelectTable(2));
    let root = install_action_route(&mut app);
    let editor = TextFieldKind::TableNote.element(app.scopes.gallery);
    let layout = LayoutEngine
        .compute(&root, Viewport::new(1280.0, 800.0, 1.0))
        .expect("table layout");
    let original_y = layout.node(editor).expect("nested editor").rect.y;
    assert_eq!(
        app.ui_actions.action_for_hit(editor),
        None,
        "editing a cell must not select its row"
    );
    app.set_window_focus(STANDALONE_WINDOW, Some(editor));
    app.table_note_state
        .insert(&mut app.text_context, " / accepted", TextEditKind::Typing)
        .expect("cell draft");
    let accepted = "Ready / accepted";
    assert_eq!(app.demo.table_note, "Ready");
    app.apply_demo_action(DemoAction::SortTable(1));
    let sorted = install_action_route(&mut app);
    let layout = LayoutEngine
        .compute(&sorted, Viewport::new(1280.0, 800.0, 1.0))
        .expect("sorted layout");
    assert_ne!(
        layout.node(editor).expect("same cell editor").rect.y,
        original_y
    );
    assert_eq!(app.window_focus(STANDALONE_WINDOW), Some(editor));
    assert_eq!(app.table_note_state.text(), accepted);
    assert_eq!(app.demo.table_selected, 2);
    assert!(
        app.table_note_state
            .act(&mut app.text_context, TextInputAction::Undo)
    );
    assert_eq!(app.table_note_state.text(), "Ready");
    assert!(
        app.table_note_state
            .act(&mut app.text_context, TextInputAction::Redo)
    );
    assert_eq!(app.table_note_state.text(), accepted);
    app.set_window_focus(STANDALONE_WINDOW, None);
    assert_eq!(
        app.demo.table_note, accepted,
        "ordinary accepted field exit commits the stable cell"
    );

    app.set_window_focus(STANDALONE_WINDOW, Some(editor));
    app.table_note_state
        .insert(&mut app.text_context, " / abandoned", TextEditKind::Typing)
        .expect("unaccepted draft");
    app.table_note_state
        .preedit(&mut app.text_context, "未确认", None)
        .expect("uncommitted composition");
    app.cancel_window_editors_for(UiWindowId(999), PopupCloseReason::OwnerRemoved);
    assert!(
        app.table_note_state.is_composing(),
        "foreign cleanup cannot cancel the live cell owner"
    );
    assert_eq!(app.demo.table_note, accepted);
    let table_root = app.scopes.gallery.element(TABLE_ROOT);
    app.retire_window_subtree_input(STANDALONE_WINDOW, table_root);
    assert_eq!(
        app.demo.table_note, accepted,
        "removed table source must cancel its unaccepted draft"
    );
    assert_eq!(app.table_note_state.text(), accepted);
    assert!(!app.table_note_state.is_composing());
    assert!(app.text_session.is_none());
    assert!(app.input_ownership.focus().is_none());
    assert!(!app.handle_window_text_event(
        STANDALONE_WINDOW,
        &WindowEvent::Ime(winit::event::Ime::Commit("迟到".into()))
    ));
    assert_eq!(app.demo.table_note, accepted);

    for (reason, suffix, commits) in [
        (PopupCloseReason::WindowUnavailable, " / lost window", false),
        (PopupCloseReason::FocusTransfer, " / blur accepted", true),
    ] {
        app.set_window_focus(STANDALONE_WINDOW, Some(editor));
        app.table_note_state
            .insert(&mut app.text_context, suffix, TextEditKind::Typing)
            .expect("terminal draft");
        app.cancel_window_editors_for(STANDALONE_WINDOW, reason);
        assert_eq!(
            app.demo.table_note,
            if commits {
                format!("{accepted}{suffix}")
            } else {
                accepted.to_owned()
            }
        );
    }
    let committed = app.demo.table_note.clone();
    app.set_window_focus(STANDALONE_WINDOW, Some(editor));
    app.table_note_state
        .reset(&mut app.text_context, &"x".repeat(129))
        .expect("invalid cell draft");
    app.set_window_focus(STANDALONE_WINDOW, None);
    assert_eq!(app.demo.table_note, committed);
    assert_eq!(app.table_note_state.text(), committed);
}

fn table_resize_survives_a_narrow_projection_after_wide_edits() {
    fn project(app: &mut App, size: Size) -> LayoutTree {
        let root = install_action_route_with_viewport(app, size);
        let layout = LayoutEngine
            .compute(&root, Viewport::new(size.width, size.height, 1.0))
            .expect("actual Table projection");
        app.table_rect = Some(
            layout
                .node(app.scopes.gallery.element(TABLE_ROOT))
                .expect("current Table rect")
                .rect,
        );
        app.hit_test = HitTestTree::build(&root, &layout);
        let rounded: [f32; 3] = std::array::from_fn(|column| {
            layout
                .node(app.scopes.gallery.element(TABLE_ROOT + 111 + column as u32))
                .expect("stable first-row cell")
                .rect
                .width
        });
        let tracks: [f32; 3] = std::array::from_fn(|column| {
            match root
                .find(app.scopes.gallery.element(TABLE_ROOT + 111 + column as u32))
                .expect("declared stable cell")
                .style
                .width
            {
                Dimension::Px(width) => width,
                _ => panic!("Table tracks must declare logical pixel widths"),
            }
        });
        assert!(tracks.iter().all(|width| *width >= 36.0 - 0.001));
        assert!(
            tracks
                .iter()
                .zip(rounded)
                .all(|(logical, rounded)| (*logical - rounded).abs() <= 1.0)
        );
        assert!((tracks.iter().sum::<f32>() - app.table_rect.expect("Table").width).abs() < 0.001);
        for column in 0..2 {
            let number = root
                .find(table_resize_element(app.scopes.gallery, column))
                .and_then(|element| element.semantic.as_ref())
                .and_then(|semantic| semantic.numeric)
                .expect("resize numeric semantics");
            assert!((number.value - f64::from(tracks[column])).abs() < 0.001);
            assert_eq!(number.min, 36.0);
            assert!(
                (number.max - f64::from(tracks[column] + tracks[column + 1] - 36.0)).abs() < 0.001
            );
        }
        println!(
            "GPV05-TABLE-RESIZE-PROJECTION viewport={size:?} logical={tracks:?} rounded={rounded:?} retained={:?}",
            app.demo.table_column_fractions
        );
        layout
    }
    let mut app = App::new();
    activate_gallery_page(&mut app, "gallery.collections");
    let wide = Size::new(1280.0, 800.0);
    project(&mut app, wide);
    for column in [0, 1, 0] {
        let previous = app.demo.table_column_fractions;
        let handle = table_resize_element(app.scopes.gallery, column);
        app.set_window_focus(STANDALONE_WINDOW, Some(handle));
        assert!(app.handle_scalar_control_key(STANDALONE_WINDOW, &Key::Named(NamedKey::End)));
        let untouched = if column == 0 { 2 } else { 0 };
        assert_eq!(
            app.demo.table_column_fractions[untouched], previous[untouched],
            "ordinary paired resize preserves the other column"
        );
        project(&mut app, wide);
    }
    let narrow = Size::new(360.0, 220.0);
    let before = app.demo.table_column_fractions;
    project(&mut app, narrow);
    let table = app.table_rect.expect("narrow Table geometry");
    let handle = table_resize_element(app.scopes.gallery, 1);
    app.set_window_focus(STANDALONE_WINDOW, Some(handle));
    app.handle_scalar_control_key(STANDALONE_WINDOW, &Key::Named(NamedKey::End));
    println!(
        "GPV05-TABLE-RESIZE-NARROW table={table:?} before={before:?} after={:?}",
        app.demo.table_column_fractions
    );
    project(&mut app, narrow);
    assert_eq!(
        app.demo.table_column_fractions, before,
        "constrained layout and unchanged End must not publish responsive proportions"
    );
    assert!(
        app.demo
            .table_column_fractions
            .iter()
            .all(|value| value.is_finite() && *value > 0.0)
    );
    let pointer_size = Size::new(360.0, 800.0);
    let layout = project(&mut app, pointer_size);
    let first_handle = table_resize_element(app.scopes.gallery, 0);
    let rect = layout.node(first_handle).expect("live first handle").rect;
    app.cursor = Point::new(rect.x + rect.width / 2.0, rect.y + rect.height / 2.0);
    assert_eq!(app.hit_test.hit(app.cursor), Some(first_handle));
    app.set_window_focus(
        STANDALONE_WINDOW,
        Some(TextFieldKind::TableNote.element(app.scopes.gallery)),
    );
    app.table_note_state
        .insert(
            &mut app.text_context,
            " / resize",
            vesta_gpui::TextEditKind::Typing,
        )
        .expect("ordinary cell draft before resize focus transfer");
    app.pointer_press();
    assert!(app.table_resize.is_some());
    assert_eq!(app.demo.table_note, "Ready / resize");
    let captured = app.input_ownership.capture();
    assert_eq!(
        captured.map(|capture| capture.target.element),
        Some(first_handle)
    );
    let initial = app.demo.table_column_fractions;
    app.cursor.x -= 12.0;
    assert!(app.update_table_resize());
    let preview = app.demo.table_column_fractions;
    assert_ne!(preview, initial);
    project(&mut app, pointer_size);
    app.cancel_window_value_edits(UiWindowId(999));
    assert_eq!(app.input_ownership.capture(), captured);
    assert_eq!(app.demo.table_column_fractions, preview);
    assert!(app.handle_window_key(STANDALONE_WINDOW, &Key::Named(NamedKey::Escape)));
    assert_eq!(app.demo.table_column_fractions, initial);
    assert!(app.table_resize.is_none() && app.input_ownership.capture().is_none());
    assert!(!app.update_table_resize());
    project(&mut app, pointer_size);
    println!(
        "GPV05-TABLE-RESIZE-TERMINALS before={initial:?} preview={preview:?} escape={:?} note={:?}",
        app.demo.table_column_fractions, app.demo.table_note
    );
    #[derive(Clone, Copy, Debug)]
    enum Terminal {
        Resize,
        Scale,
        SourceRemoved,
        Release,
    }
    for terminal in [
        Terminal::Resize,
        Terminal::Scale,
        Terminal::SourceRemoved,
        Terminal::Release,
    ] {
        let layout = project(&mut app, pointer_size);
        let rect = layout.node(first_handle).expect("fresh resize handle").rect;
        app.cursor = Point::new(rect.x + rect.width / 2.0, rect.y + rect.height / 2.0);
        assert_eq!(app.hit_test.hit(app.cursor), Some(first_handle));
        app.pointer_press();
        assert!(app.table_resize.is_some());
        let before = app.demo.table_column_fractions;
        app.cursor.x -= 12.0;
        assert!(app.update_table_resize());
        let preview = app.demo.table_column_fractions;
        let capture = app.input_ownership.capture();
        let finite_pointer = app.cursor;
        app.cursor.x = f32::NAN;
        assert!(!app.update_table_resize());
        assert_eq!(app.demo.table_column_fractions, preview);
        assert_eq!(app.input_ownership.capture(), capture);
        app.cursor = finite_pointer;
        match terminal {
            Terminal::Resize => app.main_window_resized(winit::dpi::PhysicalSize::new(360, 800)),
            Terminal::Scale => app.main_window_scale_factor_changed(),
            Terminal::SourceRemoved => app.retire_window_subtree_input(
                STANDALONE_WINDOW,
                app.scopes.gallery.element(TABLE_ROOT),
            ),
            Terminal::Release => {
                assert!(app.finish_table_resize());
                assert!(
                    app.input_ownership
                        .release_pointer(STANDALONE_WINDOW, PointerButton::Primary)
                );
            }
        }
        assert!(app.table_resize.is_none() && app.input_ownership.capture().is_none());
        assert_eq!(
            app.demo.table_column_fractions,
            if matches!(terminal, Terminal::Release) {
                preview
            } else {
                before
            }
        );
        app.cursor.x -= 7.0;
        assert!(!app.update_table_resize());
        project(&mut app, pointer_size);
        println!(
            "GPV05-TABLE-RESIZE-EXIT terminal={terminal:?} before={before:?} preview={preview:?} final={:?}",
            app.demo.table_column_fractions
        );
    }
    project(&mut app, wide);
    for (replacement, button) in [
        (app.scopes.header.element(MODE_2D), PointerButton::Primary),
        (first_handle, PointerButton::Secondary),
    ] {
        let layout = project(&mut app, pointer_size);
        let rect = layout
            .node(first_handle)
            .expect("replacement test handle")
            .rect;
        app.cursor = Point::new(rect.x + rect.width / 2.0, rect.y + rect.height / 2.0);
        assert_eq!(app.hit_test.hit(app.cursor), Some(first_handle));
        app.pointer_press();
        let before = app.demo.table_column_fractions;
        app.cursor.x -= 8.0;
        assert!(app.update_table_resize());
        let preview = app.demo.table_column_fractions;
        let capture = app.input_ownership.capture();
        assert!(!window_controls::cancel_table_resize_session(
            UiWindowId(999),
            &mut app.table_resize,
            &mut app.input_ownership,
            &mut app.demo.table_column_fractions
        ));
        assert!(app.table_resize.is_some());
        assert_eq!(app.demo.table_column_fractions, preview);
        assert_eq!(app.input_ownership.capture(), capture);
        app.input_ownership.capture_pointer(
            InputTarget {
                window: STANDALONE_WINDOW,
                element: replacement,
            },
            button,
        );
        let replacement_capture = app.input_ownership.capture();
        app.main_window_resized(winit::dpi::PhysicalSize::new(360, 800));
        assert!(app.table_resize.is_none());
        assert_eq!(app.demo.table_column_fractions, before);
        assert_eq!(app.input_ownership.capture(), replacement_capture);
        app.input_ownership.cancel_capture();
        println!(
            "GPV05-TABLE-RESIZE-REPLACEMENT button={button:?} before={before:?} preview={preview:?} final={:?}",
            app.demo.table_column_fractions
        );
    }
    let mut cascade = App::new();
    activate_gallery_page(&mut cascade, "gallery.collections");
    project(&mut cascade, wide);
    cascade.set_window_focus(
        STANDALONE_WINDOW,
        Some(table_resize_element(cascade.scopes.gallery, 0)),
    );
    for key in [NamedKey::PageDown, NamedKey::ArrowLeft] {
        assert!(cascade.handle_scalar_control_key(STANDALONE_WINDOW, &Key::Named(key)));
        project(&mut cascade, wide);
    }
    cascade.set_window_focus(
        STANDALONE_WINDOW,
        Some(table_resize_element(cascade.scopes.gallery, 1)),
    );
    assert!(cascade.handle_scalar_control_key(STANDALONE_WINDOW, &Key::Named(NamedKey::Home)));
    let retained = cascade.demo.table_column_fractions;
    let layout = project(&mut cascade, narrow);
    assert_eq!(cascade.demo.table_column_fractions, retained);
    let widths: [f32; 3] = std::array::from_fn(|column| {
        layout
            .node(
                cascade
                    .scopes
                    .gallery
                    .element(TABLE_ROOT + 111 + column as u32),
            )
            .expect("cascade cell")
            .rect
            .width
    });
    assert_eq!(
        widths,
        [36.0, 36.0, 60.0],
        "a newly constrained neighbor must be reserved on the next pass"
    );
    println!("GPV05-TABLE-RESIZE-CASCADE retained={retained:?} displayed={widths:?}");
}

fn table_query_preserves_live_cells_and_retires_missing_owners() {
    use vesta_gpui::{TextEditKind, TextInputAction};
    let mut app = App::new();
    activate_gallery_page(&mut app, "gallery.collections");
    app.apply_demo_action(DemoAction::SelectTableCell(2, 1));
    install_action_route(&mut app);
    let editor = TextFieldKind::TableNote.element(app.scopes.gallery);
    let table_id = app.scopes.gallery.element(TABLE_ROOT);
    let row_id = |id: u32| app.scopes.gallery.element(TABLE_ROOT + 100 + id * 10);
    let rows = [row_id(1), row_id(2), row_id(3)];
    let columns = [0, 1, 2].map(|column| app.scopes.gallery.element(TABLE_ROOT + 10 + column));
    app.set_window_focus(STANDALONE_WINDOW, Some(editor));
    app.table_note_state
        .insert(&mut app.text_context, " / retained", TextEditKind::Typing)
        .expect("unaccepted stable-cell draft");
    app.search_state
        .reset(&mut app.text_context, "  cU  ")
        .expect("shared object query");
    let filtered = install_action_route(&mut app);
    let table = filtered.find(table_id).expect("filtered Table shell");
    let visible = rows
        .iter()
        .enumerate()
        .filter_map(|(index, id)| table.find(*id).map(|_| index + 1))
        .collect::<Vec<_>>();
    assert_eq!(
        visible,
        vec![1],
        "actual Table must use the existing normalized object query"
    );
    assert!(table.find(editor).is_some());
    assert!(columns.iter().all(|column| table.find(*column).is_some()));
    let retained = FocusScope::new(STANDALONE_WINDOW, &app.ui_actions, None)
        .reconcile(&mut app.input_ownership);
    assert!(!retained.lost_focus && !retained.lost_capture);
    assert_eq!(app.window_focus(STANDALONE_WINDOW), Some(editor));
    assert_eq!(app.table_note_state.text(), "Ready / retained");
    assert_eq!(app.demo.table_note, "Ready");
    assert!(
        app.table_note_state
            .act(&mut app.text_context, TextInputAction::Undo)
    );
    assert_eq!(app.table_note_state.text(), "Ready");
    assert!(
        app.table_note_state
            .act(&mut app.text_context, TextInputAction::Redo)
    );
    assert_eq!(app.table_note_state.text(), "Ready / retained");
    app.apply_demo_action(DemoAction::SortTable(2));
    let sorted = install_action_route(&mut app);
    assert!(
        sorted
            .find(table_id)
            .expect("sorted Table")
            .find(editor)
            .is_some()
    );
    assert_eq!(app.window_focus(STANDALONE_WINDOW), Some(editor));
    assert_eq!(app.demo.table_selected_cell, Some((2, 1)));
    app.table_note_state
        .preedit(&mut app.text_context, "未确认", None)
        .expect("cell composition");
    app.search_state
        .reset(&mut app.text_context, "terrain")
        .expect("filter out edited row");
    let removed = install_action_route(&mut app);
    let table = removed.find(table_id).expect("Table shell survives");
    assert!(table.find(rows[2]).is_some());
    assert!(table.find(rows[0]).is_none() && table.find(editor).is_none());
    let loss = FocusScope::new(STANDALONE_WINDOW, &app.ui_actions, None)
        .reconcile(&mut app.input_ownership);
    assert!(loss.lost_focus);
    // Enter the same ordered Host source-loss boundary used after frame reconciliation.
    app.cancel_window_editors_for(STANDALONE_WINDOW, PopupCloseReason::OwnerRemoved);
    assert!(app.text_session.is_none() && !app.table_note_state.is_composing());
    assert_eq!(app.table_note_state.text(), "Ready");
    assert_eq!(app.demo.table_note, "Ready");
    assert!(!app.handle_window_text_event(
        STANDALONE_WINDOW,
        &WindowEvent::Ime(winit::event::Ime::Commit("late".into()))
    ));
    assert_eq!(app.demo.table_selected, 2);
    assert_eq!(app.demo.table_selected_cell, Some((2, 1)));

    app.search_state
        .reset(&mut app.text_context, "not-present")
        .expect("empty result");
    let empty = install_action_route(&mut app);
    let table = empty.find(table_id).expect("empty Table retains its shell");
    assert!(rows.iter().all(|row| table.find(*row).is_none()));
    assert!(columns.iter().all(|column| table.find(*column).is_some()));
    app.search_state
        .reset(&mut app.text_context, "  ")
        .expect("restore original projection");
    let restored = install_action_route(&mut app);
    let table = restored.find(table_id).expect("restored Table");
    assert!(rows.iter().all(|row| table.find(*row).is_some()));
    assert!(table.find(editor).is_some());
    assert_eq!(app.table_note_state.text(), "Ready");
    assert_eq!(app.demo.table_selected_cell, Some((2, 1)));
    assert_eq!(
        app.ui_actions.action_at(rows[1]),
        Some(&DemoAction::SelectTable(2))
    );
    assert_eq!(app.ui_actions.action_for_hit(editor), None);
}

#[test]
fn virtual_list_keeps_row_focus_when_keyboard_moves_across_the_visible_window() {
    nested_collection_child_focus_repairs_structural_changes();
    let mut app = App::new();
    let item = activate_gallery_page(&mut app, "gallery.collections");
    let root = install_action_route(&mut app);
    let layout = LayoutEngine
        .compute(&root, Viewport::new(1280.0, 800.0, 1.0))
        .expect("gallery layout");
    let virtual_id = app.scopes.gallery.element(VIRTUAL_SCROLL);
    let virtual_node = layout.node(virtual_id).expect("virtual viewport");
    let (_, content_height) = virtual_node.scroll_content.expect("virtual content");
    assert_eq!(
        content_height,
        VIRTUAL_ITEM_COUNT as f32 * VIRTUAL_ROW_HEIGHT
    );
    app.demo
        .virtual_scroll
        .sync_content_height(content_height, virtual_node.rect.height);
    let row_four = app.scopes.gallery.element(VIRTUAL_ROW_BASE + 4);
    let row_command = app.scopes.gallery.element(20_000_000 + 4 * 8 + 4);
    assert_eq!(
        app.ui_actions.action_for_hit(row_command),
        Some(&DemoAction::OpenCommandPalette)
    );
    app.set_focus_target(Some(row_command));
    app.demo.virtual_scroll.scroll_by(2_000.0);
    let retained_child = install_action_route(&mut app);
    assert!(
        retained_child.find(row_command).is_some(),
        "focused child retains its owning virtual row"
    );
    assert_eq!(app.focused_virtual_index(), Some(4));
    assert!(
        !app.handle_collection_key_for(STANDALONE_WINDOW, &Key::Named(NamedKey::ArrowDown)),
        "the child keeps its own keyboard semantics"
    );
    assert_eq!(
        collection_item_focus(
            &app.demo,
            &app.ui_actions,
            STANDALONE_WINDOW,
            Some(row_command),
            Some(row_command)
        ),
        None,
        "retention cannot cross the active modal boundary"
    );
    app.demo.virtual_scroll.scroll_by(-2_000.0);
    install_action_route(&mut app);
    app.set_focus_target(Some(row_four));
    assert!(app.handle_collection_key_for(STANDALONE_WINDOW, &Key::Named(NamedKey::ArrowDown)));
    assert!(app.demo.virtual_selection.contains(&5));
    assert_eq!(app.demo.virtual_scroll.offset_y(), VIRTUAL_ROW_HEIGHT);
    install_action_route(&mut app);
    let row_five = app.scopes.gallery.element(VIRTUAL_ROW_BASE + 5);
    assert!(app.ui_actions.focus_order().contains(&row_five));
    assert_eq!(
        app.input_ownership.focus().map(|focus| focus.element),
        Some(row_five)
    );

    assert!(app.handle_collection_key_for(STANDALONE_WINDOW, &Key::Named(NamedKey::End)));
    assert!(
        app.demo
            .virtual_selection
            .contains(&(VIRTUAL_ITEM_COUNT - 1))
    );
    assert_eq!(
        app.demo.virtual_scroll.offset_y(),
        content_height - VIRTUAL_VIEWPORT_HEIGHT
    );
    install_action_route(&mut app);
    let last_row = app
        .scopes
        .gallery
        .element(VIRTUAL_ROW_BASE + (VIRTUAL_ITEM_COUNT - 1) as u32);
    assert!(app.ui_actions.focus_order().contains(&last_row));
    assert_eq!(
        app.input_ownership.focus().map(|focus| focus.element),
        Some(last_row)
    );
    app.demo.virtual_scroll.scroll_by(-340.0);
    install_action_route(&mut app);
    assert!(app.ui_actions.focus_order().contains(&last_row));
    assert_eq!(
        app.input_ownership.focus().map(|focus| focus.element),
        Some(last_row)
    );
    app.demo.virtual_ids.swap(0, VIRTUAL_ITEM_COUNT - 1);
    let reordered = install_action_route(&mut app);
    assert!(app.ui_actions.focus_order().contains(&last_row));
    assert!(app.handle_collection_key_for(STANDALONE_WINDOW, &Key::Named(NamedKey::Home)));
    assert_eq!(app.demo.virtual_scroll.offset_y(), 0.0);
    install_action_route(&mut app);
    assert!(app.ui_actions.focus_order().contains(&last_row));
    assert!(
        app.demo
            .virtual_selection
            .contains(&(VIRTUAL_ITEM_COUNT - 1))
    );
    let (_, focus, route) = collection_window_fixture(
        reordered.find(last_row).expect("retained stable row"),
        DemoAction::SelectVirtual(VIRTUAL_ITEM_COUNT - 1),
    );
    assert_eq!(
        collection_focus(&app.demo, &route, Some(focus), None),
        Some(CollectionFocus::List {
            id: VIRTUAL_ITEM_COUNT - 1,
            index: 0
        }),
        "a foreign window retains the row by stable id after reordering"
    );
    assert_eq!(
        collection_focus(&app.demo, &route, Some(last_row), None),
        None
    );
    app.scroll_geometry
        .pages
        .insert(item, Rect::new(0.0, 64.0, 400.0, 300.0));
    app.scroll_geometry.virtual_list = Some(Rect::new(4.0, 64.0, 320.0, 40.0));
    app.scroll_geometry.unclipped.insert(
        virtual_id,
        Rect::new(4.0, -40.0, 320.0, VIRTUAL_VIEWPORT_HEIGHT),
    );
    app.focus_rects.remove(&virtual_id);
    let page = app.demo.page_scroll.entry(item).or_default();
    page.sync_content_height(1_400.0, 300.0);
    page.scroll_by(800.0);
    let inner = app.demo.virtual_scroll.offset_y();
    app.reveal_window_focus(STANDALONE_WINDOW, virtual_id);
    assert!(
        app.demo.page_scroll[&item].offset_y() < 704.0,
        "Home must reveal a clipped collection viewport through the outer page"
    );
    assert_eq!(
        app.demo.virtual_scroll.offset_y(),
        inner,
        "viewport reveal must preserve the row's selected inner offset"
    );
    let selected = VIRTUAL_ITEM_COUNT - 1;
    app.apply_demo_action(DemoAction::CollectionData(CollectionDataAction::Reverse));
    let reversed = install_action_route(&mut app);
    assert!(
        reversed.find(last_row).is_some(),
        "dynamic reorder retains the focused stable row"
    );
    assert!(app.demo.virtual_selection.contains(&selected));
    app.apply_demo_action(DemoAction::CollectionData(
        CollectionDataAction::RemoveSelected,
    ));
    install_action_route(&mut app);
    assert!(!app.demo.virtual_ids.contains(&selected));
    assert!(!app.demo.virtual_selection.contains(&selected));
    assert!(!app.ui_actions.focus_order().contains(&last_row));
    assert_eq!(
        app.window_focus(STANDALONE_WINDOW),
        Some(app.scopes.gallery.element(VIRTUAL_ROW_BASE + 1)),
        "removal repairs to the stable preceding survivor in this reversed order"
    );
    assert_eq!(app.focused_virtual_index(), Some(VIRTUAL_ITEM_COUNT - 2));
    app.apply_demo_action(DemoAction::CollectionData(CollectionDataAction::Restore));
    install_action_route(&mut app);
    assert_eq!(app.demo.virtual_ids.len(), VIRTUAL_ITEM_COUNT);
    assert!(
        !app.demo.virtual_selection.contains(&selected),
        "data restoration does not resurrect deleted selection"
    );
    exercise_variable_rows();
}

fn nested_collection_child_focus_repairs_structural_changes() {
    let mut missing = Vec::new();
    let mut tree = App::new();
    activate_gallery_page(&mut tree, "gallery.collections");
    let root = install_action_route(&mut tree);
    let layout = LayoutEngine
        .compute(&root, Viewport::new(1280.0, 800.0, 1.0))
        .expect("actual tree layout");
    tree.demo.tree_scroll.sync_layout(
        layout
            .node(tree.scopes.gallery.element(TREE_SCROLL))
            .expect("actual tree viewport"),
    );
    tree.apply_demo_action(DemoAction::SelectVirtualTree(4));
    let child = tree.scopes.gallery.element(21_000_000 + 4 * 8 + 4);
    tree.set_window_focus(STANDALONE_WINDOW, Some(child));
    assert_eq!(
        tree.ui_actions.action_for_hit(child),
        Some(&DemoAction::OpenCommandPalette)
    );
    assert!(!tree.handle_collection_key_for(STANDALONE_WINDOW, &Key::Named(NamedKey::ArrowDown)));
    tree.demo.tree_scroll.scroll_by(2000.0);
    let retained = install_action_route(&mut tree);
    assert!(retained.find(child).is_some());
    tree.apply_demo_action(DemoAction::ToggleVirtualTree(0));
    install_action_route(&mut tree);
    FocusScope::new(
        STANDALONE_WINDOW,
        &tree.ui_actions,
        tree.focus_root_for_window(STANDALONE_WINDOW),
    )
    .reconcile(&mut tree.input_ownership);
    let parent = tree.scopes.gallery.element(TREE_ROW_BASE);
    let focus = tree.window_focus(STANDALONE_WINDOW);
    println!("GPV02-CHILD-REPAIR kind=TreeCollapsed expected={parent:?} actual={focus:?}");
    if focus != Some(parent) {
        missing.push("collapsed child did not return to parent");
    }
    assert!(
        tree.demo.tree_selection.contains(&4),
        "collapse preserves hidden stable selection"
    );
    for reversed in [false, true] {
        let mut list = App::new();
        let item = activate_gallery_page(&mut list, "gallery.collections");
        list.apply_demo_action(DemoAction::SelectVirtual(4));
        install_action_route(&mut list);
        let child = list.scopes.gallery.element(20_000_000 + 4 * 8 + 4);
        list.set_window_focus(STANDALONE_WINDOW, Some(child));
        assert!(
            !list.handle_collection_key_for(STANDALONE_WINDOW, &Key::Named(NamedKey::ArrowDown))
        );
        if reversed {
            list.apply_collection_data(CollectionDataAction::Reverse);
            let live = install_action_route(&mut list);
            assert!(live.find(child).is_some());
            assert_eq!(list.window_focus(STANDALONE_WINDOW), Some(child));
            assert!(list.demo.virtual_selection.contains(&4));
        }
        let old_root = install_action_route(&mut list);
        let old_layout = LayoutEngine
            .compute(&old_root, Viewport::new(1280.0, 800.0, 1.0))
            .expect("published collection geometry before removal");
        list.demo.virtual_scroll.sync_layout(
            old_layout
                .node(list.scopes.gallery.element(VIRTUAL_SCROLL))
                .expect("published list viewport"),
        );
        capture_scroll_geometry(&mut list, &old_layout, &[item]);
        list.focus_rects = list
            .ui_actions
            .focus_order()
            .iter()
            .filter_map(|element| old_layout.node(*element).map(|node| (*element, node.rect)))
            .collect();
        list.apply_collection_data(CollectionDataAction::RemoveSelected);
        let next = if reversed { 3 } else { 5 };
        let expected = list.scopes.gallery.element(VIRTUAL_ROW_BASE + next);
        let pending = InputTarget {
            window: STANDALONE_WINDOW,
            element: expected,
        };
        assert_eq!(
            list.input_ownership.pending_focus_target(STANDALONE_WINDOW),
            Some(pending)
        );
        assert_eq!(
            list.window_focus(STANDALONE_WINDOW),
            None,
            "projection hint does not admit focus before the new tree"
        );
        let expected_index = list
            .demo
            .virtual_ids
            .iter()
            .position(|id| *id == next as usize)
            .expect("live stable fallback");
        assert_eq!(list.focused_virtual_index(), Some(expected_index));
        assert_eq!(
            list.input_ownership.pending_focus_target(STANDALONE_WINDOW),
            Some(pending)
        );
        let mut foreign = list.input_ownership.clone();
        foreign.request_focus(InputTarget {
            window: UiWindowId(999),
            ..pending
        });
        assert_eq!(
            window_collections::collection_projection_focus(
                &list.demo,
                &list.ui_actions,
                &foreign,
                STANDALONE_WINDOW,
                list.scopes.gallery,
                None
            ),
            None
        );
        assert!(foreign.pending_focus_target(UiWindowId(999)).is_some());
        foreign.set_focus(Some(InputTarget {
            window: UiWindowId(999),
            element: list.scopes.gallery.element(VIRTUAL_ROW_BASE),
        }));
        assert_eq!(
            window_collections::collection_projection_focus(
                &list.demo,
                &list.ui_actions,
                &foreign,
                STANDALONE_WINDOW,
                list.scopes.gallery,
                None
            ),
            None
        );
        assert_eq!(
            window_collections::collection_projection_focus(
                &list.demo,
                &list.ui_actions,
                &list.input_ownership,
                STANDALONE_WINDOW,
                list.scopes.gallery,
                Some(child)
            ),
            None,
            "active modal scope must not project a pending row behind it"
        );
        let mut stale = list.input_ownership.clone();
        stale.request_focus(InputTarget {
            window: STANDALONE_WINDOW,
            element: list.scopes.gallery.element(VIRTUAL_ROW_BASE + 4),
        });
        assert_eq!(
            window_collections::collection_projection_focus(
                &list.demo,
                &list.ui_actions,
                &stale,
                STANDALONE_WINDOW,
                list.scopes.gallery,
                None
            ),
            None
        );
        let (root, regions) =
            install_action_route_with_regions(&mut list, Size::new(1280.0, 800.0));
        let mut elements = Vec::new();
        collect_test_elements(
            root.find(list.scopes.gallery.element(VIRTUAL_SCROLL))
                .expect("current virtual viewport"),
            &mut elements,
        );
        let rows = elements
            .iter()
            .filter(|element| {
                matches!(
                    list.ui_actions.action_at(element.id),
                    Some(DemoAction::SelectVirtual(_))
                )
            })
            .count();
        let range = VirtualListRange::compute(
            list.demo.virtual_ids.len(),
            VIRTUAL_ROW_HEIGHT,
            VIRTUAL_VIEWPORT_HEIGHT,
            list.demo.virtual_scroll.offset_y(),
            2,
        )
        .expect("current range");
        let outside = expected_index < range.first || expected_index >= range.end;
        assert_eq!(
            rows,
            range.len() + usize::from(outside),
            "pending target adds at most one current stable offscreen row"
        );
        assert_eq!(
            list.input_ownership.pending_focus_target(STANDALONE_WINDOW),
            None
        );
        FocusScope::new(
            STANDALONE_WINDOW,
            &list.ui_actions,
            list.focus_root_for_window(STANDALONE_WINDOW),
        )
        .reconcile(&mut list.input_ownership);
        assert_eq!(
            FocusScope::new(STANDALONE_WINDOW, &list.ui_actions, None).resolve_request(&mut stale),
            Some(None)
        );
        let focus = list.window_focus(STANDALONE_WINDOW);
        println!(
            "GPV02-CHILD-REPAIR kind=ListRemoved reversed={reversed} expected={expected:?} actual={focus:?}"
        );
        if focus != Some(expected) {
            missing.push(if reversed {
                "offscreen stable neighbor was not projected/focused"
            } else {
                "removed child did not return to following row"
            });
        }
        assert!(!list.demo.virtual_ids.contains(&4));
        assert!(!list.demo.virtual_selection.contains(&4));
        let current_layout = LayoutEngine
            .compute(&root, Viewport::new(1280.0, 800.0, 1.0))
            .expect("current collection geometry after removal");
        let actions = list.ui_actions.clone();
        let published_rects = list.focus_rects.clone();
        let old_offset = list.demo.virtual_scroll.offset();
        list.reveal_current_layout_focus(
            UiWindowId(999),
            list.scopes.gallery,
            &current_layout,
            &actions,
            &regions,
            expected,
        );
        assert_eq!(list.demo.virtual_scroll.offset(), old_offset);
        list.reveal_current_layout_focus(
            STANDALONE_WINDOW,
            list.scopes.gallery,
            &current_layout,
            &actions,
            &regions,
            expected,
        );
        assert_eq!(
            list.focus_rects, published_rects,
            "future frame is not published before paint"
        );
        let offset = list.demo.virtual_scroll.offset_y();
        let expected_offset =
            (((expected_index + 1) as f32 * VIRTUAL_ROW_HEIGHT) - VIRTUAL_VIEWPORT_HEIGHT + 2.0)
                .max(0.0);
        println!(
            "GPV02-STRUCTURAL-REVEAL reversed={reversed} index={expected_index} offset={offset} expected={expected_offset}"
        );
        if reversed && (offset - expected_offset).abs() > 0.01 {
            missing.push("restored offscreen row was not revealed");
        }
        if !reversed && (offset - expected_offset).abs() > 0.01 {
            missing.push("restore revealed stale pre-removal row geometry");
        }
        let visible_root = install_action_route(&mut list);
        let visible_layout = LayoutEngine
            .compute(&visible_root, Viewport::new(1280.0, 800.0, 1.0))
            .expect("restored row after scroll rebuild");
        let row = visible_layout
            .node(expected)
            .expect("restored visible row")
            .rect;
        let viewport = visible_layout
            .node(list.scopes.gallery.element(VIRTUAL_SCROLL))
            .expect("rebuilt viewport")
            .scroll_viewport();
        assert!(row.y >= viewport.y && row.y + row.height <= viewport.y + viewport.height);
        assert_eq!(list.window_focus(STANDALONE_WINDOW), Some(expected));
        assert!(!list.demo.virtual_selection.contains(&4));
        println!(
            "GPV02-PENDING-ROW reversed={reversed} visible={} built={rows} index={expected_index} outside={outside}",
            range.len()
        );
    }
    assert!(
        missing.is_empty(),
        "structural row ownership gaps: {missing:?}"
    );
}

fn exercise_variable_rows() {
    let mut app = App::new();
    activate_gallery_page(&mut app, "gallery.collections");
    app.apply_demo_action(DemoAction::CollectionData(
        CollectionDataAction::ToggleRowHeights,
    ));
    let root = install_action_route(&mut app);
    let layout = LayoutEngine
        .compute(&root, Viewport::new(1280.0, 800.0, 1.0))
        .expect("measured Gallery layout");
    let viewport_id = app.scopes.gallery.element(VIRTUAL_SCROLL);
    let viewport = layout.node(viewport_id).expect("measured viewport");
    assert_eq!(
        viewport.scroll_content.expect("measured extent").1,
        373_352.0
    );
    let row = |id: u32| app.scopes.gallery.element(VIRTUAL_ROW_BASE + id);
    assert_eq!(layout.node(row(0)).expect("tall object").rect.height, 56.0);
    assert_eq!(layout.node(row(1)).expect("short object").rect.height, 28.0);
    assert_eq!(
        layout.node(row(3)).expect("second tall object").rect.y - viewport.rect.y,
        112.0
    );
    let row_three = row(3);
    let row_four = row(4);
    app.set_focus_target(Some(row_three));
    assert!(app.handle_collection_key_for(STANDALONE_WINDOW, &Key::Named(NamedKey::ArrowDown)));
    assert_eq!(
        app.demo.virtual_scroll.offset_y(),
        56.0,
        "reveal must use the accepted height prefix rather than index times fixed height"
    );
    install_action_route(&mut app);
    assert_eq!(app.window_focus(STANDALONE_WINDOW), Some(row_four));
    assert!(app.handle_collection_key_for(STANDALONE_WINDOW, &Key::Named(NamedKey::End)));
    assert_eq!(
        app.demo.virtual_scroll.offset_y(),
        373_352.0 - VIRTUAL_VIEWPORT_HEIGHT
    );
    let root = install_action_route(&mut app);
    let last = app.scopes.gallery.element(VIRTUAL_ROW_BASE + 9_999);
    let layout = LayoutEngine
        .compute(&root, Viewport::new(1280.0, 800.0, 1.0))
        .expect("measured bottom layout");
    let viewport = layout.node(viewport_id).expect("viewport").rect;
    let last_rect = layout.node(last).expect("last tall row").rect;
    assert_eq!(last_rect.bottom() - viewport.y, VIRTUAL_VIEWPORT_HEIGHT);
    assert_eq!(last_rect.height, 56.0);
    app.apply_demo_action(DemoAction::CollectionData(CollectionDataAction::Reverse));
    install_action_route(&mut app);
    assert_eq!(app.focused_virtual_index(), Some(0));
    assert_eq!(
        app.demo
            .virtual_metrics
            .as_ref()
            .expect("rebuilt metrics")
            .row_bounds(0),
        Some((0.0, 56.0))
    );
    assert!(app.handle_collection_key_for(STANDALONE_WINDOW, &Key::Named(NamedKey::Home)));
    assert_eq!(app.demo.virtual_scroll.offset_y(), 0.0);
    install_action_route(&mut app);
    let child = app.scopes.gallery.element(20_000_000 + 9_999 * 8 + 4);
    app.set_focus_target(Some(child));
    for _ in 0..2 {
        app.demo.virtual_scroll.scroll_by(2_000.0);
        let root = install_action_route(&mut app);
        assert!(root.find(child).is_some());
        assert_eq!(
            app.ui_actions.action_at(child),
            Some(&DemoAction::OpenCommandPalette)
        );
        assert_eq!(app.window_focus(STANDALONE_WINDOW), Some(child));
        app.apply_demo_action(DemoAction::CollectionData(
            CollectionDataAction::ToggleRowHeights,
        ));
        install_action_route(&mut app);
        assert_eq!(
            app.window_focus(STANDALONE_WINDOW),
            Some(child),
            "height policy changes keep the same live child owner"
        );
    }
    assert!(app.demo.virtual_metrics.is_some());
    app.apply_demo_action(DemoAction::CollectionData(
        CollectionDataAction::RemoveSelected,
    ));
    let root = install_action_route(&mut app);
    assert!(root.find(child).is_none());
    // The lightweight route installer resolves pending focus only. The native
    // frame also reconciles live owners after publishing its rebuilt route.
    FocusScope::new(
        STANDALONE_WINDOW,
        &app.ui_actions,
        app.focus_root_for_window(STANDALONE_WINDOW),
    )
    .reconcile(&mut app.input_ownership);
    assert_ne!(app.window_focus(STANDALONE_WINDOW), Some(child));
    assert!(!app.demo.virtual_selection.contains(&9_999));
    assert_eq!(
        app.demo
            .virtual_metrics
            .as_ref()
            .expect("deletion metrics")
            .len(),
        9_999
    );
    app.apply_demo_action(DemoAction::CollectionData(CollectionDataAction::Restore));
    install_action_route(&mut app);
    assert_eq!(
        app.demo
            .virtual_metrics
            .as_ref()
            .expect("restored metrics")
            .len(),
        10_000
    );
    assert!(!app.demo.virtual_selection.contains(&9_999));
}
