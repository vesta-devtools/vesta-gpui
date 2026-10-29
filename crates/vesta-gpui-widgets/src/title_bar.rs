//! Reusable application title bar projection. The host owns OS window actions.

use crate::{
    CHROME_TITLE_HEIGHT, CHROME_WINDOW_CONTROL_WIDTH, ChromeButtonRole, ChromeButtonStyle, Widget,
    WidgetIdentity, WidgetTheme, chrome_button,
};
use vesta_gpui::{
    Align, Axis, Dimension, Element, ElementId, ElementKey, ElementScope, Insets, Semantic,
    SemanticRole, Style, TextHAlign, TextVAlign,
};

pub const TITLE_BAR_HEIGHT: f32 = CHROME_TITLE_HEIGHT;

#[derive(Clone, Copy, Default)]
pub struct TitleBarState {
    pub maximized: bool,
    pub hovered: Option<ElementId>,
}

impl From<bool> for TitleBarState {
    fn from(maximized: bool) -> Self {
        Self {
            maximized,
            hovered: None,
        }
    }
}

pub struct TitleBarActions<A> {
    pub minimize: A,
    pub maximize: A,
    pub close: A,
}

pub struct TitleBarLabels<'a> {
    pub minimize: &'a str,
    pub maximize: &'a str,
    pub restore: &'a str,
    pub close: &'a str,
}

#[must_use]
pub fn title_bar<A>(
    scope: ElementScope,
    title: &str,
    detail: &str,
    state: impl Into<TitleBarState>,
    theme: WidgetTheme,
    labels: TitleBarLabels<'_>,
    commands: TitleBarActions<A>,
) -> Widget<A> {
    let state = state.into();
    let maximized = state.maximized;
    let mut actions = Vec::new();
    let mut root = Element::container(scope.element(1), ElementKey::new("title-bar"))
        .styled(
            Style::default()
                .size(Dimension::Percent(1.0), Dimension::Px(TITLE_BAR_HEIGHT))
                .axis(Axis::Row)
                .align(Align::Center)
                .gap(0.0)
                .padding(Insets {
                    left: 4.0,
                    ..Insets::default()
                })
                .background(theme.palette.chrome_title)
                .clipped(),
        )
        .child(
            Element::text(scope.element(2), ElementKey::new("title-bar.title"), title)
                .semantic(Semantic::new(SemanticRole::Label, title))
                .styled(
                    Style::default()
                        .size(Dimension::Px(58.0), Dimension::Px(TITLE_BAR_HEIGHT))
                        .text(theme.typography.font_size, theme.palette.text)
                        .text_align(TextHAlign::Start, TextVAlign::Baseline)
                        .pointer_events(false),
                ),
        )
        .child(
            Element::text(
                scope.element(3),
                ElementKey::new("title-bar.detail"),
                detail,
            )
            .styled(
                Style::default()
                    .size(Dimension::Auto, Dimension::Px(TITLE_BAR_HEIGHT))
                    .text(theme.typography.font_size, theme.palette.text_muted)
                    .text_align(TextHAlign::Start, TextVAlign::Baseline)
                    .pointer_events(false),
            ),
        )
        .child(
            Element::container(scope.element(4), ElementKey::new("title-bar.spacer"))
                .styled(Style::default().flex(1.0, 1.0).pointer_events(false)),
        );
    if cfg!(target_os = "windows") {
        let controls = [
            (9, 10, "−", labels.minimize, commands.minimize),
            (
                11,
                12,
                if maximized { "❐" } else { "□" },
                if maximized {
                    labels.restore
                } else {
                    labels.maximize
                },
                commands.maximize,
            ),
            (13, 14, "×", labels.close, commands.close),
        ];
        for (id, label_id, caption, accessible_name, action) in controls {
            let control = command_button(
                scope,
                id,
                label_id,
                caption,
                accessible_name,
                action,
                CHROME_WINDOW_CONTROL_WIDTH,
                true,
                state.hovered == Some(scope.element(id)),
                theme,
            );
            actions.extend(control.actions);
            root = root.child(control.element);
        }
    }
    root = root.child(
        Element::container(
            scope.element(15),
            ElementKey::new("title-bar.bottom.divider"),
        )
        .styled(
            Style::default()
                .absolute(0.0, TITLE_BAR_HEIGHT - 1.0)
                .size(Dimension::Percent(1.0), Dimension::Px(1.0))
                .background(theme.palette.chrome_divider)
                .pointer_events(false),
        ),
    );
    Widget {
        element: root,
        actions,
    }
}

#[allow(clippy::too_many_arguments)]
fn command_button<A>(
    scope: ElementScope,
    id: u32,
    label_id: u32,
    caption: &str,
    accessible_name: &str,
    action: A,
    width: f32,
    window_control: bool,
    hovered: bool,
    theme: WidgetTheme,
) -> Widget<A> {
    chrome_button(
        WidgetIdentity::new(scope.element(id), format!("title-bar.command.{id}")),
        WidgetIdentity::new(scope.element(label_id), format!("title-bar.label.{id}")),
        caption,
        accessible_name,
        action,
        ChromeButtonStyle {
            role: if window_control {
                ChromeButtonRole::Window
            } else {
                ChromeButtonRole::Command
            },
            width,
            selected: false,
            hovered,
            destructive: caption == "×",
        },
        theme,
    )
}
