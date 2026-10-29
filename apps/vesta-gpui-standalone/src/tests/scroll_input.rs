//! Existing wheel/thumb and nested ownership regressions; no native qualification.
use super::*;

pub(super) fn scrollbar_thumb_precedes_window_chrome() {
    let mut app = App::new();
    let item = activate_gallery_page(&mut app, "gallery.layout");
    let size = Size::new(720.0, 640.0);
    let root = install_action_route_with_viewport(&mut app, size);
    let layout = LayoutEngine
        .compute(&root, Viewport::new(size.width, size.height, 1.0))
        .expect("edge scrollbar layout");
    let page_id = page_scroll_id(app.scopes.gallery, item);
    let node = layout.node(page_id).expect("current Layout page");
    let state = app.demo.page_scroll.entry(item).or_default();
    state.sync_layout(node);
    let (_, thumb, _, _) = node.scrollbar_geometry(state.offset());
    let thumb = thumb.expect("overflowing page thumb");
    app.cursor = Point::new(thumb.x + thumb.width * 0.5, thumb.y + thumb.height * 0.5);
    assert_eq!(
        chrome_resize_direction(app.cursor, size),
        Some(ResizeDirection::East),
        "the actual painted thumb overlaps the native frame resize strip"
    );
    app.hit_test = HitTestTree::build(&root, &layout);
    capture_scroll_geometry(&mut app, &layout, &[item]);
    app.ui_query.input_origin = vesta_gpui_winit::UiInputOrigin::Native;
    assert!(
        app.try_native_primary_gesture(),
        "the native primary boundary must consume a live thumb before frame handling"
    );
    assert_eq!(
        app.input_ownership.capture().map(|capture| capture.target),
        Some(InputTarget {
            window: STANDALONE_WINDOW,
            element: page_id,
        })
    );
    let before = app
        .demo
        .page_scroll
        .get(&item)
        .expect("page state")
        .offset();
    app.cursor.y += 32.0;
    app.update_scrollbar_drag();
    assert!(
        app.demo
            .page_scroll
            .get(&item)
            .expect("page state")
            .offset_y()
            > before.y
    );
}

#[test]
fn overflowing_text_area_uses_wheel_before_the_dock_page() {
    let mut app = App::new();
    let item = activate_gallery_page(&mut app, "gallery.inputs");
    let area_id = app.scopes.gallery.element(TEXT_AREA);
    let page_id = page_scroll_id(app.scopes.gallery, item);
    let long_text = (0..30)
        .map(|line| format!("line {line}"))
        .collect::<Vec<_>>()
        .join("\n");
    app.area_state
        .reset(&mut app.text_context, &long_text)
        .expect("multiline sample");
    let root = install_action_route(&mut app);
    let layout = LayoutEngine
        .compute(&root, Viewport::new(1280.0, 800.0, 1.0))
        .expect("gallery layout");
    let page = layout.node(page_id).expect("active page");
    let content_height = page.scroll_content.expect("page content").1;
    let scroll = app.demo.page_scroll.entry(item).or_default();
    scroll.sync_content_height(content_height, page.rect.height);
    scroll.reveal(
        layout.node(area_id).expect("text area").rect,
        page.rect,
        8.0,
    );
    let root = install_action_route(&mut app);
    let layout = LayoutEngine
        .compute(&root, Viewport::new(1280.0, 800.0, 1.0))
        .expect("visible text area layout");
    app.hit_test = HitTestTree::build(&root, &layout);
    let area = layout.node(area_id).expect("text area").rect;
    app.page_scroll_rects
        .insert(item, layout.node(page_id).expect("active page").rect);
    capture_scroll_geometry(&mut app, &layout, &[item]);
    app.area_state
        .set_wrap_width(&mut app.text_context, text_input_wrap_width(area));
    app.area_view = Some(
        app.area_state
            .snapshot()
            .viewport(area, area, Point::default()),
    );
    app.cursor = Point::new(area.x + 20.0, area.y + 20.0);
    assert_eq!(app.hit_test.hit(app.cursor), Some(area_id));
    let page_before = app
        .demo
        .page_scroll
        .get(&item)
        .expect("active page scroll")
        .offset_y();
    app.wheel(MouseScrollDelta::LineDelta(0.0, -1.0));
    assert_eq!(app.area_scroll_y, 34.0);
    assert!(!app.area_follow_caret);
    assert_eq!(
        app.demo
            .page_scroll
            .get(&item)
            .expect("page scroll")
            .offset_y(),
        page_before
    );
    app.wheel(MouseScrollDelta::LineDelta(0.0, -100.0));
    assert!(app.area_scroll_y > 34.0);
    assert!(
        app.demo
            .page_scroll
            .get(&item)
            .expect("page scroll")
            .offset_y()
            > page_before,
        "overflow remainder should reach the dock page: before {page_before}, after {}, max {}",
        app.demo
            .page_scroll
            .get(&item)
            .expect("page scroll")
            .offset_y(),
        app.demo
            .page_scroll
            .get(&item)
            .expect("page scroll")
            .max_y(),
    );
    interleaved_area_widths_converge();
}

