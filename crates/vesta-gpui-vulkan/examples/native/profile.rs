//! Finite example-only CPU measurements; renderer timings are nested in host time.
use super::{Duration, FrameCpuProfile, Instant, Mode};

const WARMUP: usize = 24;
const SAMPLES: usize = 96;
pub(super) const PERIOD: Duration = Duration::from_millis(16);
pub(super) const TIMEOUT: Duration = Duration::from_secs(30);

struct Sample {
    host_ms: f64,
    native: FrameCpuProfile,
}

pub(super) struct Profile {
    pub next: Instant,
    pub started: Instant,
    completed: usize,
    extent_scale: Option<([u32; 2], u64)>,
    samples: Vec<Sample>,
}

impl Profile {
    pub fn new(now: Instant) -> Self {
        Self {
            next: now,
            started: now,
            completed: 0,
            extent_scale: None,
            samples: Vec::with_capacity(SAMPLES),
        }
    }
    pub fn complete(&self) -> bool {
        self.samples.len() == SAMPLES
    }
    pub fn observe(
        &mut self,
        host: Duration,
        native: Option<FrameCpuProfile>,
        extent: [u32; 2],
        scale: f64,
    ) -> Result<(), String> {
        let native = native.ok_or("successful profiled render has no CPU snapshot")?;
        self.completed += 1;
        self.next = Instant::now() + PERIOD;
        if self.completed <= WARMUP || self.complete() {
            return Ok(());
        }
        let geometry = (extent, scale.to_bits());
        if self
            .extent_scale
            .is_some_and(|previous| previous != geometry)
        {
            return Err("profile extent/scale changed during sampling".into());
        }
        self.extent_scale = Some(geometry);
        let host_ms = host.as_secs_f64() * 1000.0;
        let native_ms = native
            .stages()
            .map(|(_, duration)| duration.as_secs_f64() * 1000.0);
        println!(
            "native_profile_sample: {{\"sample\":{},\"host_cpu_ms\":{host_ms:.6},\"native_cpu_ms\":{native_ms:?},\"prepared\":{},\"draw_calls\":{},\"pipeline_binds\":{},\"scissor_updates\":{},\"bitmap_uploads\":{},\"bitmap_reuses\":{},\"glyph_uploads\":{},\"glyph_reuses\":{},\"finish_requested\":{}}}",
            self.samples.len() + 1,
            native.prepared_draws,
            native.draw_calls,
            native.pipeline_binds,
            native.scissor_updates,
            native.bitmap_uploads,
            native.bitmap_reuses,
            native.glyph_uploads,
            native.glyph_reuses,
            native.finish_requested
        );
        self.samples.push(Sample { host_ms, native });
        Ok(())
    }
    pub fn report(&self, mode: Mode) -> Result<(), String> {
        if !self.complete() {
            return Err("native_profile_incomplete: closed before 96 samples".into());
        }
        let (extent, scale) = self.extent_scale.ok_or("profile geometry missing")?;
        println!(
            "native_profile_summary: {{\"scene\":\"{mode:?}-counter-16ms\",\"warmup_frames\":{WARMUP},\"samples\":{SAMPLES},\"completed_frames\":{},\"extent\":{extent:?},\"scale\":{},\"host_cpu_ms\":{},\"renderer_cpu_ms\":{},\"quantile\":\"nearest-rank ceil(p*N)-1\",\"gpu_time\":false,\"physical_input\":false}}",
            self.completed,
            f64::from_bits(scale),
            stats(self.samples.iter().map(|s| s.host_ms)),
            stats(self.samples.iter().map(|s| {
                s.native
                    .stages()
                    .iter()
                    .map(|(_, t)| t.as_secs_f64() * 1000.0)
                    .sum()
            }))
        );
        for index in 0..FrameCpuProfile::STAGE_COUNT {
            let name = self.samples[0].native.stages()[index].0;
            println!(
                "native_profile_stage: {{\"stage\":\"{name}\",\"cpu_ms\":{}}}",
                stats(
                    self.samples
                        .iter()
                        .map(|s| s.native.stages()[index].1.as_secs_f64() * 1000.0)
                )
            );
        }
        Ok(())
    }
}

// Only called with the complete bounded sample set, not with empty/partial runs.
fn stats(values: impl Iterator<Item = f64>) -> String {
    let mut values: Vec<_> = values.collect();
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    values.sort_by(f64::total_cmp);
    let p50 = values[(values.len() * 50).div_ceil(100) - 1];
    let p95 = values[(values.len() * 95).div_ceil(100) - 1];
    let max = values[values.len() - 1];
    format!("{{\"mean\":{mean:.6},\"p50\":{p50:.6},\"p95\":{p95:.6},\"max\":{max:.6}}}")
}
