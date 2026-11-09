use super::*;
use vesta_gpui::FontRole;
use vesta_gpui_widgets::{ListScrollAnchor, variable_list_with_items};

pub(super) struct View {
    metrics: Option<LazyListMetrics>,
    identities: Vec<(u32, u64)>,
    revision: Option<u64>,
    pub(super) scroll: ScrollAreaState,
    pub(super) following: bool,
    width: f32,
    height: f32,
    last_offset: f32,
    pub(super) pending: bool,
}

impl Default for View {
    fn default() -> Self {
        Self {
            metrics: None,
            identities: Vec::new(),
            revision: None,
            scroll: ScrollAreaState::default(),
            following: true,
            width: 300.0,
            height: VIEW_HEIGHT,
            last_offset: 0.0,
            pending: false,
        }
    }
}

#[derive(Clone, Copy)]
enum Position {
    Tail,
    Item(u32, ListScrollAnchor),
    Empty,
}

impl View {
    fn position(&self) -> Position {
        if self.following {
            return Position::Tail;
        }
        self.metrics
            .as_ref()
            .and_then(|metrics| {
                metrics
                    .metrics()
                    .range(self.height, self.scroll.offset_y(), 0)
                    .ok()
            })
            .and_then(|range| range.anchor())
            .and_then(|anchor| {
                self.identities
                    .get(anchor.index())
                    .map(|entry| Position::Item(entry.0, anchor))
            })
            .unwrap_or(Position::Empty)
    }

    fn restore(&mut self, position: Position) {
        let Some(metrics) = &self.metrics else { return };
        let metrics = metrics.metrics();
        self.scroll
            .sync_content_height(metrics.content_height(), self.height);
        let offset = match position {
            Position::Tail => metrics
                .range(self.height, f32::MAX, 0)
                .expect("bounded message tail")
                .offset(),
            Position::Item(id, anchor) if !self.identities.is_empty() => {
                let index = self
                    .identities
                    .iter()
                    .position(|entry| entry.0 == id)
                    .unwrap_or(anchor.index().min(self.identities.len() - 1));
                metrics
                    .offset_for_anchor(anchor, index, self.height)
                    .expect("current message anchor")
            }
            _ => 0.0,
        };
        self.scroll.scroll_by(offset - self.scroll.offset_y());
    }

    pub(super) fn prepare(&mut self, source: &Source, context: &mut TextInputContext) {
        // Wheel/thumb/focus reveal all update this existing ScrollAreaState.
        if self.scroll.offset_y() != self.last_offset {
            self.following = self.scroll.max_y() - self.scroll.offset_y() <= 1.0;
        }
        let position = self.position();
        let width = (self.width - 60.0).max(1.0);
        let metrics = self.metrics.get_or_insert_with(|| {
            LazyListMetrics::with_estimate(0, 36.0, width, context.measurement_revision())
                .expect("initial message geometry")
        });
        metrics
            .set_context(width, context.measurement_revision())
            .expect("positive Host text width");
        if self.revision != Some(source.message_revision) {
            let prefix = self
                .identities
                .iter()
                .zip(&source.messages)
                .take_while(|(a, b)| a.0 == b.id)
                .count();
            let suffix = self.identities[prefix..]
                .iter()
                .rev()
                .zip(source.messages[prefix..].iter().rev())
                .take_while(|(a, b)| a.0 == b.id)
                .count();
            let old_end = self.identities.len() - suffix;
            let new_end = source.messages.len() - suffix;
            metrics
                .splice(prefix..old_end, &vec![36.0; new_end - prefix])
                .expect("bounded source splice");
            self.identities.splice(
                prefix..old_end,
                source.messages[prefix..new_end]
                    .iter()
                    .map(|message| (message.id, message.revision)),
            );
            for (index, (cached, message)) in
                self.identities.iter_mut().zip(&source.messages).enumerate()
            {
                if cached.1 != message.revision {
                    metrics
                        .invalidate(index..index + 1)
                        .expect("live changed message");
                    cached.1 = message.revision;
                }
            }
            self.revision = Some(source.message_revision);
        }
        self.restore(position);
        let metrics = self.metrics.as_mut().expect("prepared message geometry");
        let range = metrics
            .metrics()
            .range(self.height, self.scroll.offset_y(), 2)
            .expect("bounded message viewport");
        let result = metrics
            .measure(range.visible_rows(), 8, |index| {
                let text = context
                    .shape_static_text_wrapped(
                        &source.messages[index].text,
                        13.0,
                        FontRole::Ui,
                        Some(width),
                    )
                    .expect("bounded Host message with registered fonts");
                Some((text.layout_height().ceil() + 8.0).max(36.0))
            })
            .expect("bounded measured message heights");
        assert!(result.attempted <= 8);
        self.restore(position);
        let metrics = self.metrics.as_ref().expect("prepared message geometry");
        self.pending = metrics
            .metrics()
            .range(self.height, self.scroll.offset_y(), 2)
            .expect("current message viewport")
            .visible_rows()
            .any(|index| metrics.is_measured(index) != Some(true));
        if source.messages.is_empty() {
            self.following = true;
        }
        self.last_offset = self.scroll.offset_y();
    }