fn interleaved_area_widths_converge() {
    let mut app = App::new();
    activate_gallery_page(&mut app, "gallery.inputs");
    app.area_state
        .reset(
            &mut app.text_context,
            &"one two three four five six seven eight ".repeat(30),
        )
        .expect("wrapping source");
    let mut widths = [0.0; 2];
    let mut sources: [AreaWrapLayout; 2] = std::array::from_fn(|_| AreaWrapLayout::default());
    let mut presented: [Option<(vesta_gpui::TextInputSnapshot, Rect)>; 2] =
        std::array::from_fn(|_| None);
    for pass in 0..4 {
        for (source, width) in [1280.0, 360.0].into_iter().enumerate() {
            let (snapshot, bounds, changed) =
                project_area_width(&mut app, &mut sources[source], width);
            widths[source] = bounds.width;
            if pass > 1 {
                assert!(
                    !changed,
                    "alternating settled source must not defer every frame: source={source} width={} pass={pass}",
                    widths[source]
                );
                assert_eq!(
                    snapshot.layout_height(),
                    app.area_state.snapshot().layout_height()
                );
                presented[source] = Some((snapshot, bounds));
            }
        }
    }
    assert!(
        widths[0] > widths[1],
        "fixture has distinct presenting widths"
    );
    let (wide, bounds) = presented[0].take().expect("settled wide source");
    let (narrow, _) = presented[1].take().expect("settled narrow source");
    let wide_height = wide.layout_height();
    assert!(
        narrow.layout_height() > wide_height,
        "distinct Parley wrapping"
    );
    assert_eq!(
        app.area_state.snapshot().layout_height(),
        narrow.layout_height()
    );

    // The other source just reflowed the shared editor. A click in the earlier
    // presented viewport must still map through that viewport's shaped width.
    let view = wide
        .clone()
        .with_viewport_scroll(Point::default(), false)
        .viewport(bounds, bounds, Point::default());
    let cursor = wide.layout_cursor(60).expect("source cursor").bounds();
    let local = Point::new(cursor.x + 0.1, cursor.y + cursor.height * 0.5);
    let expected = wide.byte_index_at(local).expect("painted source position");
    app.area_view = Some(view);
    app.begin_text_selection(
        STANDALONE_WINDOW,
        TextFieldKind::Area,
        Some(Point::new(view.origin.x + local.x, view.origin.y + local.y)),
    );
    assert_eq!(
        app.area_state.inspection_selection(),
        Some((expected, expected))
    );
    let before = app.area_state.text().to_owned();
    assert!(app.handle_window_text_event(
        STANDALONE_WINDOW,
        &WindowEvent::Ime(winit::event::Ime::Enabled)
    ));
    assert!(app.handle_window_text_event(
        STANDALONE_WINDOW,
        &WindowEvent::Ime(winit::event::Ime::Preedit("preview".into(), None))
    ));
    for _ in 0..3 {
        for (source, width) in [1280.0, 360.0].into_iter().enumerate() {
            assert!(!project_area_width(&mut app, &mut sources[source], width).2);
            assert!(app.area_state.is_composing());
            assert_eq!(app.area_state.text(), before);
            assert_eq!(
                wide.layout_height(),
                wide_height,
                "held paint stays immutable"
            );
        }
    }
    assert!(app.handle_window_text_event(
        STANDALONE_WINDOW,
        &WindowEvent::Ime(winit::event::Ime::Commit("done".into()))
    ));
    assert!(!app.area_state.is_composing());
    assert_ne!(app.area_state.text(), before);
    assert!(
        app.area_state
            .act(&mut app.text_context, vesta_gpui::TextInputAction::Undo)
    );
    assert_eq!(
        app.area_state.text(),
        before,
        "reflow must not add undo steps"
    );
}

