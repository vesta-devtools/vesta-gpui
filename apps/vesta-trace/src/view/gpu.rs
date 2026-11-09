//! GPU durations remain separate from the uncalibrated CPU timeline.
use super::*;
use crate::query::protocol::{GpuRecord, GpuStatus};

#[allow(clippy::too_many_arguments)]
pub(super) fn rows(
    b: &mut Builder,
    session: &Session,
    items: &[GpuRecord],
    selected: Option<&GpuRecord>,
    body: Rect,
    width: f32,
    first: usize,
    visible: usize,
) {
    let largest = items.iter().filter_map(|r| r.elapsed_ns).max().unwrap_or(0);
    for (n, r) in items.iter().enumerate().skip(first).take(visible) {
        let y = body.y + (n - first) as f32 * 25.0;
        let Some(index) = session
            .gpu_indices
            .get(&(r.key.generation, r.key.submission))
        else {
            continue;
        };
        let placement = r.execution.as_ref().map_or_else(
            || format!("unplaced · CPU context {} ms", ms(r.recorded_cpu_ns)),
            |e| {
                format!(
                    "est {:.3}–{:.3} ms",
                    e.estimated_start_ns as f64 / 1_000_000.0,
                    e.estimated_end_ns as f64 / 1_000_000.0
                )
            },
        );
        let label = format!(
            "GPU {}:{} · window {} / T{} · {} · GPU {} ms · {:?}",
            r.key.generation,
            r.key.submission,
            r.window,
            r.lane.thread,
            placement,
            r.elapsed_ns.map_or_else(|| "—".into(), ms),
            r.status
        );
        b.target(
            20_000_000 + *index as u64,
            Rect::new(18.0, y, width - 18.0, 23.0),
            &label,
            Color::rgb8(24, 31, 41),
            selected.is_some_and(|s| s.key == r.key),
            Command::Gpu(r.key),
        );
        if let Some(value) = r.elapsed_ns
            && largest != 0
        {
            b.block(
                Rect::new(
                    24.0,
                    y + 19.0,
                    (width - 48.0) * value as f32 / largest as f32,
                    2.0,
                ),
                ACCENT,
            );
        }
        b.label(
            Rect::new(24.0, y + 1.0, width - 42.0, 19.0),
            label,
            11.0,
            if matches!(r.status, GpuStatus::Sample) {
                TEXT
            } else {
                GOLD
            },
        );
    }
}

