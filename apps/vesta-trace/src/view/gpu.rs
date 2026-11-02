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
        let label = format!(
            "GPU {}:{} · window {} / T{} · CPU {} ms · GPU {} ms · {:?}",
            r.key.generation,
            r.key.submission,
            r.window,
            r.lane.thread,
            ms(r.recorded_cpu_ns),
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
        "CPU / GPU clocks: uncalibrated",
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