fn project_area_width(
    app: &mut App,
    source: &mut AreaWrapLayout,
    width: f32,
) -> (vesta_gpui::TextInputSnapshot, Rect, bool) {
    source.prepare(&mut app.area_state, &mut app.text_context);
    let root = install_action_route_with_viewport(app, Size::new(width, 600.0));
    let layout = LayoutEngine
        .compute(&root, Viewport::new(width, 600.0, 1.0))
        .expect("actual Gallery projection");
    let area_id = app.scopes.gallery.element(TEXT_AREA);
    let bounds = layout.node(area_id).expect("actual TextArea").rect;
    let paint = PaintList::build(&root, &layout);
    let snapshot = paint
        .commands()
        .iter()
        .find_map(|command| match command {
            vesta_gpui::PaintCommand::TextInput {
                element, snapshot, ..
            } if *element == area_id => Some(snapshot.clone()),
            _ => None,
        })
        .expect("painted TextArea snapshot");
    let changed = source.sync(
        &mut app.area_state,
        &mut app.text_context,
        app.scopes.gallery,
        &layout,
    );
    (snapshot, bounds, changed)
}

#[test]
fn nested_scroll_consumes_wheel_before_the_dock_page() {
    let mut app = App::new();
    let item = activate_gallery_page(&mut app, "gallery.collections");
    let nested_id = app.scopes.gallery.element(NESTED_SCROLL);
    let page_id = page_scroll_id(app.scopes.gallery, item);
    let root = install_action_route(&mut app);
    let layout = LayoutEngine
        .compute(&root, Viewport::new(1280.0, 800.0, 1.0))
        .expect("gallery layout");
    app.hit_test = HitTestTree::build(&root, &layout);
    let page = layout.node(page_id).expect("active dock page");
    let page_content_height = page
        .scroll_content
        .map(|content| content.1)
        .expect("dock page content height");
    app.demo
        .page_scroll
        .entry(item)
        .or_default()
        .sync_content_height(page_content_height, page.rect.height);
    app.page_scroll_rects.insert(item, page.rect);
    capture_scroll_geometry(&mut app, &layout, &[item]);
    app.demo.nested_scroll.sync_content_height(192.0, 70.0);
    let nested = layout.node(nested_id).expect("nested scroll").rect;
    app.nested_scroll_rect = Some(nested);
    app.gallery_scroll_rect = layout
        .node(app.scopes.gallery.element(3))
        .map(|node| node.rect);
    let last_item = app
        .ui_actions
        .focus_order()
        .iter()
        .copied()
        .filter(|id| app.ui_actions.is_descendant_of(*id, nested_id))
        .next_back()
        .expect("last nested item");
    app.focus_rects.insert(
        last_item,
        layout.node(last_item).expect("last item layout").rect,
    );
    invalid_gallery_reveals_preserve_owners(
        &mut app,
        last_item,
        page_id,
        layout.node(last_item).expect("last item layout").rect,
    );
    app.reveal_window_focus(STANDALONE_WINDOW, last_item);
    assert!(app.demo.nested_scroll.offset_y() > 0.0);
    app.demo.nested_scroll = ScrollAreaState::default();
    app.demo.nested_scroll.sync_content_height(192.0, 70.0);
    let root = install_action_route(&mut app);
    let layout = LayoutEngine
        .compute(&root, Viewport::new(1280.0, 800.0, 1.0))
        .expect("revealed gallery layout");
    app.hit_test = HitTestTree::build(&root, &layout);
    let nested = layout.node(nested_id).expect("revealed nested scroll").rect;
    app.nested_scroll_rect = Some(nested);
    capture_scroll_geometry(&mut app, &layout, &[item]);
    app.cursor = Point::new(nested.x + 20.0, nested.y + 20.0);
    let hit = app.hit_test.hit(app.cursor).expect("nested wheel target");
    assert!(app.ui_actions.is_descendant_of(hit, nested_id));
    let page_before = app
        .demo
        .page_scroll
        .get(&item)
        .expect("active page scroll")
        .offset_y();
    app.wheel(MouseScrollDelta::LineDelta(0.0, -5.0));
    assert_eq!(
        app.demo.nested_scroll.offset_y(),
        app.demo.nested_scroll.max_y()
    );
    assert!(
        app.demo
            .page_scroll
            .get(&item)
            .expect("active page scroll")
            .offset_y()
            > page_before,
        "wheel remainder must scroll the active dock page: before {page_before}, after {}, max {}, content {page_content_height}, viewport {}",
        app.demo
            .page_scroll
            .get(&item)
            .expect("active page scroll")
            .offset_y(),
        app.demo
            .page_scroll
            .get(&item)
            .expect("active page scroll")
            .max_y(),
        page.rect.height,
    );
    for flow in [FlowDirection::LeftToRight, FlowDirection::RightToLeft] {
        for scale in [1.0, 1.25, 1.5, 2.0] {
            nested_axes_preserve_exact_remainders(flow, scale);
        }
    }
}

