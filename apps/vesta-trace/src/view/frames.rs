//! Native presentation of the shared frame-timing query result.
use super::*;
use crate::query::protocol::{Data, FrameTimingRecord, FrameTimingSort};

pub(super) fn order(b: &mut Builder, width: f32, top: f32, sort: FrameTimingSort) {
    for (n, (title, value)) in [
        ("CPU ↓", FrameTimingSort::CpuDurationDesc),
        ("GPU ↓", FrameTimingSort::GpuWorkDesc),
        ("Time ↑", FrameTimingSort::StartAsc),
    ]
    .into_iter()
    .enumerate()
    {
        let x = width - 244.0 + n as f32 * 76.0;
        b.control(
            144 + n as u64 * 2,
            Rect::new(x, top + 43.0, 70.0, 26.0),
            title,
            Command::FrameOrder(value),
        );
        if sort == value {
            b.block(Rect::new(x, top + 70.0, 70.0, 1.0), ACCENT);
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn rows(
    b: &mut Builder,
    session: &Session,
    items: &[FrameTimingRecord],
    selected: Option<EventKey>,
    body: Rect,
    width: f32,
    first: usize,
    visible: usize,
) {
    let cpu_max = items
        .iter()
        .map(|r| r.frame.duration_ns)
        .max()
        .unwrap_or(1)
        .max(1);
    let gpu_max = items
        .iter()
        .filter_map(|r| r.gpu.sampled_work_ns)
        .max()
        .unwrap_or(1)
        .max(1);
    for (n, row) in items.iter().enumerate().skip(first).take(visible) {
        let y = body.y + (n - first) as f32 * 25.0;
        let cpu = &row.frame;
        let gpu = &row.gpu;
        let Some(index) = session.indices.get(&cpu.key) else {
            continue;
        };
        let title = format!(
            "Frame {} · T{} · {} ms CPU · {} ms GPU sampled work · {}/{} sampled submissions",
            cpu.key.event,
            cpu.lane.thread,
            ms(cpu.duration_ns),
            gpu.sampled_work_ns.map_or_else(|| "—".into(), ms),
            gpu.samples,
            gpu.records
        );
        b.target(
            30_000_000 + *index as u64,
            Rect::new(18.0, y, width - 18.0, 23.0),
            &title,
            Color::rgb8(24, 31, 41),
            selected == Some(cpu.key),
            Command::Frame(cpu.key),
        );
        b.label(
            Rect::new(24.0, y + 1.0, width - 388.0, 19.0),
            format!(
                "{} · T{} · #{} · {} ms",
                cpu.name,
                cpu.lane.thread,
                cpu.key.event,
                ms(cpu.start_ns)
            ),
            11.0,
            TEXT,
        );
        b.label(
            Rect::new(width - 360.0, y + 1.0, 104.0, 19.0),
            ms(cpu.duration_ns),
            11.0,
            ACCENT,
        );
        b.label(
            Rect::new(width - 240.0, y + 1.0, 104.0, 19.0),
            gpu.sampled_work_ns.map_or_else(|| "—".into(), ms),
            11.0,
            GOLD,
        );
        b.label(
            Rect::new(width - 120.0, y + 1.0, 110.0, 19.0),
            format!(
                "{}/{}{}",
                gpu.samples,
                gpu.records,
                if gpu.all_recorded_submissions_sampled {
                    ""
                } else {
                    " *"
                }
            ),
            11.0,
            MUTED,
        );
        b.block(
            Rect::new(
                width - 360.0,
                y + 21.0,
                96.0 * cpu.duration_ns as f32 / cpu_max as f32,
                1.0,
            ),
            ACCENT,
        );
        if let Some(value) = gpu.sampled_work_ns {
            b.block(
                Rect::new(
                    width - 240.0,
                    y + 21.0,
                    96.0 * value as f32 / gpu_max as f32,
                    1.0,
                ),
                GOLD,
            );
        }
    }
}

pub(super) fn detail(b: &mut Builder, data: &Data, selected: Option<EventKey>, rect: Rect) {
    let Data::FrameTimings {
        items,
        statistics: stats,
        gpu_attached,
        source_unframed_gpu_records,
        ..
    } = data
    else {
        return;
    };
    let (x, y, w) = (rect.x, rect.y, rect.width);
    b.label(
        Rect::new(x, y, w, 21.0),
        "CPU / GPU FRAME EVIDENCE",
        11.0,
        MUTED,
    );
    b.label(
        Rect::new(x, y + 24.0, w, 21.0),
        format!(
            "{} frames · CPU P95 {} ms",
            stats.cpu.count,
            stats.cpu.p95_ns.map_or_else(|| "—".into(), ms)
        ),
        12.0,
        TEXT,
    );
    b.label(
        Rect::new(x, y + 48.0, w, 21.0),
        if *gpu_attached {
            format!(
                "GPU: {} sampled · {} pending · {} unavailable",
                stats.gpu_samples, stats.gpu_pending, stats.gpu_unavailable
            )
        } else {
            "GPU capture is not attached".into()
        },
        11.0,
        GOLD,
    );
    if let Some(row) = items
        .iter()
        .find(|r| Some(r.frame.key) == selected)
        .or_else(|| items.first())
    {
        b.label(
            Rect::new(x, y + 77.0, w, 21.0),
            format!(
                "Frame #{} · CPU {} ms{}",
                row.frame.key.event,
                ms(row.frame.duration_ns),
                if row.frame.unfinished {
                    " · clipped"
                } else {
                    ""
                }
            ),
            12.0,
            TEXT,
        );
        b.label(
            Rect::new(x, y + 101.0, w, 21.0),
            format!(
                "GPU work {} ms · {}/{} samples{}",
                row.gpu.sampled_work_ns.map_or_else(|| "—".into(), ms),
                row.gpu.samples,
                row.gpu.records,
                if row.gpu.all_recorded_submissions_sampled {
                    ""
                } else {
                    " · incomplete"
                }
            ),
            11.0,
            GOLD,
        );
        if y + 159.0 < rect.bottom()
            && let Some(key) = row.gpu.longest_submission
        {
            b.control(
                152,
                Rect::new(x, y + 131.0, w, 27.0),
                "Inspect longest GPU submission",
                Command::InspectGpu(key),
            );
        }
    }
    if y + 226.0 < rect.bottom() {
        panels::paragraph(b,Rect::new(x,y+170.0,w,54.0),
            "GPU work sums whole sampled submissions associated with the CPU frame. Overlap is possible. Inspect a submission for available clock calibration and execution estimates.".into(),MUTED);
    }
    if y + 272.0 < rect.bottom() {
        panels::paragraph(
            b,
            Rect::new(x, y + 230.0, w, 42.0),
            format!(
                "Source GPU loss: {} · unframed records: {}. No records means unknown timing.",
                stats.source_dropped_gpu_records, source_unframed_gpu_records
            ),
            MUTED,
        );
    }
}
