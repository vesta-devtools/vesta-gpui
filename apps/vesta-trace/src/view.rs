//! GPUI projection of the independent session and analysis models.
mod counters;
mod gpu;
mod panels;
use crate::{analysis::Analysis, session::*, timeline::Timeline};
use crate::{
    inspection::{Inspection, Panel},
    query::protocol::Cursor,
};
use vesta_gpui::{
    Color, Dimension, Element, ElementId, ElementKey, Rect, Semantic, SemanticRole, Style,
    TextInputSnapshot,
};
use vesta_gpui_widgets::{ControlState, Widget, WidgetIdentity, WidgetTheme, button};

#[derive(Clone, Debug)]
pub enum Command {
    Gpu(crate::query::protocol::GpuKey),
    Fit,
    ZoomIn,
    ZoomOut,
    Worst,
    Previous,
    Next,
    Scope(EventKey),
    Frame(EventKey),
    Timer(String, String),
    FocusSearch,
    ClearSearch,
    Cancel,
    Panel(Panel),
    PageNext(Cursor),
    PagePrevious,
    Timeline,
    Table,
}
pub struct Projection {
    pub widget: Widget<Command>,
    pub timeline: Rect,
    pub drawn_events: usize,
    pub collapsed_events: usize,
    pub table: Rect,
    pub table_rows: usize,
    pub table_page_rows: usize,
    pub timeline_scroll_max: f32,
    pub visible_events: usize,
    pub analysis_range: Option<TimeRange>,
}
pub const SEARCH: ElementId = ElementId::new(80);
pub const TRACK: ElementId = ElementId::new(90);
pub const TABLE: ElementId = ElementId::new(92);
pub const GPU_DETAIL: ElementId = ElementId::new(136);
pub struct View<'a> {
    pub session: Option<&'a Session>,
    pub analysis: Option<&'a Analysis>,
    pub timeline: Option<&'a Timeline>,
    pub inspection: Option<&'a Inspection>,
    pub tracks: Option<&'a crate::plot::FilteredTracks>,
    pub width: f32,
    pub height: f32,
    pub status: &'a str,
    pub filter: &'a str,
    pub search: TextInputSnapshot,
    pub search_focused: bool,
    pub panel: Panel,
    pub table_scroll: usize,
    pub gpu_scroll: vesta_gpui::Point,
    pub previous_page: bool,
    pub busy: bool,
}
struct Builder {
    root: Widget<Command>,
    next: u64,
}
const TEXT: Color = Color::rgb8(221, 229, 239);
const MUTED: Color = Color::rgb8(142, 157, 178);
const ACCENT: Color = Color::rgb8(96, 201, 187);
const GOLD: Color = Color::rgb8(240, 183, 100);