fn invalid_gallery_reveals_preserve_owners(
    app: &mut App,
    target: ElementId,
    outer: ElementId,
    target_rect: Rect,
) {
    let before = (
        app.demo.nested_scroll,
        app.demo.page_scroll.clone(),
        app.gallery_scroll,
    );
    let geometry = app.scroll_geometry.clone();
    let focus = app.input_ownership.focus();
    let selection = app.demo.selected_list;
    let invalid = [
        Rect::new(
            f32::NAN,
            target_rect.y,
            target_rect.width,
            target_rect.height,
        ),
        Rect::new(target_rect.x, target_rect.y, -1.0, target_rect.height),
        Rect::new(f32::MAX, target_rect.y, f32::MAX, target_rect.height),
    ];
    let mut failures = Vec::new();
    for invalid in invalid {
        for invalid_outer in [false, true] {
            app.demo.nested_scroll = before.0;
            app.demo.page_scroll = before.1.clone();
            app.gallery_scroll = before.2;
            app.scroll_geometry = geometry.clone();
            if invalid_outer {
                app.scroll_geometry.unclipped.insert(outer, invalid);
            }
            app.reveal_window_rect(
                STANDALONE_WINDOW,
                target,
                if invalid_outer { target_rect } else { invalid },
            );
            if app.demo.nested_scroll != before.0
                || app.demo.page_scroll != before.1
                || app.gallery_scroll != before.2
                || app.input_ownership.focus() != focus
                || app.demo.selected_list != selection
            {
                failures.push(format!(
                    "outer={invalid_outer} nested={:?} pages={:?} gallery={:?}",
                    app.demo.nested_scroll.offset(),
                    app.demo.page_scroll,
                    app.gallery_scroll.offset()
                ));
            }
        }
    }
    app.demo.nested_scroll = before.0;
    app.demo.page_scroll = before.1;
    app.gallery_scroll = before.2;
    app.scroll_geometry = geometry;
    assert!(
        failures.is_empty(),
        "invalid Gallery reveal must preserve all current owners: {failures:?}"
    );
}

