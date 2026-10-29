//! Gallery composition for the standalone acceptance host.

mod collections;
pub(crate) mod documents;
mod image_fit;
mod overlays;
mod resource_icons;
mod text_samples;
mod tree;
mod workspace;

use super::*;
use collections::{
    COLLECTION_OBJECTS, CollectionObject, collection_object_content, sample_object_table,
    sample_tile_grid, virtual_data_controls, virtual_object_content, virtual_object_list,
    virtual_object_table,
};
use vesta_gpui::{ElementKind, FlexWrap, FontRole};
use vesta_gpui_widgets::{
    ButtonPresentation, ButtonTone, ProgressState, RadioGroupOption, button_with_presentation,
    progress_bar_state, radio_group,
};

fn gallery_area(
    scope: ElementScope,
    section: GallerySectionId,
    title: &str,
    content: Element,
    demo: &DemoState,
    actions: &mut Vec<(ElementId, DemoAction)>,
    theme: WidgetTheme,
) -> Element {
    let identity = |slot, part| {
        WidgetIdentity::new(
            section.element(scope, slot),
            format!("gallery.section.{}.{}", section as u32, part),
        )
    };
    let mut area = widget_element(
        expandable_area(
            ExpandableAreaIds {
                root: identity(0, "root"),
                header: identity(1, "header"),
                disclosure: WidgetIdentity::new(
                    section.disclosure(scope),
                    format!("gallery.section.{}.disclosure", section as u32),
                ),
                label: identity(2, "label"),
                body: identity(3, "body"),
            },
            title,
            !demo.collapsed_gallery_sections.contains(&section),
            Widget::from_element(content),
            DemoAction::ToggleGallerySection(section),
            theme,
        ),
        actions,
    );
    // Structural wrappers inherit the page viewport; leaf widgets retain their clips.
    area.style.clip = ClipMode::Inherit;
    if let Some(body) = area
        .children
        .iter_mut()
        .find(|child| child.id == section.element(scope, 3))
    {
        body.style.clip = ClipMode::Inherit;
    }
    area
}

