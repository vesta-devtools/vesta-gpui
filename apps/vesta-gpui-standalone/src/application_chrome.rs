//! Main-window application chrome uses the shared widget typography and actions.

use super::*;

pub(super) struct ApplicationMenuHeader {
    pub id: MenuId,
    pub element: u32,
    label: u32,
    caption: &'static str,
    accessible_name: &'static str,
    width: f32,
    compact_width: f32,
}

pub(super) const APPLICATION_MENUS: [ApplicationMenuHeader; 4] = [
    ApplicationMenuHeader {
        id: MenuId(0),
        element: APP_MENU,
        label: 24,
        caption: "Vesta",
        accessible_name: "App menu",
        width: 54.0,
        compact_width: 42.0,
    },
    ApplicationMenuHeader {
        id: MenuId(1),
        element: HEADER_FILE_MENU,
        label: 34,
        caption: "File",
        accessible_name: "File menu",
        width: 44.0,
        compact_width: 34.0,
    },
    ApplicationMenuHeader {
        id: MenuId(3),
        element: HEADER_VIEW_MENU,
        label: 35,
        caption: "View",
        accessible_name: "View menu",
        width: 48.0,
        compact_width: 34.0,
    },
    ApplicationMenuHeader {
        id: MenuId(4),
        element: HEADER_WINDOW_MENU,
        label: 36,
        caption: "Window",
        accessible_name: "Window menu",
        width: 70.0,
        compact_width: 48.0,
    },
];

fn compact_header(width: f32) -> bool {
    width < 800.0
}

pub(super) fn application_menu_anchor(menu: MenuId, window_width: f32) -> Point {
    let compact = compact_header(window_width);
    let roots = APPLICATION_MENUS
        .iter()
        .map(|header| {
            (
                header.id,
                if compact {
                    header.compact_width
                } else {
                    header.width
                },
            )
        })
        .collect::<Vec<_>>();
    vesta_gpui_widgets::menu_bar_anchor(&roots, menu, Point::new(4.0, CHROME_HEIGHT))
        .unwrap_or(Point::new(4.0, CHROME_HEIGHT))
}

struct TitlebarButtonSpec<'a> {
    id: u32,
    label_id: u32,
    caption: &'a str,
    accessible_name: &'a str,
    action: DemoAction,
    width: f32,
    active: bool,
    destructive: bool,
    window_control: bool,
}