fn nested_axes_preserve_exact_remainders(flow: FlowDirection, scale: f32) {
    fn set_flow(element: &mut Element, target: ElementId, flow: FlowDirection) -> bool {
        if element.id == target {
            element.style.flow_direction = Some(flow);
            return true;
        }
        element
            .children
            .iter_mut()
            .any(|child| set_flow(child, target, flow))
    }
    fn offer(app: &mut App, hit: ElementId, movement: Point) -> Point {
        let mut states = ScrollStates {
            tree: &mut app.demo.tree_scroll,
            virtual_list: &mut app.demo.virtual_scroll,
            virtual_table: &mut app.demo.virtual_table_scroll,
            nested: &mut app.demo.nested_scroll,
            horizontal: &mut app.demo.horizontal_scroll,
            gallery: Some(&mut app.gallery_scroll),
            pages: &mut app.demo.page_scroll,
            tabs: &mut app.demo.tab_scroll,
        };
        wheel_scroll_path(
            STANDALONE_WINDOW,
            &app.ui_actions,
            &mut states,
            Some(hit),
            &app.scroll_geometry,
            app.scopes.gallery,
            movement,
        )
    }
    let mut app = App::new();
    let item = activate_gallery_page(&mut app, "gallery.collections");
    let nested_id = app.scopes.gallery.element(NESTED_SCROLL);
    let page_id = page_scroll_id(app.scopes.gallery, item);
    let other_item = app
        .demo
        .workspace
        .panes()
        .into_iter()
        .filter_map(|pane| pane.active)
        .find(|candidate| *candidate != item)
        .expect("another actual Gallery pane");
    let mut root = install_action_route(&mut app);
    assert!(set_flow(&mut root, nested_id, flow));
    let layout = LayoutEngine
        .compute(&root, Viewport::new(1280.0, 800.0, scale))
        .expect("actual Gallery with explicit nested flow");
    capture_scroll_geometry(&mut app, &layout, &[item, other_item]);
    let nested = layout.node(nested_id).expect("nested viewport");
    app.demo.nested_scroll.sync_layout(nested);
    let max = app.demo.nested_scroll.max();
    assert!(
        max.x > 0.0 && max.y > 0.0,
        "reserved gutters give both axes range"
    );
    let page = app.demo.page_scroll.entry(item).or_default();
    page.sync_layout(layout.node(page_id).expect("outer page"));
    page.scroll_by(100.0);
    assert!(
        page.max_y() >= 131.0,
        "fixture has room for exact forward/reverse remainder"
    );
    app.demo
        .page_scroll
        .insert(other_item, ScrollAreaState::new(37.0));
    let other = app.demo.page_scroll[&other_item];
    let focus = app.input_ownership.focus();
    let selection = app.demo.selected_list;
    let physical_x = |logical: f32| match flow {
        FlowDirection::LeftToRight => logical,
        FlowDirection::RightToLeft => -logical,
    };
    assert_eq!(
        offer(
            &mut app,
            nested_id,
            Point::new(physical_x(max.x + 37.0), max.y + 31.0)
        ),
        Point::new(physical_x(37.0), 0.0)
    );
    assert_eq!(app.demo.nested_scroll.offset(), max);
    assert_eq!(app.demo.page_scroll[&item].offset_y(), 131.0);
    assert_eq!(
        offer(
            &mut app,
            nested_id,
            Point::new(physical_x(-max.x - 47.0), -max.y - 41.0)
        ),
        Point::new(physical_x(-47.0), 0.0)
    );
    assert_eq!(app.demo.nested_scroll.offset(), Point::default());
    assert_eq!(app.demo.page_scroll[&item].offset_y(), 90.0);
    app.demo
        .page_scroll
        .get_mut(&item)
        .expect("page")
        .scroll_by(-f32::MAX);
    let untouched = Point::new(physical_x(-19.0), -23.0);
    assert_eq!(offer(&mut app, nested_id, untouched), untouched);
    assert_eq!(app.demo.nested_scroll.offset(), Point::default());
    assert_eq!(app.demo.page_scroll[&item].offset_y(), 0.0);
    let scope = FocusScope::new(
        STANDALONE_WINDOW,
        &app.ui_actions,
        Some(app.scopes.gallery.element(3)),
    );
    assert_eq!(
        vesta_gpui_widgets::scroll_wheel_path(
            &scope,
            InputTarget {
                window: UiWindowId(999),
                element: nested_id
            },
            Point::new(17.0, 19.0),
            |_, _| panic!("a foreign source must not enter this Gallery scroll path"),
        ),
        Point::new(17.0, 19.0)
    );
    assert_eq!(app.demo.page_scroll[&other_item], other);
    assert_eq!(app.input_ownership.focus(), focus);
    assert_eq!(app.demo.selected_list, selection);
}