    pub(super) fn sync_layout(&mut self, scope: ElementScope, layout: &LayoutTree) -> bool {
        let Some(node) = layout.node(scope.element(MESSAGE_VIEWPORT)) else {
            return false;
        };
        let viewport = node.scroll_viewport();
        if viewport.width > 0.0
            && viewport.height > 0.0
            && (self.width != viewport.width || self.height != viewport.height)
        {
            self.width = viewport.width;
            self.height = viewport.height;
            return true;
        }
        let changed = self.scroll.sync_layout(node);
        self.last_offset = self.scroll.offset_y();
        changed || self.pending
    }

    pub(super) fn latest(&mut self) {
        self.following = true;
    }

    pub(super) fn reveal(&mut self, source: &Source, id: u32) {
        let Some(index) = source.messages.iter().position(|message| message.id == id) else {
            return;
        };
        let Some((top, height)) = self
            .metrics
            .as_ref()
            .and_then(|metrics| metrics.metrics().row_bounds(index))
        else {
            return;
        };
        let offset = self.scroll.offset_y();
        let target = if top < offset {
            top
        } else if top + height > offset + self.height {
            top + height - self.height
        } else {
            offset
        };
        self.scroll.scroll_by(target - offset);
        self.following = self.scroll.max_y() - self.scroll.offset_y() <= 1.0;
        self.last_offset = self.scroll.offset_y();
        self.pending = self.metrics.as_ref().is_some_and(|metrics| {
            metrics
                .metrics()
                .range(self.height, self.scroll.offset_y(), 2)
                .expect("revealed message viewport")
                .visible_rows()
                .any(|index| metrics.is_measured(index) != Some(true))
        });
    }

    pub(super) fn project(
        &self,
        source: &Source,
        scope: ElementScope,
        focus: Option<Action>,
        theme: WidgetTheme,
    ) -> Widget<DemoAction> {
        let metrics = self
            .metrics
            .as_ref()
            .expect("visible page is prepared before projection")
            .metrics();
        let range = metrics
            .range(self.height, self.scroll.offset_y(), 2)
            .expect("bounded message range");
        let retained = match focus {
            Some(Action::Message(id)) => {
                source.messages.iter().position(|message| message.id == id)
            }
            _ => None,
        };
        let mut widget = variable_list_with_items(
            VirtualListIds {
                root: identity(scope, MESSAGE_VIEWPORT),
                content: identity(scope, MESSAGE_VIEWPORT + 1),
            },
            Dimension::Percent(1.0),
            &source.messages,
            range,
            retained,
            |message| {
                let base = MESSAGE_BASE + message.id * 8;
                let root = identity(scope, base);
                let text = identity(scope, base + 1);
                let index = source
                    .messages
                    .iter()
                    .position(|record| record.id == message.id)
                    .expect("live message");
                let height = metrics
                    .row_bounds(index)
                    .expect("measured or estimated row")
                    .1;
                Widget::from_element(
                    Element::container(root.element, root.key)
                        .semantic(Semantic::new(
                            SemanticRole::ListItem,
                            format!("Message {}", message.id),
                        ))
                        .styled(
                            Style::default()
                                .axis(Axis::Row)
                                .align(Align::Center)
                                .gap(6.0)
                                .padding(Insets::symmetric(7.0, 4.0)),
                        ),
                )
                .child(Widget::from_element(
                    Element::text(text.element, text.key, &message.text).styled(
                        Style::default()
                            .size(
                                Dimension::Px((self.width - 60.0).max(1.0)),
                                Dimension::Px((height - 8.0).max(1.0)),
                            )
                            .flex(0.0, 0.0)
                            .text(13.0, theme.palette.text)
                            .text_wrap(true)
                            .pointer_events(false),
                    ),
                ))
                .child({
                    let mut open = button(
                        identity(scope, base + 2),
                        identity(scope, base + 3),
                        "Open",
                        DemoAction::Sample(Action::Message(message.id)),
                        ControlState::Idle,
                        theme,
                    );
                    open.element.style.width = Dimension::Px(40.0);
                    open.element.style.padding = Insets::symmetric(4.0, 0.0);
                    open.element.semantic = Some(Semantic::new(
                        SemanticRole::Button,
                        format!("Open message {}", message.id),
                    ));
                    open.element.style.flex_shrink = 0.0;
                    open
                })
            },
        )
        .expect("matching stable message source and geometry");
        widget.element.style.scrollbar.gutter = vesta_gpui::ScrollbarGutter::Overlay;
        widget.element.semantic = Some(Semantic::new(SemanticRole::List, "Messages"));
        widget
    }
}
