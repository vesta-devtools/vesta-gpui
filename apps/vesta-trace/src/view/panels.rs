//! Tables and source-addressed detail projections; all analysis stays off-view.
use super::*;
use crate::query::protocol::{Data, EventRecord, TimerRecord};

pub(super) struct Panels<'a> {
    pub session: &'a Session,
    pub analysis: Option<&'a Analysis>,
    pub timeline: &'a Timeline,
    pub inspection: Option<&'a Inspection>,
    pub panel: Panel,
    pub scroll: usize,
    pub gpu_scroll: vesta_gpui::Point,
    pub previous: bool,
    pub width: f32,
    pub height: f32,
    pub top: f32,
}
pub(super) struct TableProjection {
    pub area: Rect,
    pub rows: usize,
    pub page_rows: usize,
    pub range: TimeRange,
}
pub(super) fn build(b: &mut Builder, p: Panels<'_>) -> TableProjection {
    let Panels {
        session,
        analysis,
        timeline,
        inspection,
        panel,
        scroll,
        gpu_scroll,
        previous,
        width,
        height,
        top,
    } = p;
    b.block(
        Rect::new(12.0, top, width - 24.0, 1.0),
        Color::rgb8(59, 71, 91),
    );
    let mut x = 20.0;
    for (n, (value, title)) in Panel::ALL.into_iter().enumerate() {
        let w = match value {
            Panel::Gpu if width < 1100.0 => 46.0,
            Panel::Memory if width < 1100.0 => 76.0,
            Panel::Counters if width < 1100.0 => 76.0,
            Panel::Events if width < 1100.0 => 94.0,
            Panel::Tree if width < 1100.0 => 78.0,
            _ if width < 1100.0 => 64.0,
            Panel::Events => 106.0,
            Panel::Tree => 90.0,
            _ => 76.0,
        };
        b.control(
            100 + n as u64 * 2,
            Rect::new(x, top + 9.0, w, 28.0),
            title,
            Command::Panel(value),
        );
        if value == panel {
            b.block(Rect::new(x, top + 38.0, w, 2.0), ACCENT);
        }
        x += w + 6.0;
    }
    if previous {
        b.control(
            120,
            Rect::new(width - 202.0, top + 9.0, 80.0, 28.0),
            "Prev page",
            Command::PagePrevious,
        );
    }
    if let Some(cursor) = inspection
        .and_then(Inspection::page)
        .and_then(|p| p.next_cursor.clone())
    {
        b.control(
            122,
            Rect::new(width - 112.0, top + 9.0, 92.0, 28.0),
            "Next page",
            Command::PageNext(cursor),
        );
    }
    let intent = timeline.analysis_range();
    let range = analysis.map_or(intent, |analysis| analysis.range);
    let preview = if range != intent {
        "Release to analyze · statistics for "
    } else {
        ""
    };
    let timer = inspection.and_then(|i| i.timer.as_ref());
    let label = if panel == Panel::Frames {
        "whole CPU frames / associated GPU work".into()
    } else if panel == Panel::Gpu {
        if session.gpu_counts.mapped != 0 {
            "estimated GPU execution / unplaced records included".into()
        } else {
            "CPU recording contexts / original device durations".into()
        }
    } else {
        timer.map_or_else(
            || "Select a recorded scope group".into(),
            |t| format!("{} / {}", t.category, t.name),
        )
    };
    if panel != Panel::Memory {
        b.label(
            Rect::new(
                20.0,
                top + 46.0,
                width - if panel == Panel::Frames { 280.0 } else { 40.0 },
                22.0,
            ),
            format!(
                "{preview}{}–{} ms · {} ms range · {label}",
                ms(range.start),
                ms(range.end),
                ms(range.duration())
            ),
            12.0,
            MUTED,
        );
    }
    let table_width = (width * 0.66).max(580.0);
    let body = Rect::new(
        18.0,
        top + 98.0,
        table_width - 18.0,
        (height - top - 145.0).max(25.0),
    );
    let data = inspection.and_then(|i| i.table.as_ref());
    let total = match panel {
        Panel::Timers => analysis.map_or(0, |a| a.timers.len()),
        Panel::Diagnostics => 6 + session.diagnostics.len(),
        _ => match data {
            Some(Data::Relations { items, .. }) => items.len(),
            Some(Data::GpuSubmissions { items, .. }) => items.len(),
            Some(Data::GpuExecution { items, .. }) => items.len(),
            Some(Data::FrameTimings { items, .. }) => items.len(),
            Some(Data::MemoryAllocations { items, .. }) => items.len(),
            Some(
                Data::Events { items, .. }
                | Data::Tree { items, .. }
                | Data::Counters { items, .. },
            ) => items.len(),
            _ => 0,
        },
    };
    let row_height = if panel == Panel::Diagnostics {
        44.0
    } else {
        25.0
    };
    let visible = (body.height / row_height).floor() as usize;
    b.add(
        Widget::from_element(
            Element::container(TABLE, ElementKey::new("trace.statistics"))
                .semantic(Semantic::new(
                    SemanticRole::Group,
                    "Recorded statistics; use Page, Home, End or arrow keys to scroll",
                ))
                .styled(
                    geometry(body)
                        .focusable(true)
                        .focus_indicator(vesta_gpui::FocusIndicator::Control),
                ),
        )
        .on_click(Command::Table),
    );
    let first = scroll.min(total.saturating_sub(visible.max(1)));
    let row_y = |index: usize| body.y + (index - first) as f32 * row_height;
    match panel {
        Panel::Memory => {
            memory::modes(
                b,
                inspection.and_then(|i| i.memory.as_ref()),
                top + 46.0,
                table_width,
            );
            b.label(
                Rect::new(24.0, top + 75.0, table_width - 40.0, 21.0),
                "ALLOCATION / BYTES / LIFETIME / ALLOCATION THREAD",
                11.0,
                MUTED,
            );
            if let Some(Data::MemoryAllocations { items, .. }) = data {
                memory::rows(
                    b,
                    session,
                    items,
                    inspection
                        .and_then(|i| i.memory.as_ref())
                        .and_then(|m| m.allocation.as_ref()),
                    body,
                    first,
                    visible,
                );
            }
        }
        Panel::Frames => {
            for (x, w, title) in [
                (24.0, table_width - 388.0, "FRAME / THREAD / START ms"),
                (table_width - 360.0, 104.0, "CPU ms"),
                (table_width - 240.0, 104.0, "GPU WORK ms"),
                (table_width - 120.0, 110.0, "SAMPLES"),
            ] {
                b.label(Rect::new(x, top + 75.0, w, 21.0), title, 11.0, MUTED);
            }
            if let Some(Data::FrameTimings { items, sort, .. }) = data {
                frames::order(b, width, top, *sort);
                frames::rows(
                    b,
                    session,
                    items,
                    inspection
                        .and_then(|i| i.detail.as_ref())
                        .and_then(|d| d.item.frame),
                    body,
                    table_width,
                    first,
                    visible,
                );
            }
        }
        Panel::Gpu => {
            b.label(
                Rect::new(24.0, top + 75.0, table_width - 32.0, 21.0),
                if session.gpu_counts.mapped != 0 {
                    "SUBMISSION / ESTIMATED EXECUTION / DEVICE DURATION"
                } else {
                    "SUBMISSION / CPU CONTEXT (ms) / GPU DURATION (ms)"
                },
                11.0,
                MUTED,
            );
            if let Some(Data::GpuSubmissions { items, .. } | Data::GpuExecution { items, .. }) =
                data
            {
                gpu::rows(
                    b,
                    session,
                    items,
                    inspection.and_then(|i| i.gpu.as_ref()),
                    body,
                    table_width,
                    first,
                    visible,
                );
            }
        }
        Panel::Timers | Panel::Callers | Panel::Callees => {
            timer_header(b, top + 75.0, table_width);
            if panel == Panel::Timers {
                if let Some(analysis) = analysis {
                    for (index, t) in analysis.timers.iter().enumerate().skip(first).take(visible) {
                        timer_row(
                            b,
                            session,
                            table_width,
                            row_y(index),
                            TimerRow {
                                category: &t.category,
                                name: &t.name,
                                calls: t.calls,
                                inclusive: t.inclusive,
                                own: t.self_time,
                            },
                            timer,
                        );
                    }
                }
            } else if let Some(Data::Relations { items, .. }) = data {
                for (index, t) in items.iter().enumerate().skip(first).take(visible) {
                    relation_row(b, session, table_width, row_y(index), t, timer);
                }
            }
        }
        Panel::Events | Panel::Tree | Panel::Counters => {
            b.label(
                Rect::new(24.0, top + 75.0, table_width - 290.0, 21.0),
                "RECORDED EVENT / PARENT",
                11.0,
                MUTED,
            );
            b.label(
                Rect::new(table_width - 260.0, top + 75.0, 165.0, 21.0),
                "START / ELAPSED ms",
                11.0,
                MUTED,
            );
            b.label(
                Rect::new(table_width - 85.0, top + 75.0, 90.0, 21.0),
                if panel == Panel::Counters {
                    "VALUE"
                } else {
                    "EVENT ID"
                },
                11.0,
                MUTED,
            );
            if let Some(
                Data::Events { items, .. }
                | Data::Tree { items, .. }
                | Data::Counters { items, .. },
            ) = data
            {
                for (index, event) in items.iter().enumerate().skip(first).take(visible) {
                    event_row(
                        b,
                        session,
                        table_width,
                        row_y(index),
                        event,
                        timeline.selected,
                        panel == Panel::Counters,
                    );
                }
            }
        }
        Panel::Diagnostics => {
            b.label(
                Rect::new(24.0, top + 75.0, table_width - 32.0, 21.0),
                "CAPTURE QUALITY AND COVERAGE",
                11.0,
                MUTED,
            );
            let lines = [
                format!("Source SHA256: {}", session.source.sha256),
                format!(
                    "{} bytes · {:?} · schema {} · detail {}",
                    session.source.bytes,
                    session.source.format,
                    session.source.producer_schema_version,
                    session.detail
                ),
                format!(
                    "Clock: {} · {} · thread IDs {}",
                    session.source.clock,
                    session.source.analysis_time_unit,
                    session.source.thread_ids
                ),
                format!(
                    "Dropped {} · clipped {} · unframed {} · expired {}",
                    session.dropped_events,
                    session.unfinished_spans,
                    session.unframed_events,
                    session.capture_expired
                ),
                format!(
                    "Recorded hierarchy reliable: {}. Parent links do not imply cross-thread causality.",
                    session.hierarchy_reliable
                ),
                format!(
                    "GPU extension: {} · CPU/device clocks uncalibrated. Full-function coverage and symbols are unavailable.",
                    session.gpu.is_some()
                ),
            ];
            for index in first..(first + visible).min(total) {
                let text = if index < lines.len() {
                    lines[index].clone()
                } else {
                    let d = &session.diagnostics[index - lines.len()];
                    format!("{}: {}", d.code, d.message)
                };
                paragraph(
                    b,
                    Rect::new(24.0, row_y(index), table_width - 32.0, 42.0),
                    text,
                    if index >= lines.len() { GOLD } else { TEXT },
                );
            }
        }
    }
    if total == 0 {
        b.label(
            Rect::new(24.0, body.y + 5.0, body.width - 12.0, 42.0),
            if panel == Panel::Tree && timeline.selected.is_none() {
                "Select a frame or event to inspect its recorded tree."
            } else {
                "No matching recorded rows in this range."
            },
            13.0,
            MUTED,
        );
    }
    let page = inspection.and_then(Inspection::page);
    b.label(
        Rect::new(20.0, height - 61.0, table_width - 30.0, 22.0),
        page.map_or_else(
            || {
                format!(
                    "{} rows · scroll to inspect · inclusive sums overlap",
                    total
                )
            },
            |p| {
                if panel == Panel::Frames {
                    format!(
                        "{} / {} rows · GPU work may overlap · {} execution estimates",
                        p.returned, p.total, session.gpu_counts.mapped
                    )
                } else {
                    format!(
                        "{} rows in page / {} total · more {} · recorded evidence only",
                        p.returned, p.total, p.has_more
                    )
                }
            },
        ),
        11.0,
        MUTED,
    );
    if panel == Panel::Frames {
        if let Some(data) = data {
            frames::detail(
                b,
                data,
                inspection
                    .and_then(|i| i.detail.as_ref())
                    .and_then(|d| d.item.frame),
                Rect::new(
                    table_width + 22.0,
                    top + 75.0,
                    width - table_width - 42.0,
                    (height - 45.0 - top - 75.0).max(0.0),
                ),
            );
        }
    } else if panel == Panel::Memory {
        memory::detail(
            b,
            inspection.and_then(|i| i.memory.as_ref()),
            gpu_scroll,
            Rect::new(
                table_width + 22.0,
                top + 75.0,
                width - table_width - 42.0,
                (height - 45.0 - top - 75.0).max(0.0),
            ),
        );
    } else if panel == Panel::Gpu {
        gpu::detail(
            b,
            session,
            inspection.and_then(|i| i.gpu.as_ref()),
            gpu_scroll,
            Rect::new(
                table_width + 22.0,
                top + 75.0,
                width - table_width - 42.0,
                (height - 45.0 - top - 75.0).max(0.0),
            ),
        );
    } else {
        detail(
            b,
            inspection,
            analysis,
            timeline,
            Rect::new(
                table_width + 22.0,
                top + 75.0,
                width - table_width - 42.0,
                (height - 45.0 - top - 75.0).max(0.0),
            ),
        );
    }
    TableProjection {
        area: body,
        rows: total,
        page_rows: visible.max(1),
        range,
    }
}
fn timer_header(b: &mut Builder, y: f32, w: f32) {
    for (x, width, label) in [
        (24.0, w - 290.0, "RECORDED SCOPE"),
        (w - 260.0, 70.0, "CALLS"),
        (w - 180.0, 90.0, "INCLUSIVE"),
        (w - 85.0, 90.0, "SELF"),
    ] {
        b.label(Rect::new(x, y, width, 21.0), label, 11.0, MUTED);
    }
}
struct TimerRow<'a> {
    category: &'a str,
    name: &'a str,
    calls: usize,
    inclusive: u64,
    own: u64,
}
fn timer_row(
    b: &mut Builder,
    session: &Session,
    w: f32,
    y: f32,
    timer: TimerRow<'_>,
    selected: Option<&crate::query::protocol::TimerKey>,
) {
    let TimerRow {
        category,
        name,
        calls,
        inclusive,
        own,
    } = timer;
    let anchor = session
        .timer_anchors
        .get(&(category.to_owned(), name.to_owned()))
        .copied();
    if let Some(anchor) = anchor {
        b.target(
            6_000_000 + anchor as u64,
            Rect::new(18.0, y, w - 18.0, 23.0),
            &format!(
                "{category} / {name}; {calls} calls; inclusive {} ms; self {} ms",
                ms(inclusive),
                ms(own)
            ),
            Color::rgb8(24, 31, 41),
            selected.is_some_and(|t| t.category == category && t.name == name),
            Command::Timer(category.into(), name.into()),
        );
    }
    for (x, width, text, color) in [
        (24.0, w - 290.0, format!("{category} / {name}"), TEXT),
        (w - 260.0, 70.0, calls.to_string(), MUTED),
        (w - 180.0, 90.0, format!("{} ms", ms(inclusive)), ACCENT),
        (w - 85.0, 90.0, format!("{} ms", ms(own)), TEXT),
    ] {
        b.label(Rect::new(x, y + 2.0, width, 20.0), text, 12.0, color);
    }
}
fn relation_row(
    b: &mut Builder,
    s: &Session,
    w: f32,
    y: f32,
    t: &TimerRecord,
    selected: Option<&crate::query::protocol::TimerKey>,
) {
    timer_row(
        b,
        s,
        w,
        y,
        TimerRow {
            category: &t.timer.category,
            name: &t.timer.name,
            calls: t.calls,
            inclusive: t.inclusive_ns,
            own: t.self_ns,
        },
        selected,
    );
}
fn event_row(
    b: &mut Builder,
    session: &Session,
    w: f32,
    y: f32,
    e: &EventRecord,
    selected: Option<EventKey>,
    counter: bool,
) {
    if let Some(index) = session.indices.get(&e.key) {
        b.target(
            5_000_000 + *index as u64,
            Rect::new(18.0, y, w - 18.0, 23.0),
            &format!(
                "{} / {}; event {}; {} ms",
                e.category,
                e.name,
                e.key.event,
                ms(e.duration_ns)
            ),
            Color::rgb8(24, 31, 41),
            selected == Some(e.key),
            Command::Scope(e.key),
        );
    }
    let indent = if e.parent.is_some() {
        format!("{}↳ ", "  ".repeat(e.depth.min(12)))
    } else {
        String::new()
    };
    b.label(
        Rect::new(24.0, y + 2.0, w - 290.0, 20.0),
        format!("{indent}{} / {}", e.category, e.name),
        12.0,
        TEXT,
    );
    b.label(
        Rect::new(w - 260.0, y + 2.0, 165.0, 20.0),
        format!("{} / {}", ms(e.start_ns), ms(e.duration_ns)),
        12.0,
        MUTED,
    );
    b.label(
        Rect::new(w - 85.0, y + 2.0, 90.0, 20.0),
        if counter {
            e.counter_value
                .map_or_else(|| "—".into(), |v| v.to_string())
        } else {
            e.key.event.to_string()
        },
        12.0,
        ACCENT,
    );
}
pub(super) fn paragraph(b: &mut Builder, rect: Rect, text: String, color: Color) {
    let text = preview(text);
    let value = b.next;
    b.next += 1;
    b.add(Widget::from_element(
        Element::text(
            id(value),
            ElementKey::new(format!("trace.label.{value}")),
            text.clone(),
        )
        .semantic(Semantic::new(SemanticRole::Label, text))
        .styled(
            geometry(rect)
                .text(12.0, color)
                .text_wrap(true)
                .pointer_events(false),
        ),
    ));
}
fn detail(
    b: &mut Builder,
    inspection: Option<&Inspection>,
    analysis: Option<&Analysis>,
    timeline: &Timeline,
    rect: Rect,
) {
    let (x, y, w, bottom) = (rect.x, rect.y, rect.width, rect.bottom());
    b.label(
        Rect::new(x, y, w, 21.0),
        "RECORDED FRAME ATTEMPTS",
        11.0,
        MUTED,
    );
    if let Some(a) = analysis {
        b.label(
            Rect::new(x, y + 25.0, w, 21.0),
            if a.frames.is_empty() {
                "No frame attempts match this filter / range.".into()
            } else {
                format!(
                    "P50 {}  P95 {}  P99 {} ms",
                    ms(a.percentile_ns[0]),
                    ms(a.percentile_ns[1]),
                    ms(a.percentile_ns[2])
                )
            },
            12.0,
            TEXT,
        );
    }
    if let Some(detail) = inspection.and_then(|i| i.detail.as_ref()) {
        let e = &detail.item;
        b.label(Rect::new(x, y + 55.0, w, 22.0), &e.name, 13.0, GOLD);
        b.label(
            Rect::new(x, y + 80.0, w, 21.0),
            format!(
                "P{} / T{} / Event {}",
                e.key.process, e.lane.thread, e.key.event
            ),
            11.0,
            MUTED,
        );
        b.label(
            Rect::new(x, y + 104.0, w, 21.0),
            if let Some(value) = e.counter_value {
                format!("Observed value {value} · at {} ms", ms(e.start_ns))
            } else {
                format!(
                    "Whole {} ms · self {} ms · {} children",
                    ms(e.duration_ns),
                    detail.self_ns.map_or_else(|| "—".into(), ms),
                    detail.children
                )
            },
            11.0,
            TEXT,
        );
        if y + 158.0 < bottom {
            if let Some(parent) = e.parent {
                b.control(
                    130,
                    Rect::new(x, y + 134.0, w * 0.48, 27.0),
                    "Parent scope",
                    Command::Scope(parent),
                );
            }
            if let Some(frame) = e.frame {
                b.control(
                    132,
                    Rect::new(x + w * 0.51, y + 134.0, w * 0.49, 27.0),
                    "Owning frame",
                    Command::Frame(frame),
                );
            }
        }
    } else {
        b.label(
            Rect::new(x, y + 60.0, w, 60.0),
            if timeline.selected.is_some() {
                "Loading selected evidence…"
            } else {
                "Select an event to follow its stable parent / frame keys."
            },
            12.0,
            MUTED,
        );
    }
}