#[test]
fn horizontal_wheel_moves_the_inner_scroll_without_moving_the_dock_page() {
    gallery_table_header_follows_its_body_in_both_directions();
    #[derive(Clone, Copy, Debug)]
    enum Exit {
        Escape,
        Resize,
        Scale,
        Hide,
    }
    for exit in [Exit::Escape, Exit::Resize, Exit::Scale, Exit::Hide] {
        let mut app = App::new();
        let item = activate_gallery_page(&mut app, "gallery.collections");
        let horizontal_id = app.scopes.gallery.element(HORIZONTAL_SCROLL);
        let page_id = page_scroll_id(app.scopes.gallery, item);
        let root = install_action_route(&mut app);
        let layout = LayoutEngine
            .compute(&root, Viewport::new(1280.0, 800.0, 1.0))
            .expect("gallery layout");
        let page = layout.node(page_id).expect("active dock page");
        let page_height = page
            .scroll_content
            .map(|content| content.1)
            .expect("dock page content height");
        app.demo
            .page_scroll
            .entry(item)
            .or_default()
            .sync_content_height(page_height, page.rect.height);
        app.page_scroll_rects.insert(item, page.rect);
        let horizontal_rect = layout.node(horizontal_id).expect("horizontal scroll").rect;
        app.demo
            .page_scroll
            .get_mut(&item)
            .expect("active page scroll")
            .reveal(horizontal_rect, page.rect, 8.0);
        let root = install_action_route(&mut app);
        let layout = LayoutEngine
            .compute(&root, Viewport::new(1280.0, 800.0, 1.0))
            .expect("layout with horizontal scroll in view");
        let node = layout.node(horizontal_id).expect("horizontal scroll");
        let (content_width, content_height) = node.scroll_content.expect("horizontal content");
        app.demo.horizontal_scroll.sync_content_size(
            Size::new(content_width, content_height),
            Size::new(node.rect.width, node.rect.height),
        );
        app.hit_test = HitTestTree::build(&root, &layout);
        capture_scroll_geometry(&mut app, &layout, &[item]);
        app.cursor = Point::new(node.rect.x + 20.0, node.rect.y + 20.0);
        let hit = app
            .hit_test
            .hit(app.cursor)
            .expect("horizontal wheel target");
        assert!(app.ui_actions.is_descendant_of(hit, horizontal_id));

        let outer_y = app
            .demo
            .page_scroll
            .get(&item)
            .expect("active page scroll")
            .offset_y();
        app.wheel(MouseScrollDelta::LineDelta(-4.0, 0.0));
        assert!(app.demo.horizontal_scroll.offset().x > 0.0);
        assert_eq!(
            app.demo
                .page_scroll
                .get(&item)
                .expect("active page scroll")
                .offset_y(),
            outer_y
        );

        let root = install_action_route(&mut app);
        let layout = LayoutEngine
            .compute(&root, Viewport::new(1280.0, 800.0, 1.0))
            .expect("layout after horizontal wheel");
        app.hit_test = HitTestTree::build(&root, &layout);
        let viewport = layout.node(horizontal_id).expect("scrollbar viewport").rect;
        app.horizontal_scroll_rect = Some(viewport);
        let (_, _, _, thumb) = vesta_gpui::scrollbar_geometry(
            viewport,
            Size::new(content_width, content_height),
            app.demo.horizontal_scroll.offset(),
        );
        let thumb = thumb.expect("horizontal thumb");
        app.cursor = Point::new(thumb.x + 3.0, thumb.y + 3.0);
        assert!(app.try_scrollbar_press());
        assert_eq!(
            app.input_ownership
                .capture()
                .map(|capture| capture.target.element),
            Some(horizontal_id)
        );
        let before_drag = app.demo.horizontal_scroll.offset().x;
        app.cursor.x += 100.0;
        app.update_scrollbar_drag();
        assert!(app.demo.horizontal_scroll.offset().x > before_drag);
        let captured = app.input_ownership.capture();
        for button in [
            MouseButton::Right,
            MouseButton::Middle,
            MouseButton::Back,
            MouseButton::Forward,
            MouseButton::Other(7),
        ] {
            let reached = app.demo.horizontal_scroll.offset();
            assert!(
                !finish_scrollbar_drag(&mut app.scrollbar_drag, button),
                "{button:?} cannot finish the primary thumb gesture"
            );
            assert!(app.scrollbar_drag.is_some());
            assert_eq!(app.input_ownership.capture(), captured);
            assert_eq!(app.demo.horizontal_scroll.offset(), reached);
            app.cursor.x -= 2.0;
            app.update_scrollbar_drag();
            assert!(
                app.demo.horizontal_scroll.offset().x < reached.x,
                "primary motion continues after {button:?} release"
            );
        }
        let active_drag = app.scrollbar_drag.expect("active thumb drag");
        app.cancel_window_value_edits(UiWindowId(99));
        assert!(
            app.scrollbar_drag.is_some(),
            "foreign cleanup keeps this drag"
        );
        assert_eq!(app.input_ownership.capture(), captured);
        if matches!(exit, Exit::Hide) {
            native_capture::qualify_scrollbar_loss(&mut app);
        }
        let reached = app.demo.horizontal_scroll.offset();
        match exit {
            Exit::Escape => {
                assert!(app.handle_window_key(STANDALONE_WINDOW, &Key::Named(NamedKey::Escape)))
            }
            Exit::Resize => app.main_window_resized(winit::dpi::PhysicalSize::new(1280, 800)),
            Exit::Scale => app.main_window_scale_factor_changed(),
            Exit::Hide => {}
        }
        assert!(app.scrollbar_drag.is_none(), "{exit:?} ends the drag");
        assert!(
            app.input_ownership.capture().is_none(),
            "{exit:?} must retire scrollbar capture"
        );
        assert_eq!(app.demo.horizontal_scroll.offset(), reached);
        app.cursor.x -= 80.0;
        app.update_scrollbar_drag();
        assert_eq!(
            app.demo.horizontal_scroll.offset(),
            reached,
            "late motion cannot resume {exit:?}"
        );
        assert!(
            !app.input_ownership
                .release_pointer(STANDALONE_WINDOW, PointerButton::Primary)
        );
        assert_eq!(app.demo.horizontal_scroll.offset(), reached);
        if matches!(exit, Exit::Hide) {
            continue;
        }
        for (replacement, button) in [
            (
                InputTarget {
                    window: UiWindowId(99),
                    element: horizontal_id,
                },
                PointerButton::Primary,
            ),
            (
                InputTarget {
                    window: STANDALONE_WINDOW,
                    element: app.scopes.header.element(MODE_2D),
                },
                PointerButton::Primary,
            ),
            (
                InputTarget {
                    window: STANDALONE_WINDOW,
                    element: horizontal_id,
                },
                PointerButton::Secondary,
            ),
        ] {
            app.scrollbar_drag = Some(active_drag);
            app.input_ownership.capture_pointer(replacement, button);
            let replacement_capture = app.input_ownership.capture();
            match exit {
                Exit::Escape => {
                    assert!(app.handle_window_key(STANDALONE_WINDOW, &Key::Named(NamedKey::Escape)))
                }
                Exit::Resize => app.main_window_resized(winit::dpi::PhysicalSize::new(1280, 800)),
                Exit::Scale => app.main_window_scale_factor_changed(),
                Exit::Hide => unreachable!("hide fixture already verifies its late terminals"),
            }
            assert!(app.scrollbar_drag.is_none());
            assert_eq!(
                app.input_ownership.capture(),
                replacement_capture,
                "{exit:?} must preserve a replacement owner/button"
            );
            app.input_ownership.cancel_capture();
        }
        app.scrollbar_drag = Some(active_drag);
        assert!(finish_scrollbar_drag(
            &mut app.scrollbar_drag,
            MouseButton::Left
        ));
        assert!(app.scrollbar_drag.is_none());
        assert_eq!(app.demo.horizontal_scroll.offset(), reached);
        assert!(!finish_scrollbar_drag(
            &mut app.scrollbar_drag,
            MouseButton::Left
        ));
    }
}