pub(super) fn build_header(
    scope: ElementScope,
    input: &UiBuildState<'_>,
) -> (Element, Vec<(ElementId, DemoAction)>) {
    let mode = input.mode;
    let maximized = input.maximized;
    let hovered = input.hovered;
    let open_menu = input.demo.menu.path().first().copied();
    let locale = input.demo.locale;
    let theme = input.theme;
    let viewport_available = input.demo.viewport3d_available;
    let expanded = input.demo.menu_origin == MenuOrigin::ApplicationBar && input.menu_open;
    let compact = compact_header(input.viewport_size.width);
    let gallery_width = if compact { 36.0 } else { 112.0 };
    let viewport_width = if compact {
        36.0
    } else if viewport_available {
        126.0
    } else {
        180.0
    };
    let theme_width = 70.0;
    let locale_width = 76.0;
    let mut actions = Vec::new();
    let gallery = titlebar_button(
        scope,
        TitlebarButtonSpec {
            id: MODE_2D,
            label_id: 20,
            caption: if compact { "2D" } else { "2D Gallery" },
            accessible_name: "2D Gallery",
            action: DemoAction::SelectMode(AcceptanceMode::Gallery2d),
            width: gallery_width,
            active: mode == AcceptanceMode::Gallery2d,
            destructive: false,
            window_control: false,
        },
        hovered,
        theme,
    );
    let mut viewport = titlebar_button(
        scope,
        TitlebarButtonSpec {
            id: MODE_3D,
            label_id: 21,
            caption: if compact {
                "3D"
            } else if viewport_available {
                "3D Viewport"
            } else {
                "3D Viewport · 待实现"
            },
            accessible_name: "3D Viewport",
            action: DemoAction::SelectMode(AcceptanceMode::Viewport3d),
            width: viewport_width,
            active: mode == AcceptanceMode::Viewport3d,
            destructive: false,
            window_control: false,
        },
        hovered,
        theme,
    );
    if !viewport_available {
        viewport = viewport
            .conditioned(WidgetAvailability {
                visible: true,
                enabled: false,
            })
            .expect("the unsupported viewport remains visible");
        viewport.element.tooltip =
            Some("3D Viewport 尚未接入原生 Vulkan；当前后端仅支持 2D。".into());
        if let Some(semantic) = &mut viewport.element.semantic {
            semantic.description =
                Some("3D Viewport 尚未接入原生 Vulkan；当前后端仅支持 2D。".into());
        }
        if let Some(label) = viewport.element.children.first_mut() {
            label.style.visual.text_color = theme.palette.text_muted;
        }
    }
    let mut left = Element::container(scope.element(37), ElementKey::new("header.left")).styled(
        Style::default()
            .absolute(4.0, 0.0)
            .axis(Axis::Row)
            .gap(0.0)
            .size(Dimension::Auto, Dimension::Px(CHROME_HEIGHT)),
    );
    if expanded {
        let entries = APPLICATION_MENUS
            .iter()
            .map(|header| vesta_gpui_widgets::MenuBarEntry {
                menu: header.id,
                root: WidgetIdentity::new(
                    scope.element(header.element),
                    format!("header.menu.{}", header.id.0),
                ),
                label: WidgetIdentity::new(
                    scope.element(header.label),
                    format!("header.menu.{}.label", header.id.0),
                ),
                caption: if compact && header.id == MenuId(4) {
                    "Win"
                } else {
                    header.caption
                }
                .into(),
                accessible_name: header.accessible_name.into(),
                width: if compact {
                    header.compact_width
                } else {
                    header.width
                },
                action: DemoAction::OpenMenu(header.id),
            })
            .collect();
        for widget in vesta_gpui_widgets::menu_bar(
            entries,
            vesta_gpui_widgets::MenuBarView {
                expanded,
                current: open_menu,
                hovered,
                height: CHROME_HEIGHT,
            },
            theme,
        ) {
            left = left.child(widget_element(widget, &mut actions));
        }
    } else {
        left = left.child(widget_element(
            titlebar_button(
                scope,
                TitlebarButtonSpec {
                    id: APP_MENU,
                    label_id: 24,
                    caption: "☰",
                    accessible_name: "App menu",
                    action: DemoAction::ToggleApplicationMenu,
                    width: 38.0,
                    active: false,
                    destructive: false,
                    window_control: false,
                },
                hovered,
                theme,
            ),
            &mut actions,
        ));
        left = left.child(
            Element::text(scope.element(25), ElementKey::new("header.brand"), "Vesta").styled(
                Style::default()
                    .size(Dimension::Px(58.0), Dimension::Px(CHROME_HEIGHT))
                    .text(theme.typography.font_size, theme.palette.text)
                    .text_align(TextHAlign::Start, TextVAlign::Baseline)
                    .pointer_events(false),
            ),
        );
    }
    let theme_button = titlebar_button(
        scope,
        TitlebarButtonSpec {
            id: THEME_TOGGLE,
            label_id: 22,
            caption: match theme.mode {
                ThemeMode::Dark => "Light",
                ThemeMode::Light => "Dark",
            },
            accessible_name: "Toggle light and dark theme",
            action: DemoAction::ToggleTheme,
            width: theme_width,
            active: false,
            destructive: false,
            window_control: false,
        },
        hovered,
        theme,
    );
    let locale_button = titlebar_button(
        scope,
        TitlebarButtonSpec {
            id: LOCALE_TOGGLE,
            label_id: 30,
            caption: match locale {
                UiLocale::EnUs => "中文",
                UiLocale::ZhCn => "English",
            },
            accessible_name: match locale {
                UiLocale::EnUs => "Switch to Chinese",
                UiLocale::ZhCn => "切换为英文",
            },
            action: DemoAction::ToggleLocale,
            width: locale_width,
            active: false,
            destructive: false,
            window_control: false,
        },
        hovered,
        theme,
    );
    let modes_width = gallery_width + viewport_width;
    let modes = Element::container(scope.element(38), ElementKey::new("header.modes"))
        .styled(
            Style::default()
                .absolute((input.viewport_size.width - modes_width) * 0.5, 0.0)
                .size(Dimension::Px(modes_width), Dimension::Px(CHROME_HEIGHT))
                .axis(Axis::Row)
                .gap(0.0),
        )
        .child(widget_element(gallery, &mut actions))
        .child(widget_element(viewport, &mut actions));
    let right_width = if compact {
        0.0
    } else {
        theme_width + locale_width
    } + if cfg!(target_os = "windows") {
        3.0 * CHROME_WINDOW_CONTROL_WIDTH
    } else {
        0.0
    };
    let mut right = Element::container(scope.element(39), ElementKey::new("header.right")).styled(
        Style::default()
            .absolute(input.viewport_size.width - right_width, 0.0)
            .size(Dimension::Px(right_width), Dimension::Px(CHROME_HEIGHT))
            .axis(Axis::Row)
            .gap(0.0),
    );
    if !compact {
        right = right
            .child(widget_element(theme_button, &mut actions))
            .child(widget_element(locale_button, &mut actions));
    }
    let mut element = Element::container(scope.element(2), ElementKey::new("header"))
        .styled(
            Style::default()
                .absolute(0.0, 0.0)
                .size(Dimension::Percent(1.0), Dimension::Px(CHROME_HEIGHT))
                .axis(Axis::Row)
                .align(Align::Center)
                .gap(0.0)
                .background(theme.palette.chrome_title)
                .clipped()
                .z_index(100),
        )
        .child(left)
        .child(modes);
    if cfg!(target_os = "windows") {
        for spec in [
            TitlebarButtonSpec {
                id: WINDOW_MINIMIZE,
                label_id: 26,
                caption: "−",
                accessible_name: "Minimize window",
                action: DemoAction::WindowMinimize,
                width: CHROME_WINDOW_CONTROL_WIDTH,
                active: false,
                destructive: false,
                window_control: true,
            },
            TitlebarButtonSpec {
                id: WINDOW_MAXIMIZE,
                label_id: 27,
                caption: if maximized { "❐" } else { "□" },
                accessible_name: "Maximize or restore window",
                action: DemoAction::WindowMaximize,
                width: CHROME_WINDOW_CONTROL_WIDTH,
                active: false,
                destructive: false,
                window_control: true,
            },
            TitlebarButtonSpec {
                id: WINDOW_CLOSE,
                label_id: 28,
                caption: "×",
                accessible_name: "Close window",
                action: DemoAction::WindowClose,
                width: CHROME_WINDOW_CONTROL_WIDTH,
                active: false,
                destructive: true,
                window_control: true,
            },
        ] {
            right = right.child(widget_element(
                titlebar_button(scope, spec, hovered, theme),
                &mut actions,
            ));
        }
    }
    element = element.child(right).child(
        Element::container(scope.element(29), ElementKey::new("header.bottom.divider")).styled(
            Style::default()
                .absolute(0.0, CHROME_HEIGHT - 1.0)
                .size(Dimension::Percent(1.0), Dimension::Px(1.0))
                .background(theme.palette.chrome_divider)
                .pointer_events(false),
        ),
    );
    (element, actions)
}

fn titlebar_button(
    scope: ElementScope,
    spec: TitlebarButtonSpec<'_>,
    hovered: Option<ElementId>,
    theme: WidgetTheme,
) -> Widget<DemoAction> {
    let is_hovered = hovered == Some(scope.element(spec.id));
    let mut control = chrome_button(
        WidgetIdentity::new(
            scope.element(spec.id),
            format!("header.control.{}", spec.id),
        ),
        WidgetIdentity::new(
            scope.element(spec.label_id),
            format!("header.label.{}", spec.id),
        ),
        spec.caption,
        spec.accessible_name,
        spec.action,
        ChromeButtonStyle {
            role: if spec.window_control {
                ChromeButtonRole::Window
            } else if matches!(spec.id, MODE_2D | MODE_3D) {
                ChromeButtonRole::Tab
            } else {
                ChromeButtonRole::Command
            },
            width: spec.width,
            selected: spec.active,
            hovered: is_hovered,
            destructive: spec.destructive,
        },
        theme,
    );
    if spec.caption == "☰"
        && let Some(label) = control.element.children.first_mut()
    {
        label.style.top = Some(-1.0);
    }
    control
}
