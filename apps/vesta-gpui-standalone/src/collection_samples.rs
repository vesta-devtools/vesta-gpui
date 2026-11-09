//! Native collection consumers. Shared source lives in DemoState; View belongs to one window.
use super::*;
use vesta_gpui_widgets::{LazyListMetrics, TableColumnMetrics};

mod input;
mod messages;
mod table;

pub(super) const PANEL: &str = "gallery.streaming";
pub(super) fn fixture_requested() -> bool {
    std::env::args().any(|argument| argument == "--collections-fixture")
}
pub(super) const MESSAGE_VIEWPORT: u32 = 80_000_001;
pub(super) const TABLE_VIEWPORT: u32 = 80_000_102;
const ROW_HEIGHT: f32 = 28.0;
const VIEW_HEIGHT: f32 = 168.0;
const MESSAGE_BASE: u32 = 81_000_000;
const TABLE_ROW_BASE: u32 = 100_000_000;
const COLUMNS: usize = 512;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Action {
    Append,
    Prepend,
    Stream,
    Empty,
    Latest,
    Message(u32),
    Cell(u32, u16),
    Sort(u16),
    Copy,
    ReverseRows,
    MoveColumn,
    ResizeColumn,
    ToggleDirection,
    ToggleFreeze,
}

#[derive(Clone, Debug)]
struct Message {
    id: u32,
    revision: u64,
    text: String,
}

pub(super) struct Source {
    messages: Vec<Message>,
    message_revision: u64,
    next_message: u32,
    first_message: u32,
    rows: Vec<u32>,
    columns: Vec<u16>,
    widths: Vec<f32>,
    sort: Option<(u16, bool)>,
}

impl Default for Source {
    fn default() -> Self {
        Self {
            messages: (1000..2000)
                .map(|id| Message {
                    id,
                    revision: 0,
                    text: message_text(id),
                })
                .collect(),
            message_revision: 0,
            next_message: 2000,
            first_message: 1000,
            rows: (0..10_000).collect(),
            columns: (0..COLUMNS as u16).collect(),
            widths: vec![96.0; COLUMNS],
            sort: None,
        }
    }
}

fn message_text(id: u32) -> String {
    format!(
        "Message {id} · 消息\n{}",
        "A measured conversation row. ".repeat((id % 4 + 1) as usize)
    )
}

impl Source {
    fn change(&mut self, action: Action, selected_column: Option<u16>) -> bool {
        let changed = match action {
            Action::Append if self.messages.len() < 10_000 && self.next_message < 1_000_000 => {
                let id = self.next_message;
                self.next_message += 1;
                self.messages.push(Message {
                    id,
                    revision: 0,
                    text: message_text(id),
                });
                true
            }
            Action::Prepend if self.messages.len() <= 9_997 && self.first_message >= 3 => {
                let first = self.first_message - 3;
                self.messages.splice(
                    0..0,
                    (first..self.first_message).map(|id| Message {
                        id,
                        revision: 0,
                        text: message_text(id),
                    }),
                );
                self.first_message = first;
                true
            }
            Action::Stream => {
                if let Some(message) = self
                    .messages
                    .last_mut()
                    .filter(|message| message.text.len() < 4000)
                {
                    message.text.push_str("\nStreaming another line · 继续输出");
                    message.revision += 1;
                    true
                } else {
                    false
                }
            }
            Action::Empty if !self.messages.is_empty() => {
                self.messages.clear();
                true
            }
            Action::ReverseRows => {
                self.rows.reverse();
                self.sort = None;
                return true;
            }
            Action::Sort(column) if self.columns.contains(&column) => {
                let descending = self
                    .sort
                    .is_some_and(|(previous, descending)| previous == column && !descending);
                if descending {
                    self.rows
                        .sort_by_cached_key(|&row| std::cmp::Reverse(Self::value(row, column)));
                } else {
                    self.rows
                        .sort_by_cached_key(|&row| Self::value(row, column));
                }
                self.sort = Some((column, descending));
                return true;
            }
            Action::MoveColumn => {
                self.columns.swap(1, 40);
                return true;
            }
            Action::ResizeColumn => {
                let column = usize::from(selected_column.unwrap_or(1));
                let Some(width) = self.widths.get_mut(column) else {
                    return false;
                };
                *width = if *width >= 192.0 { 64.0 } else { *width + 48.0 };
                return true;
            }
            _ => false,
        };
        if changed {
            self.message_revision += 1;
        }
        changed
    }