fn gallery_table_header_follows_its_body_in_both_directions() {
    fn constrain(element: &mut Element, table: ElementId, flow: FlowDirection) -> bool {
        if element.id == table {
            element.style.width = Dimension::Px(140.0);
            element.style.max_width = Dimension::Px(140.0);
            element.style.flow_direction = Some(flow);
            return true;
        }
        element
            .children
            .iter_mut()
            .any(|child| constrain(child, table, flow))
    }
    for (flow, scale) in [FlowDirection::LeftToRight, FlowDirection::RightToLeft]
        .into_iter()
        .flat_map(|flow| [1.0, 1.25, 1.5, 2.0].map(|scale| (flow, scale)))
    {
        let mut app = App::new();
        activate_gallery_page(&mut app, "gallery.collections");
        let scope = app.scopes.gallery;
        let table = scope.element(TABLE_VIRTUAL_ROOT);
        let viewport = scope.element(TABLE_VIRTUAL_ROOT + 2);
        let column = scope.element(TABLE_VIRTUAL_ROOT + 10);
        let cell = scope.element(TABLE_VIRTUAL_ROOT + 1001);
        let mut root = install_action_route(&mut app);
        assert!(constrain(&mut root, table, flow));
        let before = LayoutEngine
            .compute(&root, Viewport::new(1280.0, 800.0, scale))
            .expect("actual constrained Gallery Table");
        app.demo
            .virtual_table_scroll
            .sync_layout(before.node(viewport).expect("current Table viewport"));
        assert!(app.demo.virtual_table_scroll.max().x >= 30.0);
        app.demo
            .virtual_table_scroll
            .scroll_by_point(Point::new(flow.horizontal(30.0), 0.0));
        let offset = app.demo.virtual_table_scroll.offset().x;
        assert_eq!(offset, 30.0);
        let mut root = install_action_route(&mut app);
        assert!(constrain(&mut root, table, flow));
        let after = LayoutEngine
            .compute(&root, Viewport::new(1280.0, 800.0, scale))
            .expect("actual Gallery Table after horizontal scroll");
        let header_before = before.node(column).expect("initial column").rect;
        let cell_before = before.node(cell).expect("initial cell").rect;
        let header_after = after.node(column).expect("scrolled column").rect;
        let cell_after = after.node(cell).expect("scrolled cell").rect;
        println!(
            "GPV05-GALLERY-TABLE-SCROLL flow={flow:?} scale={scale} offset={offset} header_before={header_before:?} cell_before={cell_before:?} header_after={header_after:?} cell_after={cell_after:?}"
        );
        assert!((header_before.x - cell_before.x).abs() < 0.01);
        assert!(
            (header_after.x - cell_after.x).abs() < 0.01,
            "Table header/body must share horizontal movement in {flow:?}: header={header_after:?}, cell={cell_after:?}"
        );
        assert!((header_after.x - header_before.x + flow.horizontal(offset)).abs() < 0.01);
    }
}
