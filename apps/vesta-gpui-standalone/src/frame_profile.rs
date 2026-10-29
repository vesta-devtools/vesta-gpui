//! Optional CPU wall-clock observations of completed host frames (ADR-0314).
//! Renderer includes acquire/prepare/submit/present; it is not a GPU timestamp.

use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
use vesta_gpui::{ElementId, Point, UiWindowId};
use vesta_gpui_vulkan::FrameCpuProfile;
use winit::{
    dpi::PhysicalSize,
    event::{ElementState, MouseButton},
    window::Window,
};

const FRAME_SAMPLE_INTERVAL: Duration = Duration::from_millis(500);
const WARMUP_FRAMES: u32 = 24;
const MAX_CONTEXTS: usize = 64;
const MAX_DISTRIBUTION_FRAMES: usize = 256;
const MAX_NATIVE_OBSERVATIONS: usize = 256;
const STAGE_COUNT: usize = 10;

pub(super) enum NativeGeometryEvent {
    Created,
    Resized(PhysicalSize<u32>),
    ScaleFactorChanged,
}

#[derive(Default)]
pub(super) struct FrameMeter {
    sample_start: Option<Instant>,
    sample_frames: u32,
    pub(super) display: Option<(f32, f32)>,
}

impl FrameMeter {
    pub(super) fn submitted(&mut self, now: Instant) {
        let Some(start) = self.sample_start else {
            self.sample_start = Some(now);
            return;
        };
        self.sample_frames += 1;
        let elapsed = now.saturating_duration_since(start);
        if elapsed >= FRAME_SAMPLE_INTERVAL {
            let seconds = elapsed.as_secs_f32();
            self.display = Some((
                self.sample_frames as f32 / seconds,
                seconds * 1000.0 / self.sample_frames as f32,
            ));
            self.sample_start = Some(now);
            self.sample_frames = 0;
        }
    }

    pub(super) fn reset(&mut self) {
        *self = Self::default();
    }
}

#[derive(Clone, Copy)]
pub(super) enum FrameStage {
    Build,
    Route,
    Layout,
    Reconcile,
    Paint,
    Hit,
    Renderer,
    TextViews,
    Accessibility,
    Ime,
}