fn geometry(rect: Rect) -> Style {
    Style::default()
        .absolute(rect.x, rect.y)
        .size(
            Dimension::Px(rect.width.max(0.0)),
            Dimension::Px(rect.height.max(0.0)),
        )
        .clipped()
}
fn id(value: u64) -> ElementId {
    ElementId::new(value)
}
fn identity(value: u64) -> WidgetIdentity {
    WidgetIdentity::new(id(value), format!("trace.{value}"))
}
impl Builder {
    fn new(width: f32, height: f32) -> Self {
        Self {
            root: Widget::from_element(
                Element::container(id(1), ElementKey::new("trace.root")).styled(
                    Style::default()
                        .size(Dimension::Px(width), Dimension::Px(height))
                        .background(Color::rgb8(18, 23, 31)),
                ),
            ),
            next: 9_000_000,
        }
    }
    fn add(&mut self, widget: Widget<Command>) {
        self.root.actions.extend(widget.actions);
        self.root.element.children.push(widget.element);
    }
    fn label(&mut self, rect: Rect, text: impl Into<String>, size: f32, color: Color) {
        let text = preview(text.into());
        let value = self.next;
        self.next += 1;
        self.add(Widget::from_element(
            Element::text(
                id(value),
                ElementKey::new(format!("trace.label.{value}")),
                text.clone(),
            )
            .semantic(Semantic::new(SemanticRole::Label, text))
            .styled(
                geometry(rect)
                    .text(size, color)
                    .text_wrap(false)
                    .pointer_events(false),
            ),
        ));
    }
    fn block(&mut self, rect: Rect, color: Color) {
        let value = self.next;
        self.next += 1;
        self.add(Widget::from_element(
            Element::container(id(value), ElementKey::new(format!("trace.block.{value}")))
                .styled(geometry(rect).background(color).pointer_events(false)),
        ));
    }
    fn control(&mut self, value: u64, rect: Rect, title: &str, action: Command) {
        let mut control = button(
            identity(value),
            identity(value + 1),
            title,
            action,
            ControlState::Idle,
            WidgetTheme::default(),
        );
        control.element.style = control
            .element
            .style
            .absolute(rect.x, rect.y)
            .size(Dimension::Px(rect.width), Dimension::Px(rect.height));
        self.add(control);
    }
    fn target(
        &mut self,
        value: u64,
        rect: Rect,
        title: &str,
        color: Color,
        selected: bool,
        action: Command,
    ) {
        let mut style = geometry(rect)
            .background(color)
            .radius(2.0)
            .focusable(true)
            .focus_indicator(vesta_gpui::FocusIndicator::Control);
        if selected {
            style = style.border(1.0, GOLD);
        }
        let element =
            Element::container(id(value), ElementKey::new(format!("trace.slice.{value}")))
                .semantic(Semantic::new(SemanticRole::Button, preview(title.into())))
                .styled(style);
        self.add(Widget::from_element(element).on_click(action));
    }
    fn selectable(
        &mut self,
        value: u64,
        rect: Rect,
        title: &str,
        color: Color,
        selected: bool,
        action: Command,
    ) {
        self.target(value, rect, title, color, selected, action);
        if rect.width > 42.0 {
            self.label(
                Rect::new(
                    rect.x + 5.0,
                    rect.y + 2.0,
                    (rect.width - 10.0).max(0.0),
                    rect.height - 2.0,
                ),
                title,
                11.0,
                TEXT,
            );
        }
    }
}
fn color(category: &str) -> Color {
    let hash = category.bytes().fold(2166136261u32, |h, b| {
        h.wrapping_mul(16777619) ^ u32::from(b)
    });
    let colors = [
        Color::rgb8(51, 111, 131),
        Color::rgb8(84, 91, 143),
        Color::rgb8(117, 91, 61),
        Color::rgb8(53, 121, 99),
        Color::rgb8(115, 72, 108),
        Color::rgb8(73, 110, 163),
    ];
    colors[hash as usize % colors.len()]
}
fn ms(ns: u64) -> String {
    format!("{:.3}", ns as f64 / 1_000_000.0)
}
fn preview(mut text: String) -> String {
    if text.len() > 512 {
        let mut end = 512;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
        text.push('…');
    }
    text
}