pub(super) fn detail(
    b: &mut Builder,
    session: &Session,
    record: Option<&GpuRecord>,
    offset: vesta_gpui::Point,
    rect: Rect,
) {
    let (x, y, w) = (rect.x, rect.y, rect.width);
    b.label(
        Rect::new(x, y, w, 21.0),
        "GPU SUBMISSION EVIDENCE",
        11.0,
        MUTED,
    );
    b.label(
        Rect::new(x, y + 23.0, w, 21.0),
        if record.is_some_and(|r| r.execution.is_some()) {
            "CPU / GPU: calibrated estimate"
        } else {
            "CPU / GPU clocks: uncalibrated"
        },
        11.0,
        GOLD,
    );
    let Some(r) = record else {
        b.label(
            Rect::new(x, y + 49.0, w, 48.0),
            if session.gpu.is_some() {
                "Select a submission to inspect its raw timestamp evidence."
            } else {
                "This CPU-only input has no GPU attachment."
            },
            12.0,
            TEXT,
        );
        return;
    };
    let clock = session.gpu.as_ref().and_then(|g| {
        g.clocks
            .binary_search_by_key(&r.key.generation, |c| c.generation)
            .ok()
            .map(|i| &g.clocks[i])
    });
    let mut lines = vec![
        format!(
            "Process {} / generation {}",
            r.key.process, r.key.generation
        ),
        format!(
            "Submission {} / window {} / T{}",
            r.key.submission, r.window, r.lane.thread
        ),
        format!("State: {:?}", r.status),
        format!(
            "GPU elapsed: {} ns",
            r.elapsed_ns
                .map_or_else(|| "unavailable".into(), |v| v.to_string())
        ),
        format!("CPU recording context: {} ns", r.recorded_cpu_ns),
        format!(
            "Start tick: {}",
            r.raw_ticks
                .as_ref()
                .map_or_else(|| "unavailable".into(), |t| t[0].clone())
        ),
        format!(
            "End tick: {}",
            r.raw_ticks
                .as_ref()
                .map_or_else(|| "unavailable".into(), |t| t[1].clone())
        ),
        format!(
            "Completion bound: {} ns",
            r.completion_upper_bound_ns
                .map_or_else(|| "unavailable".into(), |v| v.to_string())
        ),
    ];
    if let Some(e) = &r.execution {
        lines.extend([
            format!("Estimated execution: {}–{} ns (CPU clock)",e.estimated_start_ns,e.estimated_end_ns),
            format!("Anchor: {} ns; anchor uncertainty ±{} ns; distance {} ns",e.anchor_cpu_ns,e.anchor_uncertainty_ns,e.anchor_distance_ns),
            "Anchor uncertainty excludes extrapolation clock drift. Execution placement is an estimate, not a presentation measurement.".into(),
        ]);
    }
    if let Some(c) = &r.calibration {
        lines.extend([
            format!("Calibration device tick: {}", c.device_ticks),
            format!(
                "Calibrated QPC tick: {} / {} Hz",
                c.host_ticks, c.host_frequency_hz
            ),
            format!(
                "Host QPC bracket: {}–{}",
                c.host_before_ticks, c.host_after_ticks
            ),
            format!(
                "Capture CPU bracket: {}–{} ns",
                c.cpu_before_ns, c.cpu_after_ns
            ),
            format!("Driver maxDeviation: {} ns", c.max_deviation_ns),
        ]);
    }
    if let Some(error) = &r.calibration_error {
        lines.push(format!("Calibration unavailable: {error}"));
    }
    if let Some(c) = clock {
        lines.extend([
            c.device_name.clone(),
            format!("Queue family: {}", c.queue_family),
            format!("Timestamp valid bits: {:?}", c.valid_bits),
            format!("Timestamp period: {:?} ns", c.period_ns),
            format!("Vendor {} / device {}", c.vendor_id, c.device_id),
            format!("Driver version: {}", c.driver_version),
        ]);
    }
    if let Some(reason) = &r.unavailable {
        lines.push(format!("Unavailable: {reason}"));
    }
    lines.push("CPU context associates the recording with its host attempt. It is not GPU execution start or presentation latency.".into());
    let body = Rect::new(x, y + 49.0, w, (rect.height - 84.0).max(1.0));
    let mut content =
        Element::scroll_box(GPU_DETAIL, ElementKey::new("trace.gpu.evidence"), offset)
            .semantic(Semantic::new(
                SemanticRole::ScrollView,
                "GPU raw submission evidence",
            ))
            .styled(
                geometry(body)
                    .axis(vesta_gpui::Axis::Column)
                    .gap(5.0)
                    .focusable(true)
                    .focus_indicator(vesta_gpui::FocusIndicator::Control),
            );
    for (n, line) in lines.into_iter().enumerate() {
        content = content.child(
            Element::text(
                id(200 + n as u64),
                ElementKey::new(format!("trace.gpu.fact.{n}")),
                line.clone(),
            )
            .semantic(Semantic::new(SemanticRole::Label, line))
            .styled(
                Style::default()
                    .size(Dimension::Px((w - 18.0).max(1.0)), Dimension::Auto)
                    .flex(0.0, 0.0)
                    .text(11.0, TEXT)
                    .text_wrap(true)
                    .pointer_events(false),
            ),
        );
    }
    b.add(Widget::from_element(content));
    if let Some(frame) = r.frame
        && rect.height > 65.0
    {
        b.control(
            134,
            Rect::new(x, rect.bottom() - 30.0, w, 27.0),
            "Recorded CPU frame",
            Command::Frame(frame),
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn timeline(
    b: &mut Builder,
    session: &Session,
    timeline: &Timeline,
    tracks: &[crate::gpu::plot::Track],
    rect: Rect,
    top: f32,
    columns: usize,
    limit: usize,
    selected: Option<&GpuRecord>,
) -> (usize, usize, usize) {
    let Some(capture) = &session.gpu else {
        return (0, 0, 0);
    };
    let (mut drawn, mut collapsed, mut visible) = (0, 0, 0);
    for (row, track) in tracks.iter().enumerate() {
        let y = top + row as f32 * 48.0;
        if y >= rect.bottom() {
            break;
        }
        if y + 44.0 <= rect.y {
            continue;
        }
        if y >= rect.y && y + 20.0 <= rect.bottom() {
            b.label(
                Rect::new(20.0, y + 2.0, 160.0, 20.0),
                format!("GPU G{} / W{}", track.generation, track.window),
                11.0,
                Color::rgb8(178, 160, 237),
            );
            b.label(
                Rect::new(rect.x + 4.0, y + 2.0, rect.width - 8.0, 20.0),
                "Execution estimates · fresh device/QPC calibration · drift bound unknown",
                11.0,
                MUTED,
            );
        }
        let bar_top = (y + 23.0).max(rect.y);
        let bar_bottom = (y + 42.0).min(rect.bottom());
        if bar_top >= bar_bottom {
            continue;
        }
        for bucket in track.buckets(timeline.view, columns) {
            visible += bucket.count;
            if drawn >= limit {
                collapsed += bucket.count;
                continue;
            }
            collapsed += bucket.count - 1;
            drawn += 1;
            let record = &capture.submissions[bucket.record];
            let Some(e) = session
                .gpu_execution
                .get(&(record.generation, record.submission))
            else {
                continue;
            };
            let x = rect.x
                + (timeline.fraction(e.start_ns.max(0) as u64).clamp(0.0, 1.0)
                    * f64::from(rect.width)) as f32;
            let right = rect.x
                + (timeline.fraction(e.end_ns.max(0) as u64).clamp(0.0, 1.0)
                    * f64::from(rect.width)) as f32;
            let key = crate::query::protocol::GpuKey {
                process: u64::from(capture.process_id),
                generation: record.generation,
                submission: record.submission,
            };
            b.selectable(
                40_000_000 + bucket.record as u64,
                Rect::new(
                    x.min(rect.right() - 1.0),
                    bar_top,
                    (right - x).max(2.0).min((rect.right() - x).max(1.0)),
                    bar_bottom - bar_top,
                ),
                &format!(
                    "GPU #{} · estimated {}–{} ns · anchor ±{} ns; drift unbounded",
                    record.submission, e.start_ns, e.end_ns, e.anchor_uncertainty_ns
                ),
                Color::rgb8(93, 77, 146),
                selected.is_some_and(|s| s.key == key),
                Command::GpuExecution(key),
            );
        }
    }
    (drawn, collapsed, visible)
}
