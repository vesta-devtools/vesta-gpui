//! Application view and data for the two reference consumers; no product state.
use super::*;
use vesta_gpui::{Axis, Dimension, Element, ElementKey, Insets, Semantic, SemanticRole, Style};
use vesta_gpui_widgets::{ControlState, Widget, WidgetIdentity, WidgetTheme, button, text_field};

pub(super) const INCREMENT: u32 = 20;
pub(super) const EDITOR: u32 = 30;
pub(super) const SCROLL: u32 = 40;

#[derive(Clone, Copy, Debug)]
// Each binary constructs one mode; both views share this example-only host.
#[allow(dead_code)]
pub enum Mode {
    Starter,
    Interaction,
}

impl Mode {
    pub(super) fn title(self) -> &'static str {
        match self {
            Self::Starter => "GPUI native starter",
            Self::Interaction => "GPUI native interaction",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) enum Action {
    Increment,
    Theme,
}

pub(super) struct Model {
    pub mode: Mode,
    pub scope: vesta_gpui::ElementScope,
    pub count: u32,
    pub text: TextInputState,
    pub input: InputOwnership,
    pub scroll: vesta_gpui_widgets::ScrollAreaState,
    pub theme: WidgetTheme,
}

impl Model {
    pub fn id(&self, local: u32) -> ElementId {
        self.scope.element(local)
    }
    fn identity(&self, local: u32) -> WidgetIdentity {
        WidgetIdentity::new(self.id(local), format!("native.{local}"))
    }
    fn label(&self, local: u32, text: impl Into<String>) -> Widget<Action> {
        let text = text.into();
        Widget::from_element(
            Element::text(
                self.id(local),
                ElementKey::new(format!("native.{local}")),
                &text,
            )
            .semantic(Semantic::new(SemanticRole::Label, text))
            .styled(
                Style::default()
                    .text(16.0, self.theme.palette.text)
                    .flex(0.0, 0.0),
            ),
        )
    }
    pub fn view(&self) -> Widget<Action> {
        let mut root = Widget::from_element(
            Element::container(self.id(1), ElementKey::new("native.root")).styled(
                Style::default()
                    .size(Dimension::Percent(1.0), Dimension::Percent(1.0))
                    .axis(Axis::Column)
                    .padding(Insets::all(20.0))
                    .gap(12.0)
                    .background(self.theme.palette.canvas),
            ),
        )
        .child(self.label(2, self.mode.title()))
        .child(self.label(3, format!("Actions: {}", self.count)));
        for (local, title, action) in [
            (INCREMENT, "Increment", Action::Increment),
            (22, "Switch theme", Action::Theme),
        ] {
            let target = InputTarget {
                window: WINDOW,
                element: self.id(local),
            };
            let state = if self.input.capture().is_some_and(|c| c.target == target) {
                ControlState::Pressed
            } else {
                ControlState::Idle
            };
            root = root.child(button(
                self.identity(local),
                self.identity(local + 1),
                title,
                action,
                state,
                self.theme,
            ));
        }
        if matches!(self.mode, Mode::Interaction) {
            root = root
                .child(self.label(
                    4,
                    "Tab to Text. Type, select, Copy/Paste, Undo/Redo. Enter increments.",
                ))
                .child(Widget::from_element(text_field(
                    self.identity(EDITOR),
                    self.text.snapshot(),
                    self.input
                        .focus()
                        .is_some_and(|t| t.element == self.id(EDITOR)),
                    self.theme,
                )));
            root = root.child(Widget::from_element(
                Element::image(
                    self.id(5),
                    ElementKey::new("native.image"),
                    "example://checker",
                )
                .semantic(Semantic::new(
                    SemanticRole::Label,
                    "Host-supplied checker image",
                ))
                .styled(
                    Style::default()
                        .size(Dimension::Px(96.0), Dimension::Px(40.0))
                        .flex(0.0, 0.0),
                ),
            ));
            let mut rows = Widget::from_element(
                Element::container(self.id(41), ElementKey::new("native.rows"))
                    .styled(Style::default().axis(Axis::Column).gap(6.0).flex(0.0, 0.0)),
            );
            for row in 0..32 {
                rows = rows.child(self.label(
                    100 + row,
                    format!("Scroll row {:02} · caller-owned content", row + 1),
                ));
            }
            root = root.child(
                Widget::from_element(
                    Element::scroll_box(
                        self.id(SCROLL),
                        ElementKey::new("native.scroll"),
                        self.scroll.offset(),
                    )
                    .styled(
                        Style::default()
                            .size(Dimension::Percent(1.0), Dimension::Px(180.0))
                            .min_height(Dimension::Px(64.0))
                            .scrollbar(self.theme.scrollbar_style()),
                    ),
                )
                .child(rows),
            );
        }
        root
    }
}

pub(super) struct Images(Arc<vesta_gpui::render::ImageData>);
impl Default for Images {
    fn default() -> Self {
        let mut rgba = Vec::with_capacity(8 * 8 * 4);
        for y in 0..8 {
            for x in 0..8 {
                rgba.extend_from_slice(if (x + y) % 2 == 0 {
                    &[64, 128, 220, 255]
                } else {
                    &[230, 236, 244, 255]
                });
            }
        }
        Self(Arc::new(vesta_gpui::render::ImageData {
            width: 8,
            height: 8,
            rgba,
        }))
    }
}
impl vesta_gpui::render::ImageResolver for Images {
    fn resolve(&self, source: &str) -> Option<vesta_gpui::render::ImageData> {
        (source == "example://checker").then(|| (*self.0).clone())
    }
    fn resolve_state(&self, source: &str) -> vesta_gpui::render::ImageResolution {
        if source == "example://checker" {
            vesta_gpui::render::ImageResolution::Ready(self.0.clone())
        } else {
            vesta_gpui::render::ImageResolution::Missing
        }
    }
}
