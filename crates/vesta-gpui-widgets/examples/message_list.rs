//! Caller-owned messages and scroll policy over budgeted shared geometry.
//! Run with one or more Host-provided fonts. No native window/provider is opened.
use std::{error::Error, sync::Arc};
use vesta_gpui::{
    AccessibilityTree, ActionRoute, Axis, Dimension, Element, ElementId, FontRole, HitTestTree,
    InputTarget, Point, RetainedLayoutEngine, Semantic, SemanticRole, Style, TextInputContext,
    UiQueryLimits, UiSnapshot, UiSnapshotFrame, UiSnapshotId, UiWindowId, Viewport,
};
use vesta_gpui_widgets::{
    LazyListMetrics, ListScrollAnchor, ScrollAreaState, VirtualListIds, Widget, WidgetIdentity,
    WidgetTheme, variable_list_with_items,
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;
const FONT_SIZE: f32 = 13.0;
const ESTIMATE: f32 = 32.0;
const BUDGET: usize = 8;

struct Message {
    id: u64,
    text: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Select(u64),
    Open(u64),
}
#[derive(Clone, Copy)]
enum Position {
    Tail,
    Item(u64, ListScrollAnchor),
    Empty,
}

struct Host {
    messages: Vec<Message>,
    geometry: LazyListMetrics,
    scroll: ScrollAreaState,
    following_tail: bool,
    width: f32,
    height: f32,
    next_id: u64,
    focused: Option<u64>,
    context: TextInputContext,
    layout: RetainedLayoutEngine,
    accessibility: AccessibilityTree,
    revision: u64,
}

fn identity(id: u64, part: u64) -> WidgetIdentity {
    WidgetIdentity::new(
        ElementId::new(100 + id * 10 + part),
        format!("message.{id}.{part}"),
    )
}
fn message(id: u64) -> Message {
    Message {
        id,
        text: format!(
            "Message {id}: {}",
            "English 与中文内容, wrapped by Core. ".repeat(1 + id as usize % 6)
        ),
    }
}

impl Host {
    fn new(context: TextInputContext) -> Result<Self> {
        Ok(Self {
            messages: (1..=1000).map(message).collect(),
            geometry: LazyListMetrics::with_estimate(
                1000,
                ESTIMATE,
                400.0,
                context.measurement_revision(),
            )?,
            scroll: ScrollAreaState::default(),
            following_tail: true,
            width: 440.0,
            height: 240.0,
            next_id: 1001,
            focused: Some(1),
            context,
            layout: RetainedLayoutEngine::default(),
            accessibility: AccessibilityTree::default(),
            revision: 0,
        })
    }

    fn position(&self) -> Result<Position> {
        if self.following_tail {
            return Ok(Position::Tail);
        }
        Ok(self
            .geometry
            .metrics()
            .range(self.height, self.scroll.offset_y(), 0)?
            .anchor()
            .and_then(|anchor| {
                self.messages
                    .get(anchor.index())
                    .map(|m| Position::Item(m.id, anchor))
            })
            .unwrap_or(Position::Empty))
    }

    fn restore(&mut self, position: Position) -> Result<()> {
        let metrics = self.geometry.metrics();
        self.scroll
            .sync_content_height(metrics.content_height(), self.height);
        let offset = match position {
            Position::Tail => metrics.range(self.height, f32::MAX, 0)?.offset(),
            Position::Item(id, anchor) if !self.messages.is_empty() => {
                let index = self
                    .messages
                    .iter()
                    .position(|m| m.id == id)
                    .unwrap_or(anchor.index().min(self.messages.len() - 1));
                metrics.offset_for_anchor(anchor, index, self.height)?
            }
            _ => 0.0,
        };
        self.scroll.scroll_by(offset - self.scroll.offset_y());
        Ok(())
    }

    fn wheel(&mut self, delta: f32) {
        self.scroll.scroll_by(delta);
        self.following_tail = self.scroll.max_y() - self.scroll.offset_y() <= 1.0;
    }

    fn append(&mut self, count: usize, prepend: bool) -> Result<()> {
        let position = self.position()?;
        let index = if prepend { 0 } else { self.messages.len() };
        let added = (0..count)
            .map(|_| {
                let id = self.next_id;
                self.next_id += 1;
                message(id)
            })
            .collect::<Vec<_>>();
        self.geometry.splice(index..index, &vec![ESTIMATE; count])?;
        self.messages.splice(index..index, added);
        self.restore(position)
    }

    fn stream(&mut self) -> Result<()> {
        let position = self.position()?;
        if let Some(last) = self.messages.last_mut() {
            last.text.push_str(&"More streamed 内容. ".repeat(12));
            let index = self.messages.len() - 1;
            self.geometry.invalidate(index..index + 1)?;
        }
        self.restore(position)
    }

    fn resize(&mut self, width: f32) -> Result<()> {
        let position = self.position()?;
        self.geometry
            .set_context(width - 40.0, self.context.measurement_revision())?;
        self.width = width;
        self.restore(position)
    }

    fn settle(&mut self) -> Result<usize> {
        for pass in 1..=8 {
            let position = self.position()?;
            self.restore(position)?;
            let range = self
                .geometry
                .metrics()
                .range(self.height, self.scroll.offset_y(), 2)?
                .visible_rows();
            let messages = &self.messages;
            let context = &mut self.context;
            let width = self.width - 40.0;
            let result = self.geometry.measure(range, BUDGET, |index| {
                context
                    .shape_static_text_wrapped(
                        &messages[index].text,
                        FONT_SIZE,
                        FontRole::Ui,
                        Some(width),
                    )
                    .ok()
                    .map(|snapshot| snapshot.layout_height().ceil().max(28.0))
            })?;
            assert!(result.attempted <= BUDGET);
            self.restore(position)?;
            let range = self
                .geometry
                .metrics()
                .range(self.height, self.scroll.offset_y(), 2)?
                .visible_rows();
            if range
                .clone()
                .all(|index| self.geometry.is_measured(index) == Some(true))
            {
                return Ok(pass);
            }
        }
        Err("message measurement did not settle within the Host budget".into())
    }

    fn frame(&mut self, label: &str) -> Result<()> {
        let passes = self.settle()?;
        let theme = WidgetTheme::dark();
        let range = self
            .geometry
            .metrics()
            .range(self.height, self.scroll.offset_y(), 2)?;
        let visible = range.visible_rows();
        let retained = self
            .focused
            .and_then(|id| self.messages.iter().position(|m| m.id == id));
        let expected_rows =
            visible.len() + usize::from(retained.is_some_and(|index| !visible.contains(&index)));
        let text_width = self.width - 40.0;
        let mut built = 0;
        let mut widget = variable_list_with_items(
            VirtualListIds {
                root: WidgetIdentity::new(ElementId::new(1), "messages"),
                content: WidgetIdentity::new(ElementId::new(2), "messages.content"),
            },
            Dimension::Px(self.width),
            &self.messages,
            range,
            retained,
            |record| {
                built += 1;
                let root = identity(record.id, 0);
                let text = identity(record.id, 1);
                let button = identity(record.id, 2);
                Widget::from_element(
                    Element::container(root.element, root.key)
                        .semantic(Semantic::new(
                            SemanticRole::ListItem,
                            format!("Message {}", record.id),
                        ))
                        .styled(Style::default().axis(Axis::Row).gap(8.0).focusable(true)),
                )
                .on_click(Action::Select(record.id))
                .child(Widget::from_element(
                    Element::text(text.element, text.key, record.text.clone()).styled(
                        Style::default()
                            .size(Dimension::Px(text_width), Dimension::Percent(1.0))
                            .flex(0.0, 0.0)
                            .text(FONT_SIZE, theme.palette.text)
                            .text_wrap(true)
                            .pointer_events(false),
                    ),
                ))
                .child(
                    Widget::from_element(
                        Element::container(button.element, button.key)
                            .semantic(Semantic::new(
                                SemanticRole::Button,
                                format!("Open message {}", record.id),
                            ))
                            .styled(
                                Style::default()
                                    .size(Dimension::Px(32.0), Dimension::Px(28.0))
                                    .flex(0.0, 0.0)
                                    .background(theme.palette.surface)
                                    .focusable(true),
                            ),
                    )
                    .on_click(Action::Open(record.id)),
                )
            },
        )?;
        // The Host chooses overlay gutters, so the measured content width is exact.
        widget.element.style.scrollbar.gutter = vesta_gpui::ScrollbarGutter::Overlay;
        assert_eq!(built, expected_rows);
        assert!(built <= 20);
        let viewport = Viewport::new(self.width, self.height, 1.5);
        let layout =
            self.layout
                .compute_with_text_context(&widget.element, viewport, &mut self.context)?;
        let route = ActionRoute::build(&widget.element, widget.actions)?;
        let hits = HitTestTree::build(&widget.element, &layout);
        let focus = self.focused.map(|id| identity(id, 2).element);
        let accessible = self
            .accessibility
            .build(&widget.element, &layout, focus, "Messages");
        self.revision += 1;
        let snapshot = UiSnapshot::capture(
            UiSnapshotFrame {
                id: UiSnapshotId {
                    instance: 1,
                    window: UiWindowId(1),
                    revision: self.revision,
                },
                root: &widget.element,
                layout: &layout,
                route: &route,
                focus: focus.map(|element| InputTarget {
                    window: UiWindowId(1),
                    element,
                }),
                modal_root: None,
            },
            UiQueryLimits::default(),
            |_| None,
            |_| false,
        )?;
        assert!(!snapshot.node_limit_reached && !snapshot.text_truncated);
        if let Some(index) = self
            .geometry
            .metrics()
            .range(self.height, self.scroll.offset_y(), 0)?
            .anchor()
            .map(|a| a.index())
        {
            let id = self.messages[index].id;
            let rect = layout
                .node(identity(id, 0).element)
                .expect("visible row")
                .rect;
            assert_eq!(
                hits.hit(Point::new(4.0, rect.y.max(0.0) + 0.25)),
                Some(identity(id, 0).element)
            );
        }
        if let Some(id) = self.focused {
            let child = identity(id, 2).element;
            assert_eq!(route.action_at(child), Some(&Action::Open(id)));
            assert!(route.focus_order().contains(&child));
            assert_eq!(
                accessible.focus,
                self.accessibility
                    .node_for_element(child)
                    .expect("retained child semantics")
            );
            assert!(
                snapshot
                    .nodes()
                    .iter()
                    .any(|node| node.id == child && node.focused)
            );
        }
        if self.following_tail {
            assert!((self.scroll.max_y() - self.scroll.offset_y()).abs() <= 1.0);
        }
        println!(
            "{label}: source={} built={built} passes={passes} offset={:.1} following={}",
            self.messages.len(),
            self.scroll.offset_y(),
            self.following_tail
        );
        Ok(())
    }
}

fn observed(host: &Host) -> Result<(u64, f32)> {
    match host.position()? {
        Position::Item(id, anchor) => Ok((id, anchor.offset_in_row())),
        _ => Err("expected a reader anchor".into()),
    }
}

fn main() -> Result<()> {
    let mut context = TextInputContext::default();
    let fonts = std::env::args_os().skip(1).collect::<Vec<_>>();
    if fonts.is_empty() {
        return Err("pass one or more Host font paths".into());
    }
    for font in fonts {
        assert!(context.register_font(Arc::new(std::fs::read(font)?)));
    }
    let mut host = Host::new(context)?;
    host.frame("initial tail")?;
    host.append(3, false)?;
    host.frame("append while following")?;
    host.stream()?;
    host.frame("stream while following")?;
    host.wheel(-4000.0);
    host.frame("reader scrolls up")?;
    assert!(!host.following_tail);
    let anchor = observed(&host)?;
    host.append(5, true)?;
    host.frame("prepend history")?;
    assert_eq!(observed(&host)?, anchor);
    host.append(2, false)?;
    host.stream()?;
    host.frame("append while reading history")?;
    assert_eq!(observed(&host)?, anchor);
    host.resize(280.0)?;
    host.frame("narrow reader viewport")?;
    assert_eq!(observed(&host)?.0, anchor.0);
    host.following_tail = true;
    host.frame("jump to latest")?;
    host.resize(440.0)?;
    host.frame("wide tail viewport")?;
    let count = host.messages.len();
    host.geometry.splice(0..count, &[])?;
    host.messages.clear();
    host.focused = None;
    host.frame("empty")?;
    host.append(1, false)?;
    host.focused = host.messages.first().map(|m| m.id);
    host.frame("append after empty")?;
    Ok(())
}