impl FrameStage {
    const ALL: [Self; STAGE_COUNT] = [
        Self::Build,
        Self::Route,
        Self::Layout,
        Self::Reconcile,
        Self::Paint,
        Self::Hit,
        Self::Renderer,
        Self::TextViews,
        Self::Accessibility,
        Self::Ime,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Build => "build",
            Self::Route => "route",
            Self::Layout => "layout",
            Self::Reconcile => "reconcile",
            Self::Paint => "paint",
            Self::Hit => "hit",
            Self::Renderer => "renderer",
            Self::TextViews => "text_views",
            Self::Accessibility => "accessibility",
            Self::Ime => "ime",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct FrameContext {
    pub(super) window: UiWindowId,
    pub(super) gallery: bool,
    pub(super) text_focused: bool,
    pub(super) active: bool,
}

pub(super) struct FrameTrace(Option<Timing>);

struct Timing {
    last: Instant,
    stages: [Duration; STAGE_COUNT],
    renderer: Option<FrameCpuProfile>,
}

impl FrameTrace {
    pub(super) fn renderer_profile(&mut self) -> Option<&mut Option<FrameCpuProfile>> {
        self.0.as_mut().map(|timing| &mut timing.renderer)
    }

    pub(super) fn mark(&mut self, stage: FrameStage) {
        if let Some(timing) = &mut self.0 {
            let now = Instant::now();
            timing.stages[stage as usize] += now.saturating_duration_since(timing.last);
            timing.last = now;
        }
    }
}

#[derive(Default)]
struct Aggregate {
    warmup: u32,
    frames: u64,
    sums: [Duration; STAGE_COUNT],
    maxima: [Duration; STAGE_COUNT],
    total_max: Duration,
    native_frames: u64,
    native_sums: [Duration; FrameCpuProfile::STAGE_COUNT],
    native_maxima: [Duration; FrameCpuProfile::STAGE_COUNT],
    native_draws: u64,
    native_pipeline_binds: u64,
    native_scissor_updates: u64,
    native_prepared: u64,
    native_bitmap_uploads: u64,
    native_glyph_uploads: u64,
    native_bitmap_reuses: u64,
    native_glyph_reuses: u64,
    native_finishes: u64,
    // First completed post-warmup frames only; all-frame sums stay independent.
    distribution: Vec<(Duration, Option<Duration>)>,
}

impl Aggregate {
    fn submitted(&mut self, timing: Timing) {
        if self.warmup < WARMUP_FRAMES {
            self.warmup += 1;
            return;
        }
        self.frames += 1;
        let total = timing.stages.iter().sum();
        let mut native_total = None;
        if let Some(profile) = timing.renderer {
            self.native_frames += 1;
            native_total = Some(
                profile
                    .stages()
                    .into_iter()
                    .map(|(_, duration)| duration)
                    .sum(),
            );
            for (index, (_, duration)) in profile.stages().into_iter().enumerate() {
                self.native_sums[index] += duration;
                self.native_maxima[index] = self.native_maxima[index].max(duration);
            }
            self.native_draws += profile.draw_calls as u64;
            self.native_pipeline_binds += profile.pipeline_binds as u64;
            self.native_scissor_updates += profile.scissor_updates as u64;
            self.native_prepared += profile.prepared_draws as u64;
            self.native_bitmap_uploads += profile.bitmap_uploads as u64;
            self.native_glyph_uploads += profile.glyph_uploads as u64;
            self.native_bitmap_reuses += profile.bitmap_reuses as u64;
            self.native_glyph_reuses += profile.glyph_reuses as u64;
            self.native_finishes += u64::from(profile.finish_requested);
        }
        if self.distribution.len() < MAX_DISTRIBUTION_FRAMES {
            self.distribution.push((total, native_total));
        }
        self.total_max = self.total_max.max(total);
        for (index, duration) in timing.stages.into_iter().enumerate() {
            self.sums[index] += duration;
            self.maxima[index] = self.maxima[index].max(duration);
        }
    }

    fn percentiles(samples: &[Duration]) -> Option<(Duration, Duration)> {
        if samples.is_empty() {
            return None;
        }
        let mut sorted = samples.to_vec();
        sorted.sort_unstable();
        Some((
            sorted[(sorted.len() * 50).div_ceil(100) - 1],
            sorted[(sorted.len() * 95).div_ceil(100) - 1],
        ))
    }

    fn report(&self, context: FrameContext) {
        if self.frames == 0 {
            return;
        }
        let mean_ms = |duration: Duration| duration.as_secs_f64() * 1000.0 / self.frames as f64;
        let mut line = format!(
            "ui_profile window={} mode={} text_focus={} active={} frames={} warmup={} cpu_mean_ms={:.6} cpu_max_ms={:.6}",
            context.window.0,
            if context.gallery { "2d" } else { "3d" },
            context.text_focused,
            context.active,
            self.frames,
            self.warmup,
            mean_ms(self.sums.iter().sum()),
            self.total_max.as_secs_f64() * 1000.0,
        );
        let host_samples = self
            .distribution
            .iter()
            .map(|(host, _)| *host)
            .collect::<Vec<_>>();
        let native_samples = self
            .distribution
            .iter()
            .filter_map(|(_, native)| *native)
            .collect::<Vec<_>>();
        let raw_host = host_samples
            .iter()
            .map(|duration| duration.as_nanos().to_string())
            .collect::<Vec<_>>()
            .join(",");
        let raw_native = self
            .distribution
            .iter()
            .map(|(_, native)| {
                native.map_or_else(
                    || "null".to_owned(),
                    |duration| duration.as_nanos().to_string(),
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        line.push_str(&format!(
            " distribution_window=first_completed_post_warmup distribution_limit={} cpu_distribution_frames={} cpu_samples_ns=[{}] native_cpu_distribution_frames={} native_cpu_samples_ns=[{}]",
            MAX_DISTRIBUTION_FRAMES, host_samples.len(), raw_host, native_samples.len(), raw_native,
        ));
        for (label, samples) in [("cpu", &host_samples), ("native_cpu", &native_samples)] {
            if let Some((p50, p95)) = Self::percentiles(samples) {
                line.push_str(&format!(
                    " {label}_p50_ms={:.6} {label}_p95_ms={:.6}",
                    p50.as_secs_f64() * 1000.0,
                    p95.as_secs_f64() * 1000.0,
                ));
            }
        }
        for stage in FrameStage::ALL {
            let index = stage as usize;
            line.push_str(&format!(
                " {}_mean_ms={:.6} {}_max_ms={:.6}",
                stage.label(),
                mean_ms(self.sums[index]),
                stage.label(),
                self.maxima[index].as_secs_f64() * 1000.0,
            ));
        }
        // Native stages are nested observations, never added to the host total.
        if self.native_frames > 0 {
            line.push_str(&format!(
                " native_frames={} native_finish_frames={}",
                self.native_frames, self.native_finishes
            ));
            let native_mean_ms =
                |duration: Duration| duration.as_secs_f64() * 1000.0 / self.native_frames as f64;
            line.push_str(&format!(
                " native_cpu_mean_ms={:.6}",
                native_mean_ms(self.native_sums.iter().sum())
            ));
            for (index, (label, _)) in FrameCpuProfile::default().stages().into_iter().enumerate() {
                line.push_str(&format!(
                    " native_{label}_mean_ms={:.6} native_{label}_max_ms={:.6}",
                    native_mean_ms(self.native_sums[index]),
                    self.native_maxima[index].as_secs_f64() * 1000.0
                ));
            }
            for (label, count) in [
                ("prepared_draws", self.native_prepared),
                ("draw_calls", self.native_draws),
                ("pipeline_binds", self.native_pipeline_binds),
                ("scissor_updates", self.native_scissor_updates),
                ("bitmap_uploads", self.native_bitmap_uploads),
                ("bitmap_reuses", self.native_bitmap_reuses),
                ("glyph_uploads", self.native_glyph_uploads),
                ("glyph_reuses", self.native_glyph_reuses),
            ] {
                line.push_str(&format!(
                    " native_{label}_mean={:.3}",
                    count as f64 / self.native_frames as f64
                ));
            }
        }
        eprintln!("{line}");
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct NativeProjection {
    pub(super) window: UiWindowId,
    pub(super) active: bool,
    pub(super) scale_factor: f64,
    pub(super) owner: Option<ElementId>,
    pub(super) expected_node: Option<accesskit::NodeId>,
    pub(super) projected_node: accesskit::NodeId,
    pub(super) role: Option<accesskit::Role>,
    pub(super) bounds: Option<accesskit::Rect>,
}

impl NativeProjection {
    pub(super) fn from_update(
        window: UiWindowId,
        active: bool,
        scale_factor: f64,
        owner: Option<ElementId>,
        expected_node: Option<accesskit::NodeId>,
        update: &accesskit::TreeUpdate,
    ) -> Self {
        let node = update
            .nodes
            .iter()
            .find(|(id, _)| *id == update.focus)
            .map(|(_, node)| node);
        Self {
            window,
            active,
            scale_factor,
            owner,
            expected_node,
            projected_node: update.focus,
            role: node.map(accesskit::Node::role),
            bounds: node.and_then(accesskit::Node::bounds),
        }
    }
}

#[derive(Debug)]
pub(super) enum NativeObservation {
    Projection(NativeProjection),
    Pointer {
        window: UiWindowId,
        state: ElementState,
        button: MouseButton,
        logical: Point,
        derived_physical: (f64, f64),
        hit: Option<ElementId>,
    },
    Action {
        window: UiWindowId,
        node: accesskit::NodeId,
        action: accesskit::Action,
        resolved_owner: Option<ElementId>,
    },
}

// Debug formatting is the only consumer of non-projection fields; records contain no content.
impl NativeObservation {
    fn report(&self, sequence: usize) {
        match self {
            Self::Projection(projection) => {
                eprintln!("ui_native_observation sequence={sequence} projection={projection:?}");
            }
            Self::Pointer {
                window,
                state,
                button,
                logical,
                derived_physical,
                hit,
            } => {
                eprintln!(
                    "ui_native_observation sequence={sequence} pointer window={} state={state:?} button={button:?} logical=({},{}) derived_physical=({},{}) hit={hit:?}",
                    window.0, logical.x, logical.y, derived_physical.0, derived_physical.1
                );
            }
            Self::Action {
                window,
                node,
                action,
                resolved_owner,
            } => {
                eprintln!(
                    "ui_native_observation sequence={sequence} action window={} node={} kind={action:?} resolved_owner={resolved_owner:?}",
                    window.0, node.0
                );
            }
        }
    }
}

#[derive(Default)]
pub(super) struct FrameProfiler {
    enabled: bool,
    samples: HashMap<FrameContext, Aggregate>,
    limit_reported: bool,
    native_observations: usize,
    native_projections: HashMap<UiWindowId, NativeProjection>,
}

impl FrameProfiler {
    pub(super) fn from_args() -> Self {
        Self {
            enabled: std::env::args().any(|argument| argument == "--profile-ui"),
            samples: HashMap::new(),
            limit_reported: false,
            native_observations: 0,
            native_projections: HashMap::new(),
        }
    }

    pub(super) fn begin(&self) -> FrameTrace {
        FrameTrace(self.enabled.then(|| Timing {
            last: Instant::now(),
            stages: [Duration::ZERO; STAGE_COUNT],
            renderer: None,
        }))
    }

    pub(super) fn observe_native(&mut self, read: impl FnOnce() -> NativeObservation) {
        if !self.enabled || self.native_observations >= MAX_NATIVE_OBSERVATIONS {
            return;
        }
        let observation = read();
        if let NativeObservation::Projection(projection) = &observation {
            if self.native_projections.get(&projection.window) == Some(projection)
                || !self.native_projections.contains_key(&projection.window)
                    && self.native_projections.len() >= MAX_CONTEXTS
            {
                return;
            }
            self.native_projections
                .insert(projection.window, *projection);
        }
        self.native_observations += 1;
        observation.report(self.native_observations);
        if self.native_observations == MAX_NATIVE_OBSERVATIONS {
            eprintln!(
                "ui_native_observation limit={MAX_NATIVE_OBSERVATIONS}: further records omitted"
            );
        }
    }

    pub(super) fn report_native_geometry(
        &self,
        id: UiWindowId,
        event: NativeGeometryEvent,
        window: &Window,
    ) {
        if !self.enabled {
            return;
        }
        let (reason, physical) = match event {
            NativeGeometryEvent::Created => ("created", window.inner_size()),
            NativeGeometryEvent::Resized(size) => ("resized", size),
            NativeGeometryEvent::ScaleFactorChanged => {
                ("scale_factor_changed", window.inner_size())
            }
        };
        let scale = window.scale_factor();
        let logical = physical.to_logical::<f64>(scale);
        eprintln!(
            "ui_native_window_geometry window={} reason={reason} scale_factor={scale:.6} physical_width={} physical_height={} logical_width={:.6} logical_height={:.6}",
            id.0, physical.width, physical.height, logical.width, logical.height,
        );
    }

    pub(super) fn submitted(&mut self, trace: FrameTrace, context: FrameContext) {
        let Some(timing) = trace.0 else {
            return;
        };
        if !self.samples.contains_key(&context) && self.samples.len() >= MAX_CONTEXTS {
            if !self.limit_reported {
                eprintln!("ui_profile context_limit={MAX_CONTEXTS}: additional contexts omitted");
                self.limit_reported = true;
            }
            return;
        }
        self.samples.entry(context).or_default().submitted(timing);
    }
}

impl Drop for FrameProfiler {
    fn drop(&mut self) {
        let mut samples = self.samples.iter().collect::<Vec<_>>();
        samples.sort_by_key(|(context, _)| {
            (
                context.window.0,
                context.gallery,
                context.text_focused,
                context.active,
            )
        });
        for (context, aggregate) in samples {
            aggregate.report(*context);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distributions_exclude_warmup_partial_frames_and_foreign_contexts_and_stay_bounded() {
        let context = FrameContext {
            window: UiWindowId(1),
            gallery: true,
            text_focused: false,
            active: true,
        };
        let mut profiler = FrameProfiler::default();
        profiler.submitted(profiler.begin(), context);
        assert!(
            profiler.samples.is_empty(),
            "disabled sampling allocates no context"
        );
        profiler.observe_native(|| panic!("disabled diagnostics must not query platform metadata"));
        assert!(profiler.native_projections.is_empty());
        profiler.enabled = true;
        {
            let _unfinished = profiler.begin();
        }
        assert!(
            profiler.samples.is_empty(),
            "unsubmitted/failed outer frames never count"
        );
        let timing = |milliseconds, native: bool| {
            let mut stages = [Duration::ZERO; STAGE_COUNT];
            stages[FrameStage::Renderer as usize] = Duration::from_millis(milliseconds);
            FrameTrace(Some(Timing {
                last: Instant::now(),
                stages,
                renderer: native.then(FrameCpuProfile::default),
            }))
        };
        for _ in 0..WARMUP_FRAMES {
            profiler.submitted(timing(10_000, true), context);
        }
        assert!(profiler.samples[&context].distribution.is_empty());
        for milliseconds in 1..=MAX_DISTRIBUTION_FRAMES as u64 {
            profiler.submitted(timing(milliseconds, milliseconds % 2 == 0), context);
        }
        profiler.submitted(timing(20_000, true), context);
        let aggregate = &profiler.samples[&context];
        let samples = aggregate
            .distribution
            .iter()
            .map(|(host, _)| *host)
            .collect::<Vec<_>>();
        assert_eq!(aggregate.frames, MAX_DISTRIBUTION_FRAMES as u64 + 1);
        assert_eq!(aggregate.total_max, Duration::from_millis(20_000));
        assert_eq!(samples.len(), MAX_DISTRIBUTION_FRAMES);
        assert_eq!(
            Aggregate::percentiles(&samples),
            Some((Duration::from_millis(128), Duration::from_millis(244)))
        );
        assert_eq!(
            aggregate
                .distribution
                .iter()
                .filter(|(_, native)| native.is_some())
                .count(),
            128
        );
        assert_eq!(
            aggregate.native_frames, 129,
            "all-frame counts differ from capped distribution"
        );

        let foreign = FrameContext {
            window: UiWindowId(2),
            ..context
        };
        profiler.submitted(timing(30_000, true), foreign);
        assert_eq!(profiler.samples[&foreign].warmup, 1);
        assert!(profiler.samples[&foreign].distribution.is_empty());
        assert_eq!(
            profiler.samples[&context].frames,
            MAX_DISTRIBUTION_FRAMES as u64 + 1
        );
        for window in 3..=MAX_CONTEXTS as u64 {
            profiler.submitted(
                timing(1, false),
                FrameContext {
                    window: UiWindowId(window),
                    ..context
                },
            );
        }
        profiler.submitted(
            timing(1, false),
            FrameContext {
                window: UiWindowId(MAX_CONTEXTS as u64 + 1),
                ..context
            },
        );
        assert_eq!(profiler.samples.len(), MAX_CONTEXTS);
        assert!(profiler.limit_reported);
        let projection = NativeProjection {
            window: UiWindowId(1),
            active: true,
            scale_factor: 1.25,
            owner: Some(ElementId::new(10)),
            expected_node: Some(accesskit::NodeId(2)),
            projected_node: accesskit::NodeId(2),
            role: Some(accesskit::Role::TextInput),
            bounds: None,
        };
        profiler.observe_native(|| NativeObservation::Projection(projection));
        profiler.observe_native(|| NativeObservation::Projection(projection));
        assert_eq!(
            profiler.native_observations, 1,
            "unchanged projections do not emit"
        );
        for window in 2..=MAX_CONTEXTS as u64 + 1 {
            profiler.observe_native(|| {
                NativeObservation::Projection(NativeProjection {
                    window: UiWindowId(window),
                    ..projection
                })
            });
        }
        assert_eq!(profiler.native_projections.len(), MAX_CONTEXTS);
        assert_eq!(profiler.native_observations, MAX_CONTEXTS);
        for _ in MAX_CONTEXTS..MAX_NATIVE_OBSERVATIONS {
            profiler.observe_native(|| NativeObservation::Action {
                window: UiWindowId(1),
                node: accesskit::NodeId(2),
                action: accesskit::Action::Focus,
                resolved_owner: projection.owner,
            });
        }
        profiler.observe_native(|| panic!("exhausted diagnostics must not query more metadata"));
        assert_eq!(profiler.native_observations, MAX_NATIVE_OBSERVATIONS);
        // Avoid dumping synthetic benchmark values from this boundary test.
        profiler.samples.clear();
    }
}