    fn geometry(&self, view: &View) -> TableColumnMetrics {
        let widths = self
            .columns
            .iter()
            .map(|&column| self.widths[usize::from(column)])
            .collect::<Vec<_>>();
        // This consumer keeps the freeze preference through narrow panes, but
        // uses zero frozen columns until both ends leave a scrolling viewport.
        let frozen = self.effective_frozen(view);
        TableColumnMetrics::new(&widths, frozen, frozen).expect("bounded Host column widths")
    }

    fn effective_frozen(&self, view: &View) -> usize {
        let first = usize::from(self.columns[0]);
        let last = usize::from(self.columns[self.columns.len() - 1]);
        usize::from(view.freeze && view.table_width > self.widths[first] + self.widths[last])
    }

    fn value(row: u32, column: u16) -> String {
        match column {
            0 => format!("Asset {row}"),
            511 => "Ready".into(),
            _ => format!("{row}:{column}"),
        }
    }
}

pub(super) struct View {
    messages: messages::View,
    pub(super) table_scroll: ScrollAreaState,
    table_width: f32,
    rtl: bool,
    freeze: bool,
    selection: Option<((u32, u16), (u32, u16))>,
    focus: Option<Action>,
    pending_focus: Option<Action>,
    notice: String,
}

impl Default for View {
    fn default() -> Self {
        Self {
            messages: messages::View::default(),
            table_scroll: ScrollAreaState::default(),
            table_width: 300.0,
            rtl: false,
            freeze: true,
            selection: None,
            focus: None,
            pending_focus: None,
            notice: String::new(),
        }
    }
}

fn identity(scope: ElementScope, slot: u32) -> WidgetIdentity {
    WidgetIdentity::new(scope.element(slot), format!("collections.{slot}"))
}

pub(super) fn active(demo: &DemoState, container: ContainerRef) -> bool {
    container_panes(&demo.workspace, container)
        .into_iter()
        .filter_map(|pane| demo.workspace.pane(pane))
        .filter_map(|pane| pane.active_item())
        .any(|item| item.panel_type.as_str() == PANEL)
}

impl View {
    pub(super) fn prepare(
        &mut self,
        source: &Source,
        context: &mut TextInputContext,
        focus: Option<Action>,
    ) {
        let previous = self.focus;
        self.focus = self.pending_focus.or(focus);
        if self.focus != previous
            && self.pending_focus.is_none()
            && let Some(Action::Cell(row, column)) = self.focus
            && self.selection.map(|(_, end)| end) != Some((row, column))
        {
            self.selection = Some(((row, column), (row, column)));
        }
        let reveal = self
            .pending_focus
            .or(self.focus.filter(|focus| Some(*focus) != previous));
        self.messages.prepare(source, context);
        let total = source.widths.iter().sum();
        self.table_scroll.sync_content_size(
            Size::new(total, source.rows.len() as f32 * ROW_HEIGHT),
            Size::new(self.table_width, VIEW_HEIGHT),
        );
        self.prepare_reveal(source, reveal);
    }

    pub(super) fn sync_layout(&mut self, scope: ElementScope, layout: &LayoutTree) -> bool {
        let mut changed = self.messages.sync_layout(scope, layout);
        if let Some(node) = layout.node(scope.element(80_000_100)) {
            let width = node.rect.width;
            if width > 0.0 && self.table_width != width {
                self.table_width = width;
                changed = true;
            } else if let Some(node) = layout.node(scope.element(TABLE_VIEWPORT)) {
                changed |= self.table_scroll.sync_layout(node);
            }
        }
        changed
    }

    pub(super) fn scroll(&mut self, message: bool) -> &mut ScrollAreaState {
        if message {
            &mut self.messages.scroll
        } else {
            &mut self.table_scroll
        }
    }

    fn rectangle(
        &self,
        source: &Source,
    ) -> Option<(
        std::ops::RangeInclusive<usize>,
        std::ops::RangeInclusive<usize>,
    )> {
        let (anchor, end) = self.selection?;
        let row_a = source.rows.iter().position(|&id| id == anchor.0)?;
        let row_b = source.rows.iter().position(|&id| id == end.0)?;
        let col_a = source.columns.iter().position(|&id| id == anchor.1)?;
        let col_b = source.columns.iter().position(|&id| id == end.1)?;
        Some((
            row_a.min(row_b)..=row_a.max(row_b),
            col_a.min(col_b)..=col_a.max(col_b),
        ))
    }