pub(super) fn build_gallery(input: &UiBuildState<'_>) -> GalleryUi {
    let section_axis = Axis::Column;
    let scope = input.scopes.gallery;
    let theme = input.theme;
    let scrollbar = theme.scrollbar_style();
    let snapshot = input.snapshot.clone();
    let focused = input.focused;
    let search_snapshot = input.search_snapshot.clone();
    let area_snapshot = input.area_snapshot.clone();
    let demo = input.demo;
    let combo_open = demo
        .combo
        .opener()
        .is_some_and(|opener| opener.window == input.ui_window);
    let popover_open = demo
        .popover
        .opener()
        .is_some_and(|opener| opener.window == input.ui_window);
    let inline_editing = demo
        .inline_session
        .opener()
        .is_some_and(|opener| opener.window == input.ui_window);
    let hovered = input.hovered;
    let mut ids = Ids::new(scope, 1000);
    let mut actions = Vec::new();

    let controls = [
        ("Primary", ControlState::Idle),
        ("Hovered", ControlState::Hovered),
        ("Pressed", ControlState::Pressed),
        ("Disabled", ControlState::Disabled),
    ]
    .into_iter()
    .map(|(text, state)| {
        let root = ids.identity("button.root");
        let dynamic_state = if state == ControlState::Idle && hovered == Some(root.element) {
            ControlState::Hovered
        } else {
            state
        };
        widget_element(
            button_with_presentation(
                root,
                ids.identity("button.label"),
                text,
                DemoAction::StepNumber(1),
                dynamic_state,
                ButtonPresentation {
                    tone: if state == ControlState::Idle {
                        ButtonTone::Primary
                    } else {
                        ButtonTone::Normal
                    },
                    loading: false,
                },
                theme,
            ),
            &mut actions,
        )
    })
    .collect::<Vec<_>>();
    let mut controls = controls;
    for (caption, action, presentation) in [
        (
            "Subtract",
            DemoAction::StepNumber(-1),
            ButtonPresentation {
                tone: ButtonTone::Danger,
                loading: false,
            },
        ),
        (
            "Run work",
            DemoAction::RestartBusy,
            ButtonPresentation {
                tone: ButtonTone::Primary,
                loading: demo.busy,
            },
        ),
    ] {
        controls.push(widget_element(
            button_with_presentation(
                ids.identity("command-button.root"),
                ids.identity("command-button.label"),
                caption,
                action,
                ControlState::Idle,
                presentation,
                theme,
            ),
            &mut actions,
        ));
    }
    controls.push(widget_element(
        icon_button(
            ids.identity("icon-button.root"),
            ids.identity("icon-button.glyph"),
            IconButtonSpec {
                icon: "−",
                name: "Decrease sample value",
                shortcut: None,
                kind: CompactButtonKind::Icon,
                state: ControlState::Idle,
            },
            DemoAction::StepNumber(-1),
            theme,
        ),
        &mut actions,
    ));
    controls.push(widget_element(
        icon_button(
            ids.identity("toolbar-button.root"),
            ids.identity("toolbar-button.glyph"),
            IconButtonSpec {
                icon: "◐",
                name: "Switch theme",
                shortcut: Some("T"),
                kind: CompactButtonKind::Toolbar,
                state: ControlState::Idle,
            },
            DemoAction::ToggleTheme,
            theme,
        ),
        &mut actions,
    ));
    if !demo_images::faults_enabled() {
        controls.extend(resource_icons::samples(
            scope,
            resource_icons::Host::Controls,
            demo.compact_toggle,
            theme,
            &mut actions,
        ));
    }
    controls.push(widget_element(
        toggle_button(
            ids.identity("toggle-button.root"),
            ids.identity("toggle-button.label"),
            "Pinned",
            demo.compact_toggle,
            ControlState::Idle,
            DemoAction::ToggleCompactButton,
            theme,
        ),
        &mut actions,
    ));
    controls.push(toolbar_demo::sample(
        input,
        toolbar_demo::Host::Controls,
        &mut actions,
    ));
    let buttons = section(&mut ids, "Buttons & command controls", controls, theme);
    let picker = build_color_picker(input, ColorEditTarget::Swatch, &mut actions);
    let controls = vec![
        widget_element(
            toggle(
                ToggleIds {
                    root: ids.identity("toggle.on"),
                    track: ids.identity("toggle.on.track"),
                    knob: ids.identity("toggle.on.knob"),
                    label: ids.identity("toggle.on.label"),
                },
                "Feature",
                demo.toggle,
                DemoAction::ToggleSwitch,
                ControlState::Idle,
                theme,
            ),
            &mut actions,
        ),
        widget_element(
            checkbox(
                CheckboxIds {
                    root: ids.identity("checkbox.root"),
                    box_identity: ids.identity("checkbox.box"),
                    mark: ids.identity("checkbox.mark"),
                    label: ids.identity("checkbox.label"),
                },
                "Checkbox",
                demo.checkbox,
                DemoAction::CycleCheckbox,
                ControlState::Idle,
                theme,
            ),
            &mut actions,
        ),
        widget_element(
            radio_group(
                WidgetIdentity::new(scope.element(RADIO_GROUP_ROOT), "gallery.radio-group"),
                "Options",
                Some(&demo.radio),
                ["Option A", "Option B"]
                    .into_iter()
                    .enumerate()
                    .map(|(index, label)| {
                        let value = u8::try_from(index).expect("two gallery options");
                        let base = RADIO_GROUP_ROOT + 1 + u32::from(value) * 4;
                        let identity = |slot, part| {
                            WidgetIdentity::new(
                                scope.element(base + slot),
                                format!("gallery.radio.{value}.{part}"),
                            )
                        };
                        RadioGroupOption {
                            value,
                            ids: RadioIds {
                                root: identity(0, "root"),
                                ring: identity(1, "ring"),
                                dot: identity(2, "dot"),
                                label: identity(3, "label"),
                            },
                            label: label.into(),
                            action: DemoAction::SelectRadio(value),
                            state: ControlState::Idle,
                        }
                    })
                    .collect(),
                theme,
            )
            .expect("unique gallery radio values"),
            &mut actions,
        ),
    ];
    let toggles = section(&mut ids, "Toggle / Check / Radio", controls, theme);
    let compound = conditional_controls::widget(&mut ids, demo, theme);
    let conditional = section(
        &mut ids,
        "Compound condition: toggle disables; Option B hides",
        vec![widget_element(compound, &mut actions)],
        theme,
    );
    let mut field = text_field(
        WidgetIdentity::new(scope.element(TEXT_FIELD), "gallery.text-field"),
        snapshot,
        focused,
        theme,
    );
    field.style.width = Dimension::Px(360.0);
    field.style.max_width = Dimension::Percent(1.0);
    let mut area = text_area(
        WidgetIdentity::new(scope.element(TEXT_AREA), "gallery.text-area"),
        area_snapshot,
        input.area_focused,
        theme,
    );
    area.style.width = Dimension::Px(360.0);
    area.style.max_width = Dimension::Percent(1.0);
    let number = widget_element(
        editable_number_field(
            NumberFieldIds {
                root: ids.identity("number.root"),
                value_label: WidgetIdentity::new(
                    scope.element(NUMBER_FIELD),
                    "gallery.number.value",
                ),
                decrement: ids.identity("number.decrement"),
                decrement_label: ids.identity("number.decrement.label"),
                increment: ids.identity("number.increment"),
                increment_label: ids.identity("number.increment.label"),
            },
            input.number_snapshot.clone(),
            input.number_focused,
            f64::from(demo.number),
            f64::from(NUMBER_MIN),
            f64::from(NUMBER_MAX),
            1.0,
            DemoAction::StepNumber(-1),
            DemoAction::StepNumber(1),
            ControlState::Idle,
            theme,
        ),
        &mut actions,
    );
    let inline_snapshot = search_snapshot.clone();
    let mut search = search_field(
        SearchFieldIds {
            root: ids.identity("search.root"),
            icon: ids.identity("search.icon"),
            input: WidgetIdentity::new(scope.element(SEARCH_FIELD), "gallery.search.input"),
            placeholder: ids.identity("search.placeholder"),
            clear: ids.identity("search.clear"),
        },
        search_snapshot,
        input.search_query.is_empty(),
        input.search_focused,
        DemoAction::FocusSearch,
        DemoAction::ClearSearch,
        theme,
    );
    search.element.style.max_width = Dimension::Percent(1.0);
    let search = suggestion_demo::project(input, search);
    let mut selectable = vesta_gpui_widgets::selectable_label(
        WidgetIdentity::new(
            scope.element(SELECTABLE_LABEL_FIELD),
            "gallery.text.selectable",
        ),
        TextContent::Literal(SELECTABLE_LABEL_SOURCE),
        input.readonly_label,
        input.readonly_label_focused,
        theme,
    )
    .expect("matching read-only gallery source");
    selectable.style.width = Dimension::Px(360.0);
    selectable.style.max_width = Dimension::Percent(1.0);
    let mut controls = vec![field];
    if demo.inline_session.opener().is_none() && !demo.palette_session.is_open() {
        controls.push(widget_element(search, &mut actions));
    }
    let password_identity = |slot, part: &str| {
        WidgetIdentity::new(
            scope.element(PASSWORD_ROOT + slot),
            format!("gallery.password.{part}"),
        )
    };
    let password = widget_element(
        password_field(
            PasswordFieldIds {
                root: password_identity(0, "root"),
                display: password_identity(1, "display"),
                controls: password_identity(2, "controls"),
                reveal: password_identity(3, "reveal"),
                reveal_label: password_identity(4, "reveal.label"),
                clear: password_identity(5, "clear"),
                clear_label: password_identity(6, "clear.label"),
            },
            PasswordFieldView {
                mask: &input.password_mask,
                snapshot: input.password_snapshot.clone(),
                revealed: demo.password_revealed,
                focused: input.focused_element == Some(scope.element(PASSWORD_ROOT)),
            },
            PasswordFieldActions {
                focus: DemoAction::FocusPassword,
                reveal: DemoAction::TogglePasswordReveal,
                clear: DemoAction::ClearPassword,
            },
            theme,
        ),
        &mut actions,
    );
    controls.extend([number, password]);
    let mut palette_button = button(
        ids.identity("palette.open"),
        ids.identity("palette.open.label"),
        "Commands",
        DemoAction::OpenCommandPalette,
        ControlState::Idle,
        theme,
    );
    palette_button.element = palette_button
        .element
        .tooltip("Commands (Ctrl+Shift+P)")
        .semantic(Semantic::new(
            SemanticRole::Button,
            "Commands (Ctrl+Shift+P)",
        ));
    controls.push(widget_element(palette_button, &mut actions));
    controls.push(selectable);
    let inputs = section_with_axis(
        &mut ids,
        "Text / Number / Search input",
        controls,
        theme,
        section_axis,
    );
    let multiline = text_samples::text_samples(input, &mut ids, &mut actions, area);
    let combo = {
        let mut combo = combo_box(
            ComboBoxIds {
                root: WidgetIdentity::new(scope.element(COMBO_TRIGGER), "combo.root"),
                label: ids.identity("combo.label"),
                arrow: ids.identity("combo.arrow"),
            },
            "Projection",
            COMBO_VALUES[usize::from(demo.combo_selected).min(COMBO_VALUES.len() - 1)],
            combo_open,
            DemoAction::ToggleCombo,
            ControlState::Idle,
            theme,
        );
        combo.element.style.max_width = Dimension::Percent(1.0);
        widget_element(combo, &mut actions)
    };
    let menu_button = widget_element(
        button(
            ids.identity("menu.root"),
            ids.identity("menu.label"),
            "File menu",
            DemoAction::ToggleMenu,
            if input.menu_open {
                ControlState::Pressed
            } else {
                ControlState::Idle
            },
            theme,
        ),
        &mut actions,
    );
    let popover_button = popover_trigger(
        WidgetIdentity::new(scope.element(POPOVER_TRIGGER), "popover.trigger"),
        ids.identity("popover.trigger.label"),
        "Popover",
        popover_open,
        DemoAction::TogglePopover,
        if popover_open {
            ControlState::Pressed
        } else {
            ControlState::Idle
        },
        theme,
    );
    let key_caption = demo.key_chord.map(key_chord_caption);
    let controls = vec![
        combo,
        menu_button,
        widget_element(popover_button, &mut actions),
        widget_element(
            key_chord_selector(
                WidgetIdentity::new(scope.element(KEY_CHORD_TRIGGER), "gallery.key-chord.root"),
                ids.identity("key-chord.label"),
                KeyChordSelectorView {
                    label: "Sample shortcut",
                    value: key_caption.as_deref(),
                    capturing: demo
                        .key_capture
                        .target()
                        .is_some_and(|opener| opener.window == input.ui_window),
                    conflict: demo.key_capture.conflict(),
                    enabled: true,
                },
                DemoAction::BeginKeyCapture,
                theme,
            ),
            &mut actions,
        ),
        control(
            &mut ids,
            "Root  ›  Assets  ›  Mesh",
            theme.palette.surface,
            theme,
        ),
        picker,
    ];
    let selection = section(&mut ids, "Selection controls", controls, theme);
    let slider_root = ids.identity("slider.root");
    let slider_track = ids.identity("slider.track");
    let slider_id = slider_track.element;
    let busy_root;
    let indeterminate_root;
    let controls = vec![
        widget_element(
            slider_demo::row(
                SliderIds {
                    root: slider_root,
                    track: slider_track,
                    fill: ids.identity("slider.fill"),
                    thumb: ids.identity("slider.thumb"),
                    value_label: ids.identity("slider.value"),
                },
                demo,
                scope,
                theme,
            ),
            &mut actions,
        ),
        progress_bar_state(
            ids.identity("progress.root"),
            ids.identity("progress.fill"),
            ids.identity("progress.label"),
            ProgressState::Determinate(demo.slider),
            theme,
        )
        .expect("finite gallery progress"),
        {
            let root = ids.identity("progress.busy.root");
            indeterminate_root = root.element;
            progress_bar_state(
                root,
                ids.identity("progress.busy.fill"),
                ids.identity("progress.busy.label"),
                if demo.busy {
                    ProgressState::Indeterminate {
                        phase: f32::from(demo.busy_phase) / 4.0,
                    }
                } else {
                    ProgressState::Determinate(1.0)
                },
                theme,
            )
            .expect("finite host-driven phase")
        },
        {
            let root = ids.identity("busy.root");
            busy_root = root.element;
            widget_element(
                busy_indicator(
                    BusyIndicatorIds {
                        root,
                        spinner: ids.identity("busy.spinner"),
                        label: ids.identity("busy.label"),
                        cancel: ids.identity("busy.cancel"),
                        cancel_label: ids.identity("busy.cancel.label"),
                    },
                    "Loading",
                    demo.busy,
                    demo.busy_phase,
                    demo.busy,
                    DemoAction::CancelBusy,
                    theme,
                ),
                &mut actions,
            )
        },
        widget_element(
            button(
                ids.identity("busy.restart"),
                ids.identity("busy.restart.label"),
                "Restart",
                DemoAction::RestartBusy,
                if demo.busy {
                    ControlState::Disabled
                } else {
                    ControlState::Idle
                },
                theme,
            ),
            &mut actions,
        ),
        widget_element(
            button(
                WidgetIdentity::new(scope.element(TOAST_SHOW), "gallery.toast.show"),
                ids.identity("toast.show.label"),
                "Show notice",
                DemoAction::ShowToast,
                ControlState::Idle,
                theme,
            ),
            &mut actions,
        ),
        widget_element(
            status_badge(
                ids.identity("badge.info.root"),
                ids.identity("badge.info.label"),
                "Ready",
                StatusTone::Information,
                theme,
            ),
            &mut actions,
        ),
        widget_element(
            status_badge(
                ids.identity("badge.success.root"),
                ids.identity("badge.success.label"),
                "Saved",
                StatusTone::Success,
                theme,
            ),
            &mut actions,
        ),
        widget_element(
            status_badge(
                ids.identity("badge.warning.root"),
                ids.identity("badge.warning.label"),
                "Review",
                StatusTone::Warning,
                theme,
            ),
            &mut actions,
        ),
        widget_element(
            status_badge(
                ids.identity("badge.error.root"),
                ids.identity("badge.error.label"),
                "Failed",
                StatusTone::Error,
                theme,
            ),
            &mut actions,
        ),
    ];
    let feedback = section(&mut ids, "Range & feedback", controls, theme);
    let collection_pane_width = container_panes(&demo.workspace, input.workspace_container)
        .into_iter()
        .filter_map(|pane| demo.workspace.pane(pane))
        .filter(|pane| {
            pane.active_item()
                .is_some_and(|item| item.panel_type.as_str() == "gallery.collections")
        })
        .filter_map(|pane| demo.pane_widths.get(&pane.id).copied())
        .fold(f32::INFINITY, f32::min);
    let collection_pane_width = if collection_pane_width.is_finite() {
        collection_pane_width
    } else {
        (input.viewport_size.width - 100.0) / 2.0
    };
    let collection_content_width = (collection_pane_width - 16.0).max(40.0);
    let [breadcrumb, breadcrumb_next] =
        breadcrumb_demo::samples(input, breadcrumb_demo::Host::Collections, &mut actions);
    let mut tabs = Element::container(ids.id(), ids.key("tabs")).styled(
        Style::default()
            .size(Dimension::Percent(1.0), Dimension::Auto)
            .axis(Axis::Row)
            .gap(4.0),
    );
    for (index, name) in ["General", "Render", "Input"].into_iter().enumerate() {
        let mut item = tab(
            ids.identity("tab.root"),
            ids.identity("tab.label"),
            name,
            demo.selected_tab == index as u8,
            DemoAction::SelectTab(index as u8),
            ControlState::Idle,
            theme,
        );
        item.element.style.width =
            Dimension::Px(((collection_content_width.min(237.0) - 8.0) / 3.0).max(20.0));
        item.element.style.min_width = Dimension::Px(0.0);
        tabs = tabs.child(widget_element(item, &mut actions));
    }
    let scene = CollectionObject {
        id: 0_u8,
        name: "Scene",
        thumbnail: "demo://checkerboard",
    };
    let tree_content_root = ids.identity("tree.content");
    let tree_image = ids.identity("tree.image");
    let tree_label = ids.identity("tree.label");
    let tree_command = ids.identity("tree.commands");
    let tree_command_label = ids.identity("tree.commands.label");
    let mut tree_widget = tree_view_row_from(
        TreeRowContentIds {
            root: ids.identity("tree.root"),
            disclosure: ids.identity("tree.disclosure"),
        },
        &scene,
        TreeViewRowState {
            depth: 0,
            has_children: true,
            expanded: demo.tree_expanded,
            selected: false,
            control: ControlState::Idle,
        },
        TreeViewRowActions {
            select: DemoAction::ToggleTree,
            toggle: DemoAction::ToggleTree,
        },
        theme,
        |item| {
            collection_object_content(
                item,
                |slot, _| match slot {
                    1 => tree_content_root.clone(),
                    2 => tree_image.clone(),
                    _ => tree_label.clone(),
                },
                Some(button(
                    tree_command,
                    tree_command_label,
                    "Commands",
                    DemoAction::OpenCommandPalette,
                    ControlState::Idle,
                    theme,
                )),
                theme,
            )
        },
    );
    tree_widget.element.style.width = Dimension::Px(320.0);
    tree_widget.element.style.max_width = Dimension::Percent(1.0);
    let tree = widget_element(tree_widget, &mut actions);
    let mut list = Element::container(ids.id(), ids.key("list"))
        .styled(Style::default().axis(Axis::Column).gap(3.0));
    let search_query = input.search_query.trim().to_lowercase();
    let mut visible_items = 0;
    for object in &COLLECTION_OBJECTS {
        if !object.name.to_lowercase().contains(&search_query) {
            continue;
        }
        visible_items += 1;
        let identity = |slot, part: &str| {
            WidgetIdentity::new(
                scope.element(BASIC_LIST_ROOT + u32::from(object.id) * 8 + slot),
                format!("collection.{}.{part}", object.id),
            )
        };
        let mut row = list_item_from(
            WidgetIdentity::new(
                scope.element(BASIC_LIST_ROOT + u32::from(object.id) * 8),
                format!("collection.{}.row", object.id),
            ),
            object,
            demo.selected_list == object.id,
            DemoAction::SelectList(object.id),
            ControlState::Idle,
            theme,
            |item| {
                collection_object_content(
                    item,
                    identity,
                    Some(button(
                        identity(4, "commands"),
                        identity(5, "commands.label"),
                        "Commands",
                        DemoAction::OpenCommandPalette,
                        ControlState::Idle,
                        theme,
                    )),
                    theme,
                )
            },
        );
        row.element.style.width = Dimension::Px(320.0);
        row.element.style.max_width = Dimension::Percent(1.0);
        list = list.child(widget_element(row, &mut actions));
    }
    if visible_items == 0 {
        list = list.child(label(
            &mut ids,
            "No matching objects",
            theme.palette.text_muted,
            theme,
        ));
    }
    let table_width = collection_content_width.max(132.0);
    let table = sample_object_table(input, table_width, &search_query, &mut actions);
    let virtual_table = widget_element(
        virtual_object_table(input, table_width, scrollbar),
        &mut actions,
    );
    let (tiles, tile_columns) = sample_tile_grid(input, collection_content_width);
    let tiles = widget_element(tiles, &mut actions);
    let controls = vec![
        breadcrumb,
        breadcrumb_next,
        tabs,
        tree,
        list,
        table,
        virtual_table,
        tiles,
    ];
    let navigation = section_with_axis(
        &mut ids,
        "Navigation & collections",
        controls,
        theme,
        section_axis,
    );
    let mut nested_content = Element::container(ids.id(), ids.key("nested.content")).styled(
        Style::default()
            .size(Dimension::Px(220.0), Dimension::Px(192.0))
            .axis(Axis::Column)
            .gap(2.0),
    );
    for (index, name) in [
        "Layer 1", "Layer 2", "Layer 3", "Layer 4", "Layer 5", "Layer 6",
    ]
    .into_iter()
    .enumerate()
    {
        nested_content = nested_content.child(widget_element(
            list_item(
                ids.identity("nested.item"),
                ids.identity("nested.item.label"),
                name,
                demo.selected_list == index as u8,
                DemoAction::SelectList(index as u8),
                ControlState::Idle,
                theme,
            ),
            &mut actions,
        ));
    }
    let nested_scroll = Element::scroll_box(
        scope.element(NESTED_SCROLL),
        ElementKey::new("gallery.nested.scroll"),
        demo.nested_scroll.offset(),
    )
    .styled(
        Style::default()
            .size(Dimension::Px(220.0), Dimension::Px(70.0))
            .max_width(Dimension::Percent(1.0))
            .scrollbar(scrollbar)
            .background(theme.palette.field)
            .border(1.0, theme.palette.border),
    )
    .child(nested_content);
    let mut nested = section(
        &mut ids,
        "Nested scroll (wheel at edge passes to page)",
        vec![nested_scroll],
        theme,
    );
    nested.style.height = Dimension::Px(130.0);
    let mut horizontal_content = Element::container(ids.id(), ids.key("horizontal.content"))
        .styled(
            Style::default()
                .size(Dimension::Px(940.0), Dimension::Px(40.0))
                .axis(Axis::Row)
                .gap(8.0),
        );
    for (index, name) in ["North", "South", "East", "West", "Above", "Below"]
        .into_iter()
        .enumerate()
    {
        let label = match (demo.horizontal_flow, index) {
            (FlowDirection::RightToLeft, 0) => "مرحبا / North",
            (FlowDirection::RightToLeft, 1) => "南 / South",
            (FlowDirection::RightToLeft, 2) => "East · English",
            _ => name,
        };
        let mut item = list_item(
            ids.identity("horizontal.item"),
            ids.identity("horizontal.item.label"),
            label,
            demo.selected_list == index as u8,
            DemoAction::SelectList(index as u8),
            ControlState::Idle,
            theme,
        );
        if index == 2 {
            item.element.style.flow_direction = Some(FlowDirection::LeftToRight);
        }
        horizontal_content = horizontal_content.child(widget_element(item, &mut actions));
    }
    let horizontal_scroll = Element::scroll_box(
        scope.element(HORIZONTAL_SCROLL),
        ElementKey::new("gallery.horizontal.scroll"),
        demo.horizontal_scroll.offset(),
    )
    .styled(
        Style::default()
            .size(Dimension::Px(320.0), Dimension::Px(60.0))
            .max_width(Dimension::Percent(1.0))
            .scrollbar(scrollbar)
            .background(theme.palette.field)
            .border(1.0, theme.palette.border),
    )
    .child(horizontal_content);
    let flow_toggle = widget_element(
        button(
            WidgetIdentity::new(scope.element(25_100_000), "horizontal.flow.toggle"),
            WidgetIdentity::new(scope.element(25_100_001), "horizontal.flow.label"),
            match demo.horizontal_flow {
                FlowDirection::LeftToRight => "LTR · switch to RTL",
                FlowDirection::RightToLeft => "RTL · switch to LTR",
            },
            DemoAction::ToggleHorizontalFlow,
            ControlState::Idle,
            theme,
        ),
        &mut actions,
    );
    let mut horizontal_scroll = horizontal_scroll;
    horizontal_scroll.style.flow_direction = Some(demo.horizontal_flow);
    let mut horizontal = section(
        &mut ids,
        "Horizontal scroll (trackpad X / Shift + wheel)",
        vec![flow_toggle, horizontal_scroll],
        theme,
    );
    horizontal.style.height = Dimension::Px(154.0);
    let mut grid = Element::container(ids.id(), ids.key("layout.grid")).styled(
        Style::default()
            .size(Dimension::Px(420.0), Dimension::Px(60.0))
            .max_width(Dimension::Percent(1.0))
            .grid(
                vec![GridTrack::Fr(1.0), GridTrack::Fr(2.0), GridTrack::Fr(1.0)],
                vec![GridTrack::Px(27.0), GridTrack::Px(27.0)],
            )
            .gap(6.0),
    );
    for (name, column, row, span) in [
        ("1fr", 1, 1, 1),
        ("2fr", 2, 1, 1),
        ("1fr", 3, 1, 1),
        ("Column span 2", 1, 2, 2),
    ] {
        let mut cell = control(&mut ids, name, theme.palette.surface, theme);
        cell.style.grid_column = GridCell::new(Some(column), span);
        cell.style.grid_row = GridCell::new(Some(row), 1);
        grid = grid.child(cell);
    }
    let mut grid_section = section(&mut ids, "Grid tracks and cell spans", vec![grid], theme);
    grid_section.style.height = Dimension::Px(130.0);
    let mut accordion = Element::container(ids.id(), ids.key("layout.accordion")).styled(
        Style::default()
            .size(Dimension::Px(420.0), Dimension::Auto)
            .max_width(Dimension::Percent(1.0))
            .gap(4.0),
    );
    for (index, heading, command, action) in [
        (0, "Appearance", "Switch Theme", DemoAction::ToggleTheme),
        (1, "Values", "Increment Value", DemoAction::StepNumber(1)),
    ] {
        let content = button(
            ids.identity("accordion.content.command"),
            ids.identity("accordion.content.label"),
            command,
            action,
            ControlState::Idle,
            theme,
        );
        let panel = collapsible(
            CollapsibleIds {
                root: ids.identity("accordion.panel"),
                header: ids.identity("accordion.header"),
                disclosure: ids.identity("accordion.header.disclosure"),
                label: ids.identity("accordion.header.label"),
            },
            heading,
            demo.accordion_open == Some(index),
            content,
            DemoAction::ToggleAccordion(index),
            theme,
        );
        accordion = accordion.child(widget_element(panel, &mut actions));
    }
    let mut accordion_section =
        section(&mut ids, "Collapsible / accordion", vec![accordion], theme);
    accordion_section.style.height = Dimension::Px(170.0);
    let virtual_widget = virtual_object_list(
        scope,
        ids.identity("gallery.virtual.content"),
        demo,
        input.virtual_focused,
        theme,
        scrollbar,
    );
    let virtual_controls = virtual_data_controls(scope, demo, theme);
    let mut virtual_section = section(
        &mut ids,
        "Virtual list · 10,000 objects",
        vec![
            widget_element(virtual_controls, &mut actions),
            widget_element(virtual_widget, &mut actions),
        ],
        theme,
    );
    virtual_section.style.height = Dimension::Px(330.0);
    let tree_section = tree::tree_section(
        scope,
        &mut ids,
        demo,
        input.tree_focused,
        theme,
        scrollbar,
        &mut actions,
    );
    let composed_property = property_row_widget(
        PropertyRowIds {
            root: ids.identity("composed_property.root"),
            label: ids.identity("composed_property.label"),
            validation: ids.identity("composed_property.validation"),
        },
        PropertyRowState {
            label: "Iterations".into(),
            availability: WidgetAvailability {
                visible: true,
                enabled: demo.toggle,
            },
            validation: (demo.number > 128).then(|| PropertyValidation {
                severity: ValidationSeverity::Warning,
                message: "Above suggested range".into(),
            }),
        },
        button(
            ids.identity("composed_property.value"),
            ids.identity("composed_property.value_label"),
            format!("Increase {}", demo.number),
            DemoAction::StepNumber(1),
            ControlState::Idle,
            theme,
        ),
        theme,
    )
    .expect("visible demo property");
    let vector_ids = VectorFieldIds {
        root: ids.identity("vector.root"),
        axis_labels: std::array::from_fn(|_| ids.identity("vector.axis")),
        components: std::array::from_fn(|axis| NumberFieldIds {
            root: ids.identity("vector.component"),
            value_label: {
                let _ = ids.identity("vector.value.legacy_slot");
                WidgetIdentity::new(
                    scope.element(VECTOR_VALUE_BASE + axis as u32),
                    format!("gallery.vector.value.{axis}"),
                )
            },
            decrement: ids.identity("vector.decrement"),
            decrement_label: ids.identity("vector.decrement.label"),
            increment: ids.identity("vector.increment"),
            increment_label: ids.identity("vector.increment.label"),
        }),
    };
    let vector = widget_element(
        editable_vector_field(
            vector_ids,
            VectorFieldSpec {
                label: "Position",
                values: demo.vector.map(|value| {
                    value.map_or(VectorComponentValue::Mixed, VectorComponentValue::Value)
                }),
                unit: "m",
                precision: 3,
                state: ControlState::Idle,
            },
            VectorFieldEdit {
                components: std::array::from_fn(|axis| VectorComponentEdit {
                    snapshot: input.vector_snapshots[axis].clone(),
                    focused: input.vector_focused[axis],
                }),
                min: f64::from(VECTOR_MIN),
                max: f64::from(VECTOR_MAX),
                step: 1.0,
            },
            [
                [DemoAction::StepVector(0, -1), DemoAction::StepVector(0, 1)],
                [DemoAction::StepVector(1, -1), DemoAction::StepVector(1, 1)],
                [DemoAction::StepVector(2, -1), DemoAction::StepVector(2, 1)],
            ],
            theme,
        ),
        &mut actions,
    );
    let (ramp, ramp_controls, ramp_history, ramp_track) = build_ramp(input, &mut ids, &mut actions);
    let inline_identity = |slot, part: &str| {
        WidgetIdentity::new(
            scope.element(INLINE_ROOT + slot),
            format!("gallery.inline.{part}"),
        )
    };
    let inline_editor = inline_editing.then(|| {
        text_field(
            WidgetIdentity::new(scope.element(SEARCH_FIELD), "gallery.search.input"),
            inline_snapshot,
            input.search_focused,
            theme,
        )
    });
    let inline = widget_element(
        inline_editable_label(
            InlineEditIds {
                root: inline_identity(0, "root"),
                display: inline_identity(1, "display"),
                display_label: inline_identity(2, "display.label"),
                controls: inline_identity(3, "controls"),
                commit: inline_identity(4, "commit"),
                commit_label: inline_identity(5, "commit.label"),
                cancel: inline_identity(6, "cancel"),
                cancel_label: inline_identity(7, "cancel.label"),
            },
            &demo.inline_label,
            inline_editor,
            DemoAction::BeginInlineEdit,
            DemoAction::CommitInlineEdit,
            DemoAction::CancelInlineEdit,
            theme,
        ),
        &mut actions,
    );
    let property_section = section(
        &mut ids,
        "Composed property row",
        vec![
            widget_element(composed_property, &mut actions),
            vector,
            inline,
            ramp,
            ramp_controls,
            ramp_history,
        ],
        theme,
    );
    let property = property_row(
        ids.identity("property.root"),
        ids.identity("property.name"),
        ids.identity("property.value"),
        "Roughness",
        format!("{:.2}", demo.slider),
        theme,
    );
    let tooltip_root = ids.identity("tooltip.root");
    let tooltip_id = tooltip_root.element;
    let mut tooltip_button = widget_element(
        button(
            tooltip_root,
            ids.identity("tooltip.label"),
            "Hover or focus for tooltip",
            DemoAction::Noop,
            if hovered == Some(tooltip_id) {
                ControlState::Hovered
            } else {
                ControlState::Idle
            },
            theme,
        ),
        &mut actions,
    );
    tooltip_button = tooltip_button.tooltip(
        "Tooltip is framework metadata, not product state. 中文提示保留完整原文；\
         narrow windows wrap this explanation without changing its owner. \
         مرحبا بالعالم — resize, switch windows, or hide the source to clear it. \
         完整尾句 / tooltip end.",
    );
    let mut controls = vec![
        property,
        control(
            &mut ids,
            "Panel A     │     Panel B",
            theme.palette.surface,
            theme,
        ),
        tooltip_button,
        control(&mut ids, "Popover", theme.palette.overlay, theme),
    ];
    controls.push(widget_element(
        button(
            ids.identity("dialog.open"),
            ids.identity("dialog.open.label"),
            "Open dialog",
            DemoAction::OpenDemoDialog,
            ControlState::Idle,
            theme,
        ),
        &mut actions,
    ));
    let mut drawer_tabs = Widget::from_element(
        Element::container(ids.id(), ids.key("drawer.tabs"))
            .styled(Style::default().axis(Axis::Row).gap(3.0)),
    );
    for (index, title) in ["Actions", "Info"].into_iter().enumerate() {
        drawer_tabs = drawer_tabs.child(tab(
            ids.identity("drawer.tab"),
            ids.identity("drawer.tab.label"),
            title,
            demo.drawer_tab == index as u8,
            DemoAction::SelectDrawerTab(index as u8),
            ControlState::Idle,
            theme,
        ));
    }
    let drawer_page = if demo.drawer_tab == 0 {
        button(
            ids.identity("drawer.inner"),
            ids.identity("drawer.inner.label"),
            "Switch theme",
            DemoAction::ToggleTheme,
            ControlState::Idle,
            theme,
        )
    } else {
        Widget::from_element(
            Element::text(ids.id(), ids.key("drawer.info"), "Host-owned drawer page").styled(
                Style::default()
                    .text(theme.typography.font_size, theme.palette.text_muted)
                    .pointer_events(false),
            ),
        )
    };
    let drawer_body = Widget::from_element(
        Element::container(ids.id(), ids.key("drawer.tab-container"))
            .styled(Style::default().axis(Axis::Column).gap(6.0)),
    )
    .child(drawer_tabs)
    .child(drawer_page);
    controls.push(widget_element(
        sidebar_drawer(
            SidebarDrawerIds {
                root: ids.identity("drawer.root"),
                toggle: WidgetIdentity::new(scope.element(DRAWER_TOGGLE), "gallery.drawer.toggle"),
                toggle_label: ids.identity("drawer.toggle.label"),
                body: WidgetIdentity::new(scope.element(DRAWER_BODY), "gallery.drawer.body"),
                resize: WidgetIdentity::new(scope.element(DRAWER_RESIZE), "gallery.drawer.resize"),
                main: ids.identity("drawer.main"),
            },
            SidebarDrawerState {
                title: "Sample sidebar",
                open: demo.drawer_open,
                width: demo.drawer_width,
                min_width: 100.0,
                max_width: 320.0,
                keyboard_step: 8.0,
            },
            drawer_body,
            Widget::from_element(
                Element::text(ids.id(), ids.key("drawer.main.label"), "Main content").styled(
                    Style::default()
                        .text(theme.typography.font_size, theme.palette.text_muted)
                        .pointer_events(false),
                ),
            ),
            DemoAction::ToggleDrawer,
            theme,
        ),
        &mut actions,
    ));
    let image_id = ids.id();
    let image_key = ids.key("gallery.image");
    controls.push(
        Element::image(image_id, image_key, "demo://checkerboard").styled(
            Style::default()
                .size(Dimension::Px(120.0), Dimension::Auto)
                .max_width(Dimension::Percent(1.0))
                .aspect_ratio(120.0 / 54.0)
                .radius(theme.metrics.radius),
        ),
    );
    let mut baseline_row = Element::container(
        scope.element(25_200_000),
        ElementKey::new("gallery.layout.baseline"),
    )
    .styled(
        Style::default()
            .axis(Axis::Row)
            .align(Align::Baseline)
            .flex_wrap(FlexWrap::Wrap)
            .gap(10.0)
            .max_width(Dimension::Percent(1.0)),
    );
    for (index, (text, size, role)) in [
        ("Vesta · 地形", 24.0, FontRole::Ui),
        ("Hg 0123", 14.0, FontRole::Mono),
        ("مرحبا / Baseline", 18.0, FontRole::Ui),
    ]
    .into_iter()
    .enumerate()
    {
        baseline_row = baseline_row.child(
            Element::text(
                scope.element(25_200_001 + index as u32),
                ElementKey::new(format!("gallery.layout.baseline.text.{index}")),
                text,
            )
            .styled(
                Style::default()
                    .text(size, theme.palette.text)
                    .font_role(role)
                    .text_wrap(true)
                    .max_width(Dimension::Percent(1.0))
                    .text_align(TextHAlign::Start, TextVAlign::Start)
                    .pointer_events(false),
            ),
        );
    }
    controls.push(baseline_row);
    controls.extend(breadcrumb_demo::samples(
        input,
        breadcrumb_demo::Host::Layout,
        &mut actions,
    ));
    controls.push(toolbar_demo::sample(
        input,
        toolbar_demo::Host::Layout,
        &mut actions,
    ));
    if !demo_images::faults_enabled() {
        // The existing fault sequence isolates one failing resource source.
        controls.push(image_fit::samples(scope, theme));
        controls.extend(resource_icons::samples(
            scope,
            resource_icons::Host::Layout,
            demo.compact_toggle,
            theme,
            &mut actions,
        ));
    }
    let overlays = section(&mut ids, "Layout & overlays", controls, theme);
    let pages = HashMap::from([
        (
            "gallery.controls",
            vec![
                gallery_area(
                    scope,
                    GallerySectionId::Buttons,
                    "Buttons & command controls",
                    buttons,
                    demo,
                    &mut actions,
                    theme,
                ),
                gallery_area(
                    scope,
                    GallerySectionId::Toggles,
                    "Toggle / Check / Radio",
                    toggles,
                    demo,
                    &mut actions,
                    theme,
                ),
                gallery_area(
                    scope,
                    GallerySectionId::Conditional,
                    "Compound condition: toggle disables; Option B hides",
                    conditional,
                    demo,
                    &mut actions,
                    theme,
                ),
                gallery_area(
                    scope,
                    GallerySectionId::Feedback,
                    "Range & feedback",
                    feedback,
                    demo,
                    &mut actions,
                    theme,
                ),
            ],
        ),
        (
            "gallery.inputs",
            vec![
                gallery_area(
                    scope,
                    GallerySectionId::Inputs,
                    "Text / Number / Search input",
                    inputs,
                    demo,
                    &mut actions,
                    theme,
                ),
                gallery_area(
                    scope,
                    GallerySectionId::Multiline,
                    "Multiline text / wrap / IME",
                    multiline,
                    demo,
                    &mut actions,
                    theme,
                ),
                gallery_area(
                    scope,
                    GallerySectionId::Selection,
                    "Selection controls",
                    selection,
                    demo,
                    &mut actions,
                    theme,
                ),
                gallery_area(
                    scope,
                    GallerySectionId::Property,
                    "Composed property row",
                    property_section,
                    demo,
                    &mut actions,
                    theme,
                ),
            ],
        ),
        (
            "gallery.collections",
            vec![
                gallery_area(
                    scope,
                    GallerySectionId::Navigation,
                    "Navigation & collections",
                    navigation,
                    demo,
                    &mut actions,
                    theme,
                ),
                gallery_area(
                    scope,
                    GallerySectionId::Nested,
                    "Nested scroll (wheel at edge passes to page)",
                    nested,
                    demo,
                    &mut actions,
                    theme,
                ),
                gallery_area(
                    scope,
                    GallerySectionId::Horizontal,
                    "Horizontal scroll (trackpad X / Shift + wheel)",
                    horizontal,
                    demo,
                    &mut actions,
                    theme,
                ),
                gallery_area(
                    scope,
                    GallerySectionId::VirtualList,
                    "Virtual list · 10,000 objects",
                    virtual_section,
                    demo,
                    &mut actions,
                    theme,
                ),
                gallery_area(
                    scope,
                    GallerySectionId::VirtualTree,
                    "Virtual tree · 1,000 objects",
                    tree_section,
                    demo,
                    &mut actions,
                    theme,
                ),
            ],
        ),
        (
            "gallery.layout",
            vec![
                gallery_area(
                    scope,
                    GallerySectionId::Grid,
                    "Grid tracks and cell spans",
                    grid_section,
                    demo,
                    &mut actions,
                    theme,
                ),
                gallery_area(
                    scope,
                    GallerySectionId::Accordion,
                    "Collapsible / accordion",
                    accordion_section,
                    demo,
                    &mut actions,
                    theme,
                ),
                gallery_area(
                    scope,
                    GallerySectionId::Overlays,
                    "Layout & overlays",
                    overlays,
                    demo,
                    &mut actions,
                    theme,
                ),
            ],
        ),
    ]);
    let mut pages = pages;
    native_capture::project_selected_feedback(input, &mut pages, &mut actions);
    input
        .documents
        .register_page(&mut pages, input.scopes, theme, &mut actions);
    let (scroll, workspace_regions) =
        workspace::build_workspace(input, pages, &mut ids, &mut actions);

    let root = Element::container(scope.element(4), ElementKey::new("gallery.root"))
        .styled(
            Style::default()
                .size(Dimension::Percent(1.0), Dimension::Percent(1.0))
                .pointer_events(false),
        )
        .child(scroll);
    let mut root = overlays::compose(
        input,
        root,
        &mut ids,
        &mut actions,
        combo_open,
        popover_open,
    );
    theme.apply_scrollbars(&mut root);
    GalleryUi {
        element: root,
        actions,
        busy_roots: [busy_root, indeterminate_root],
        slider: slider_id,
        ramp_track,
        workspace_regions,
        tile_columns,
    }
}