pub fn build(view: View<'_>) -> Projection {
    let _projection = vesta_gpui::trace::span("vesta.trace", "view.project");
    let View {
        session,
        analysis,
        timeline,
        inspection,
        tracks,
        width,
        height,
        status,
        filter,
        search,
        search_focused,
        panel,
        table_scroll,
        gpu_scroll,
        previous_page,
        busy,
    } = view;
    let mut b = Builder::new(width, height);
    b.block(Rect::new(0.0, 0.0, width, 58.0), Color::rgb8(24, 31, 42));
    b.label(
        Rect::new(20.0, 12.0, 180.0, 34.0),
        "VESTA TRACE",
        22.0,
        ACCENT,
    );
    b.label(
        Rect::new(
            210.0,
            18.0,
            (width - if busy { 330.0 } else { 230.0 }).max(0.0),
            27.0,
        ),
        status,
        13.0,
        MUTED,
    );
    if busy {
        b.control(
            86,
            Rect::new(width - 108.0, 14.0, 88.0, 28.0),
            "Cancel",
            Command::Cancel,
        );
    }
    for (n, (title, action, w)) in [
        ("Fit all", Command::Fit, 76.0),
        ("+", Command::ZoomIn, 40.0),
        ("−", Command::ZoomOut, 40.0),
        ("Slowest frame", Command::Worst, 126.0),
        ("Previous", Command::Previous, 92.0),
        ("Next", Command::Next, 68.0),
    ]
    .into_iter()
    .enumerate()
    {
        let x = 20.0 + [0.0, 84.0, 132.0, 180.0, 314.0, 414.0][n];
        b.control(
            10 + n as u64 * 2,
            Rect::new(x, 68.0, w, 28.0),
            title,
            action,
        );
    }
    b.label(Rect::new(530.0, 73.0, 48.0, 22.0), "Find", 12.0, MUTED);
    let mut search = vesta_gpui_widgets::text_field(
        identity(80),
        search,
        search_focused,
        WidgetTheme::default(),
    )
    .semantic(Semantic::new(
        SemanticRole::TextInput,
        "Search recorded labels",
    ));
    search.style = search.style.absolute(580.0, 66.0).size(
        Dimension::Px((width - 690.0).max(120.0)),
        Dimension::Px(31.0),
    );
    b.add(Widget::from_element(search));
    b.control(
        82,
        Rect::new(width - 92.0, 68.0, 72.0, 28.0),
        "Clear",
        Command::ClearSearch,
    );
    let track = Rect::new(
        188.0,
        218.0,
        (width - 208.0).max(1.0),
        (height * 0.55 - 218.0).max(100.0),
    );
    b.add(
        Widget::from_element(
            Element::container(TRACK, ElementKey::new("trace.timeline"))
                .semantic(Semantic::new(
                    SemanticRole::Group,
                    "Recorded thread timeline",
                ))
                .styled(
                    geometry(track)
                        .focusable(true)
                        .focus_indicator(vesta_gpui::FocusIndicator::Control),
                ),
        )
        .on_click(Command::Timeline),
    );
    let Some((session, timeline)) = session.zip(timeline) else {
        b.label(
            Rect::new(32.0, 150.0, width - 64.0, 45.0),
            "Open a GPUI trace to inspect recorded frames and threads",
            22.0,
            TEXT,
        );
        b.label(
            Rect::new(32.0, 205.0, width - 64.0, 32.0),
            "Pass a .json / .html file, or drop one onto this window.",
            14.0,
            MUTED,
        );
        return Projection {
            widget: b.root,
            analysis_range: None,
            timeline: track,
            drawn_events: 0,
            collapsed_events: 0,
            table: Rect::default(),
            table_rows: 0,
            table_page_rows: 1,
            timeline_scroll_max: 0.0,
            visible_events: 0,
        };
    };
    b.label(
        Rect::new(20.0, 111.0, 160.0, 24.0),
        "CPU FRAME ATTEMPTS",
        11.0,
        MUTED,
    );
    let overview = Rect::new(track.x, 110.0, track.width, 65.0);
    b.block(overview, Color::rgb8(25, 32, 43));
    let columns = (track.width / 2.0).ceil() as usize;
    let maximum = session
        .frame_plot
        .longest(&session.frames)
        .map_or(1, |i| session.events[i].end - session.events[i].start)
        .max(1);
    for bucket in
        session
            .frame_plot
            .buckets(&session.frames, &session.events, session.bounds, columns)
    {
        let index = bucket.index;
        let e = &session.events[index];
        let h = ((e.end - e.start) as f64 / maximum as f64 * 58.0).max(2.0) as f32;
        b.selectable(
            4_000_000 + index as u64,
            Rect::new(
                overview.x
                    + ((e.start - session.bounds.start) as f64 / session.bounds.duration() as f64
                        * f64::from(overview.width)) as f32,
                overview.bottom() - h,
                2.0,
                h,
            ),
            &e.name,
            ACCENT,
            false,
            Command::Frame(e.key),
        );
    }
    b.label(
        Rect::new(20.0, 142.0, 160.0, 24.0),
        format!("{} frames · peak / bin", session.frames.len()),
        11.0,
        MUTED,
    );
    b.block(
        Rect::new(track.x, 186.0, track.width, 30.0),
        Color::rgb8(29, 37, 49),
    );
    let ticks = (track.width / 110.0).floor().max(2.0) as u32;
    for tick in 0..ticks {
        let fraction = f64::from(tick) / f64::from(ticks);
        let x = track.x + track.width * fraction as f32;
        b.label(
            Rect::new(x + 4.0, 190.0, track.width / ticks as f32 - 8.0, 23.0),
            format!("{} ms", ms(timeline.at(fraction))),
            11.0,
            MUTED,
        );
        b.block(
            Rect::new(x, track.y, 1.0, track.height),
            Color::rgb8(37, 46, 60),
        );
    }
    let filter = filter.to_lowercase();
    let tracks = tracks.filter(|t| t.source_sha256 == session.source.sha256 && t.text == filter);
    let mut y = track.y - timeline.scroll;
    let mut drawn = 0;
    let mut collapsed = 0;
    let mut visible_events = 0;
    if panel == Panel::Counters {
        let counts = counters::build(&mut b, session, timeline, tracks, &filter, track, columns);
        drawn = counts.drawn;
        collapsed = counts.collapsed;
        visible_events = counts.visible;
        y = track.y - timeline.scroll + counts.height;
    } else {
        for (lane_index, lane) in session.lanes.iter().enumerate() {
            if y >= track.y && y + 24.0 <= track.bottom() {
                b.block(
                    Rect::new(12.0, y, width - 24.0, 24.0),
                    Color::rgb8(30, 39, 51),
                );
                b.label(
                    Rect::new(20.0, y + 3.0, 160.0, 20.0),
                    format!("P{} / thread {}", lane.key.process, lane.key.thread),
                    12.0,
                    TEXT,
                );
                b.label(
                    Rect::new(track.x + 4.0, y + 3.0, track.width - 8.0, 20.0),
                    "Capture-local thread · nested recorded scopes",
                    11.0,
                    MUTED,
                );
            }
            y += 27.0;
            for (row_index, row) in lane.rows.iter().enumerate() {
                if y + 20.0 > track.y && y < track.bottom() {
                    if y >= track.y {
                        b.label(
                            Rect::new(26.0, y + 2.0, 145.0, 18.0),
                            format!("depth {}", row.depth),
                            11.0,
                            MUTED,
                        );
                    }
                    let row = if filter.is_empty() {
                        Some(row)
                    } else {
                        tracks
                            .and_then(|t| t.lanes.get(lane_index))
                            .and_then(|rows| rows.get(row_index))
                    };
                    let buckets = row.map_or_else(Vec::new, |row| {
                        row.plot
                            .buckets(&row.events, &session.events, timeline.view, columns)
                    });
                    for bucket in buckets {
                        visible_events += bucket.count;
                        let index = bucket.index;
                        let e = &session.events[index];
                        let x = track.x
                            + (timeline.fraction(e.start).max(0.0) * f64::from(track.width)) as f32;
                        let x = x.min(track.right() - 1.0);
                        let right = track.x
                            + (timeline.fraction(e.end).min(1.0) * f64::from(track.width)) as f32;
                        if drawn >= 6000 {
                            collapsed += bucket.count;
                            continue;
                        }
                        collapsed += bucket.count - 1;
                        let top = y.max(track.y);
                        let bottom = (y + 20.0).min(track.bottom());
                        let title = match e.kind {
                            EventKind::Counter(v) => format!("{} = {v}", e.name),
                            EventKind::Marker => format!("◆ {}", e.name),
                            _ => e.name.clone(),
                        };
                        b.selectable(
                            10_000 + index as u64,
                            Rect::new(
                                x,
                                top,
                                (right - x).max(2.0).min(track.right() - x),
                                bottom - top,
                            ),
                            &title,
                            color(&e.category),
                            timeline.selected == Some(e.key),
                            Command::Scope(e.key),
                        );
                        drawn += 1;
                    }
                }
                y += 23.0;
            }
            y += 10.0;
        }
    }
    if let Some(selection) = timeline.selection {
        let left =
            (timeline.fraction(selection.start).clamp(0.0, 1.0) * f64::from(track.width)) as f32;
        let right =
            (timeline.fraction(selection.end).clamp(0.0, 1.0) * f64::from(track.width)) as f32;
        b.block(
            Rect::new(track.x + left, track.y, right - left, track.height),
            Color::srgba(0.3, 0.65, 0.8, 0.16),
        );
    }
    let table = panels::build(
        &mut b,
        panels::Panels {
            session,
            analysis,
            timeline,
            inspection,
            panel,
            scroll: table_scroll,
            gpu_scroll,
            previous: previous_page,
            width,
            height,
            top: track.bottom() + 18.0,
        },
    );
    let diagnostic=session.diagnostics.first().map(|d|format!("{}: {}",d.code,d.message)).unwrap_or_else(||"No recorded loss or nesting errors. Wall time includes waits; source code outside recorded scopes is unattributed.".into());
    b.block(
        Rect::new(0.0, height - 34.0, width, 34.0),
        Color::rgb8(27, 34, 45),
    );
    b.label(
        Rect::new(16.0, height - 27.0, width - 32.0, 24.0),
        format!("{diagnostic}  ·  {drawn} drawn / {collapsed} collapsed"),
        11.0,
        if session.diagnostics.is_empty() {
            MUTED
        } else {
            GOLD
        },
    );
    Projection {
        widget: b.root,
        analysis_range: analysis.map(|_| table.range),
        timeline: track,
        drawn_events: drawn,
        collapsed_events: collapsed,
        table: table.area,
        table_rows: table.rows,
        table_page_rows: table.page_rows,
        timeline_scroll_max: (y + timeline.scroll - track.y - track.height).max(0.0),
        visible_events,
    }
}