    fn copy(&self, source: &Source) -> Result<String, &'static str> {
        let (rows, columns) = self.rectangle(source).ok_or("Select a cell to copy")?;
        if rows.clone().count().saturating_mul(columns.clone().count()) > 4096 {
            return Err("Select up to 4096 cells to copy");
        }
        // Fixture values contain no TSV delimiters. No generic export/clipboard service.
        Ok(rows
            .map(|row| {
                columns
                    .clone()
                    .map(|column| Source::value(source.rows[row], source.columns[column]))
                    .collect::<Vec<_>>()
                    .join("\t")
            })
            .collect::<Vec<_>>()
            .join("\n"))
    }
}

pub(super) fn register_page(
    input: &UiBuildState<'_>,
    pages: &mut HashMap<&str, Vec<Element>>,
    actions: &mut Vec<(ElementId, DemoAction)>,
) {
    if !active(input.demo, input.workspace_container) {
        return;
    }
    let scope = input.scopes.gallery;
    let view = input.collection_samples;
    let source = &input.demo.collection_source;
    let theme = input.theme;
    let mut page = Widget::from_element(
        Element::container(
            scope.element(80_000_000),
            ElementKey::new("collections.page"),
        )
        .styled(
            Style::default()
                .size(Dimension::Percent(1.0), Dimension::Auto)
                .gap(8.0)
                .padding(Insets::all(12.0)),
        ),
    );
    let label = |slot, text: String, role| {
        let id = identity(scope, slot);
        Widget::from_element(
            Element::text(id.element, id.key, text.clone())
                .semantic(Semantic::new(role, text))
                .styled(
                    Style::default()
                        .size(Dimension::Percent(1.0), Dimension::Auto)
                        .min_height(Dimension::Px(24.0))
                        .text(13.0, theme.palette.text)
                        .text_wrap(true)
                        .pointer_events(false),
                ),
        )
    };
    page = page.child(label(
        80_000_010,
        format!(
            "Messages · {} · {}",
            source.messages.len(),
            if view.messages.following {
                "Following latest"
            } else {
                "Reading history"
            }
        ),
        SemanticRole::Status,
    ));
    page = page.child(controls(
        scope,
        80_000_020,
        &[
            ("Append", Action::Append),
            ("Earlier", Action::Prepend),
            ("Stream", Action::Stream),
            ("Latest", Action::Latest),
            ("Empty", Action::Empty),
        ],
        theme,
    ));
    page = page.child(view.messages.project(source, scope, view.focus, theme));
    page = page.child(label(
        80_000_060,
        "Data · 10,000 rows × 512 fields · Shift+click / Shift+arrows selects a range".into(),
        SemanticRole::Label,
    ));
    page = page.child(controls(
        scope,
        80_000_070,
        &[
            ("Reverse", Action::ReverseRows),
            ("Move field", Action::MoveColumn),
            ("Resize", Action::ResizeColumn),
            ("Copy", Action::Copy),
        ],
        theme,
    ));
    page = page.child(controls(
        scope,
        80_000_080,
        &[
            (
                if view.rtl { "LTR" } else { "RTL" },
                Action::ToggleDirection,
            ),
            (
                if source.effective_frozen(view) > 0 {
                    "Unfreeze"
                } else if view.freeze {
                    "Freeze paused"
                } else {
                    "Freeze ends"
                },
                Action::ToggleFreeze,
            ),
        ],
        theme,
    ));
    page = page.child(table::project(source, view, scope, theme));
    page = page.child(label(80_000_090, view.notice.clone(), SemanticRole::Status));
    actions.extend(page.actions);
    pages.insert(PANEL, vec![page.element]);
}

fn controls(
    scope: ElementScope,
    base: u32,
    entries: &[(&str, Action)],
    theme: WidgetTheme,
) -> Widget<DemoAction> {
    let root = identity(scope, base);
    let mut widget = Widget::from_element(
        Element::container(root.element, root.key).styled(
            Style::default()
                .axis(Axis::Row)
                .gap(4.0)
                .flex_wrap(vesta_gpui::FlexWrap::Wrap),
        ),
    );
    for (index, &(caption, action)) in entries.iter().enumerate() {
        let mut control = button(
            identity(scope, base + 1 + index as u32 * 2),
            identity(scope, base + 2 + index as u32 * 2),
            caption,
            DemoAction::Sample(action),
            ControlState::Idle,
            theme,
        );
        if action == Action::ToggleFreeze && caption == "Freeze paused" {
            control.element = control
                .element
                .tooltip("Widen the pane to freeze both ends; activate to clear the preference");
        }
        widget = widget.child(control);
    }
    widget
}

#[cfg(test)]
mod tests;
