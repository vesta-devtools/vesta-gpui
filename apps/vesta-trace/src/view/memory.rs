//! Bounded heap workspace projections; all lifecycle analysis runs on the worker.
use super::*;
use crate::{
    inspection::{MemoryInspection, MemoryMode},
    query::protocol::MemoryAllocationRecord,
};

fn bytes(value: u64) -> String {
    if value >= 1024 * 1024 {
        format!("{:.2} MiB", value as f64 / (1024.0 * 1024.0))
    } else if value >= 1024 {
        format!("{:.2} KiB", value as f64 / 1024.0)
    } else {
        format!("{value} B")
    }
}
pub(super) fn track(
    b: &mut Builder,
    memory: Option<&MemoryInspection>,
    timeline: &Timeline,
    rect: Rect,
) {
    let Some(m) = memory else {
        return;
    };
    b.label(
        Rect::new(20.0, rect.y + 6.0, 160.0, 20.0),
        "OBSERVED RUST HEAP",
        11.0,
        ACCENT,
    );
    let Some(c) = &m.coverage else {
        b.label(
            Rect::new(rect.x + 12.0, rect.y + 12.0, rect.width - 24.0, 40.0),
            &m.message,
            13.0,
            GOLD,
        );
        return;
    };
    let Some(stats) = &m.statistics else {
        return;
    };
    let half = (rect.height - 40.0).max(20.0) * 0.5;
    let bytes_bottom = rect.y + half + 15.0;
    let count_bottom = rect.bottom() - 8.0;
    for (y, title) in [
        (
            rect.y + 32.0,
            format!("Peak {}", bytes(stats.peak_observed_live_bytes)),
        ),
        (
            rect.y + 55.0,
            format!("End {}", bytes(stats.observed_live_bytes_at_end)),
        ),
        (
            rect.y + half + 32.0,
            format!("Peak {} blocks", stats.peak_observed_live_count),
        ),
    ] {
        b.label(Rect::new(20.0, y, 160.0, 20.0), title, 11.0, MUTED);
    }
    for p in &m.points {
        if !timeline.view.contains(p.time_ns) {
            continue;
        }
        let x = rect.x + (timeline.fraction(p.time_ns) * f64::from(rect.width)) as f32;
        let h = (p.observed_live_bytes as f64 / stats.peak_observed_live_bytes.max(1) as f64
            * f64::from(half - 15.0)) as f32;
        b.block(Rect::new(x, bytes_bottom - h, 2.0, h.max(1.0)), ACCENT);
        let h = (p.observed_live_count as f64 / stats.peak_observed_live_count.max(1) as f64
            * f64::from(half - 15.0)) as f32;
        b.block(
            Rect::new(x, count_bottom - h, 2.0, h.max(1.0)),
            Color::rgb8(126, 150, 228),
        );
    }
    if c.through_ns < timeline.view.end.saturating_sub(1) {
        let start = timeline
            .fraction(c.through_ns.saturating_add(1))
            .clamp(0.0, 1.0) as f32;
        b.block(
            Rect::new(
                rect.x + start * rect.width,
                rect.y,
                rect.width * (1.0 - start),
                rect.height,
            ),
            Color::srgba(0.45, 0.27, 0.12, 0.35),
        );
        b.label(
            Rect::new(
                rect.x + start * rect.width + 8.0,
                rect.y + 10.0,
                (rect.width * (1.0 - start) - 16.0).max(0.0),
                22.0,
            ),
            "Heap coverage ended · later values unknown",
            11.0,
            GOLD,
        );
    }
    b.label(Rect::new(rect.x+8.0,rect.y+2.0,rect.width-16.0,18.0),"Bytes / block count · actual post-operation samples with extrema retained · drag an A–B interval",11.0,MUTED);
}
pub(super) fn modes(b: &mut Builder, memory: Option<&MemoryInspection>, y: f32, width: f32) {
    let selected = memory.map(|m| m.mode).unwrap_or_default();
    let w = ((width - 30.0) / 6.0 - 5.0).min(112.0);
    for (n, (mode, title)) in MemoryMode::ALL.into_iter().enumerate() {
        let x = 20.0 + n as f32 * (w + 5.0);
        b.control(
            2000 + n as u64 * 2,
            Rect::new(x, y, w, 24.0),
            title,
            Command::MemoryMode(mode),
        );
        if mode == selected {
            b.block(Rect::new(x, y + 25.0, w, 2.0), ACCENT);
        }
    }
}
pub(super) fn rows(
    b: &mut Builder,
    session: &Session,
    items: &[MemoryAllocationRecord],
    selected: Option<&MemoryAllocationRecord>,
    body: Rect,
    first: usize,
    visible: usize,
) {
    let Some(m) = &session.memory else {
        return;
    };
    for (n, a) in items.iter().enumerate().skip(first).take(visible) {
        let y = body.y + (n - first) as f32 * 25.0;
        let Some(index) = m.indices.get(&a.key.allocation) else {
            continue;
        };
        let name = session
            .thread_names
            .get(&a.allocation_lane)
            .and_then(|n| n.as_deref())
            .unwrap_or("unnamed");
        let title = format!(
            "#{} {} · {} · {}–{} ms · T{} {}",
            a.key.allocation,
            a.address,
            bytes(a.size_bytes),
            ms(a.allocated_ns),
            a.freed_ns.map_or_else(|| "live".into(), ms),
            a.allocation_lane.thread,
            name
        );
        b.selectable(
            30_000_000 + *index as u64,
            Rect::new(body.x, y, body.width, 23.0),
            &title,
            Color::rgb8(24, 31, 41),
            selected.is_some_and(|s| s.key == a.key),
            Command::MemoryAllocation(a.key),
        );
    }
}
pub(super) fn detail(
    b: &mut Builder,
    memory: Option<&MemoryInspection>,
    offset: vesta_gpui::Point,
    rect: Rect,
) {
    let Some(m) = memory else {
        return;
    };
    let mut lines = vec![
        "MEMORY INSIGHTS · OBSERVED LIFETIMES".into(),
        m.message.clone(),
    ];
    if let Some(c) = &m.coverage {
        lines.push(format!(
            "Coverage through {} ms · {} operations · limit {} · hook errors {}",
            ms(c.through_ns),
            c.operations,
            c.event_limit_reached,
            c.hook_errors
        ));
        lines.push(format!(
            "Stack frames: {} named / {} unresolved",
            c.resolved_frames, c.unresolved_frames
        ));
        lines.push(format!("Stacks on {} / {} allocations · {} potentially truncated · {} stack observations omitted by budget · symbol budget {}",
            c.allocations_with_stacks,c.allocation_records,c.potentially_truncated_stacks,c.stack_observations_dropped,c.symbol_budget_exhausted));
    }
    if let Some(c) = &m.comparison {
        lines.push(format!(
            "A {} / {} blocks → B {} / {} blocks",
            bytes(c.observed_live_at_start.bytes),
            c.observed_live_at_start.allocations,
            bytes(c.observed_live_at_end.bytes),
            c.observed_live_at_end.allocations
        ));
        lines.push(format!(
            "New at B {} · released {} · common {} · transient {}",
            bytes(c.new_survivors.bytes),
            bytes(c.released.bytes),
            bytes(c.common.bytes),
            bytes(c.transient.bytes)
        ));
    }
    lines.push("Pre-capture and recorder-excluded allocations, native/driver and GPU heaps are outside this baseline. Survivors alone do not prove leaks.".into());
    if let Some(a) = &m.allocation {
        lines.push(format!(
            "ALLOCATION {}:{} · {}",
            a.key.process, a.key.allocation, a.address
        ));
        lines.push(format!(
            "{} bytes · allocated {} ns · freed {} · observed lifetime {} ns",
            a.size_bytes,
            a.allocated_ns,
            a.freed_ns
                .map_or_else(|| "not observed".into(), |t| format!("{t} ns")),
            a.observed_lifetime_ns
        ));
        lines.push(format!(
            "Allocation thread {} · free thread {:?} · end operation {:?}",
            a.allocation_lane.thread,
            a.free_lane.map(|l| l.thread),
            a.end_operation
        ));
        if let Some(prior) = a.reallocated_from {
            lines.push(format!(
                "Reallocated from original allocation {}",
                prior.allocation
            ));
        }
        if a.unknown_prior_reallocation {
            lines.push(
                "Reallocated a block whose original allocation was outside recording coverage."
                    .into(),
            );
        }
        if let Some(stack) = &m.stack {
            lines.push(format!(
                "ALLOCATION STACK {} · potentially truncated {}",
                stack.key.stack, stack.potentially_truncated
            ));
            for (n, f) in stack.frames.iter().enumerate() {
                lines.push(format!(
                    "{n:02} {} · {}",
                    f.address,
                    f.function.as_deref().unwrap_or("unresolved")
                ));
                if let Some(file) = &f.file {
                    lines.push(format!("{file}:{}", f.line.unwrap_or(0)));
                }
            }
        } else {
            lines.push("No allocation stack was recorded for this lifetime.".into());
        }
    } else {
        lines.push("Select an allocation to inspect its original lifetime, thread, realloc lineage and allocation stack.".into());
    }
    let mut content =
        Element::scroll_box(GPU_DETAIL, ElementKey::new("trace.memory.evidence"), offset)
            .semantic(Semantic::new(
                SemanticRole::ScrollView,
                "Memory allocation evidence and stack",
            ))
            .styled(
                geometry(rect)
                    .axis(vesta_gpui::Axis::Column)
                    .gap(5.0)
                    .focusable(true)
                    .focus_indicator(vesta_gpui::FocusIndicator::Control),
            );
    for (n, line) in lines.into_iter().enumerate() {
        content = content.child(
            Element::text(
                id(200 + n as u64),
                ElementKey::new(format!("trace.memory.fact.{n}")),
                line.clone(),
            )
            .semantic(Semantic::new(SemanticRole::Label, line))
            .styled(
                Style::default()
                    .size(Dimension::Px((rect.width - 18.0).max(1.0)), Dimension::Auto)
                    .flex(0.0, 0.0)
                    .text(11.0, TEXT)
                    .text_wrap(true)
                    .pointer_events(false),
            ),
        );
    }
    b.add(Widget::from_element(content));
}
