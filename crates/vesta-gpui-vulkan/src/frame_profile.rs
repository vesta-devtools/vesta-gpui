//! Opt-in observations of a successful native frame; no scheduling or GPU timing.
use std::time::{Duration, Instant};

/// CPU wall-clock intervals inside one renderer call. Nested in host render time.
#[derive(Clone, Copy, Debug, Default)]
pub struct FrameCpuProfile {
    durations: [Duration; Self::STAGE_COUNT],
    /// Prepared entries after adjacent-shape merging, before glyph/bitmap batching.
    /// Includes individual glyphs and excludes the optional 3D scene.
    pub prepared_draws: usize,
    /// Uploaded UI batches; optional scene draw calls are not counted here.
    pub draw_calls: usize,
    /// Actual safe UI pipeline binds in this record call; excludes scene bindings.
    pub pipeline_binds: usize,
    /// Actual safe UI scissor updates in this record call; excludes scene updates.
    pub scissor_updates: usize,
    pub bitmap_uploads: usize,
    pub bitmap_reuses: usize,
    /// Existing RenderOutput glyph counters, not physical atlas page transfers.
    pub glyph_uploads: usize,
    pub glyph_reuses: usize,
    /// The caller requested the existing explicit finish path for this frame.
    pub finish_requested: bool,
}

impl FrameCpuProfile {
    pub const STAGE_COUNT: usize = 9;

    pub fn stages(self) -> [(&'static str, Duration); Self::STAGE_COUNT] {
        Stage::ALL.map(|stage| (stage.label(), self.durations[stage as usize]))
    }
}

#[derive(Clone, Copy)]
pub(super) enum Stage {
    Readiness,
    Targets,
    Prepare,
    Acquire,
    Upload,
    Record,
    Commands,
    Submit,
    Finish,
}

impl Stage {
    const ALL: [Self; FrameCpuProfile::STAGE_COUNT] = [
        Self::Readiness,
        Self::Targets,
        Self::Prepare,
        Self::Acquire,
        Self::Upload,
        Self::Record,
        Self::Commands,
        Self::Submit,
        Self::Finish,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Readiness => "readiness",
            Self::Targets => "targets",
            Self::Prepare => "prepare",
            Self::Acquire => "acquire",
            Self::Upload => "upload",
            Self::Record => "record",
            Self::Commands => "commands",
            Self::Submit => "submit",
            Self::Finish => "finish",
        }
    }
}

pub(super) struct Trace {
    timing: Option<Timing>,
    interval: vesta_gpui::trace::TraceSpan,
}

struct Timing {
    last: Instant,
    profile: FrameCpuProfile,
}

impl Trace {
    pub(super) fn new(enabled: bool) -> Self {
        Self {
            timing: enabled.then(|| Timing {
                last: Instant::now(),
                profile: FrameCpuProfile::default(),
            }),
            interval: vesta_gpui::trace::span("gpui.vulkan.cpu", "phase.interrupted"),
        }
    }

    pub(super) fn mark(&mut self, stage: Stage) {
        std::mem::take(&mut self.interval).finish_with_name(stage.label());
        self.interval = vesta_gpui::trace::span("gpui.vulkan.cpu", "phase.interrupted");
        if let Some(timing) = &mut self.timing {
            let now = Instant::now();
            timing.profile.durations[stage as usize] += now.saturating_duration_since(timing.last);
            timing.last = now;
        }
    }

    pub(super) fn prepared(&mut self, count: usize) {
        if let Some(timing) = &mut self.timing {
            timing.profile.prepared_draws = count;
        }
    }

    pub(super) fn recorded(&mut self, bindings: super::checked_pipeline::UiBindings) {
        if let Some(timing) = &mut self.timing {
            timing.profile.pipeline_binds = bindings.pipeline_binds;
            timing.profile.scissor_updates = bindings.scissor_updates;
        }
    }

    pub(super) fn completed(
        self,
        output: &vesta_gpui::render::RenderOutput,
        finish_requested: bool,
    ) -> Option<FrameCpuProfile> {
        self.interval.finish_with_name("renderer.tail");
        self.timing.map(|timing| FrameCpuProfile {
            draw_calls: output.draw_calls,
            bitmap_uploads: output.bitmap_uploads,
            bitmap_reuses: output.bitmap_reuses,
            glyph_uploads: output.glyph_uploads,
            glyph_reuses: output.glyph_reuses,
            finish_requested,
            ..timing.profile
        })
    }
}
