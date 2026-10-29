//! Optional CPU observations of the existing lazy consumer, not a native renderer.

use super::*;
use std::time::Instant;

const WARMUP: usize = 24;
const SAMPLES: usize = 160;

#[derive(Clone, Debug, Default)]
pub(super) struct Frame {
    pub width: f32,
    pub stages_ns: [u128; 6],
    pub rows: usize,
    pub nodes: usize,
    pub paint: usize,
    pub hits: usize,
    pub semantics: usize,
}

pub(super) struct Trace {
    enabled: bool,
    start: Option<Instant>,
    pub stages_ns: [u128; 6],
}

impl Trace {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            start: None,
            stages_ns: [0; 6],
        }
    }

    pub fn start(&mut self) {
        self.start = self.enabled.then(Instant::now);
    }

    pub fn finish(&mut self, stage: usize) {
        if let Some(start) = self.start.take() {
            self.stages_ns[stage] = start.elapsed().as_nanos();
        }
    }
}

fn emit(
    projection: &LazyProjection,
    phase: usize,
    data_count: usize,
    transition: &Frame,
    samples: &[Frame],
) {
    let raw = samples.iter().map(|f| f.stages_ns).collect::<Vec<_>>();
    let counts = samples
        .iter()
        .map(|f| [f.rows, f.nodes, f.paint, f.hits, f.semantics])
        .collect::<Vec<_>>();
    let mut p50 = [0; 6];
    let mut p95 = [0; 6];
    let transition_counts = [
        transition.rows,
        transition.nodes,
        transition.paint,
        transition.hits,
        transition.semantics,
    ];
    for stage in 0..6 {
        let mut sorted = samples
            .iter()
            .map(|frame| frame.stages_ns[stage])
            .collect::<Vec<_>>();
        sorted.sort_unstable();
        p50[stage] = sorted[(sorted.len() * 50).div_ceil(100) - 1];
        p95[stage] = sorted[(sorted.len() * 95).div_ceil(100) - 1];
    }
    println!(
        "collection_cpu_profile {{\"schema_version\":1,\"window\":{},\"kind\":\"{:?}\",\"phase\":{phase},\"data_count\":{data_count},\"width\":{},\"warmup\":{WARMUP},\"samples\":{SAMPLES},\"stage_names\":[\"caller_and_build\",\"route\",\"layout\",\"paint\",\"hit\",\"semantics\"],\"transition_ns\":{:?},\"transition_counts\":{transition_counts:?},\"p50_ns\":{p50:?},\"p95_ns\":{p95:?},\"sample_ns\":{raw:?},\"count_names\":[\"factory_rows\",\"layout_nodes\",\"paint_commands\",\"hit_entries\",\"semantic_nodes\"],\"counts\":{counts:?}}}",
        projection.window.0, projection.kind, transition.width, transition.stages_ns,
    );
}

pub(super) fn run(context: &mut TextInputContext, prototype: &Record) -> Result<(), String> {
    println!(
        "collection_cpu_identity pid={} native=false gpu=false latency=false",
        std::process::id()
    );
    let mut records = (102_u16..10_102)
        .map(|id| Record {
            id: RecordId(id),
            name: format!("Record {id}"),
            note: prototype.note.clone(),
        })
        .collect::<Vec<_>>();
    let focused = records[1].id;
    let mut selection = ListSelectionState::single(focused);
    let mut projections = Vec::new();
    for window in [UiWindowId(1), UiWindowId(2)] {
        for kind in [
            Kind::List,
            Kind::VariableList,
            Kind::Tree,
            Kind::Table,
            Kind::Tiles,
        ] {
            let scope = ElementIdAllocator
                .allocate_scope()
                .map_err(|error| format!("profile scope: {error:?}"))?;
            let mut input = InputOwnership::default();
            input.set_focus(Some(InputTarget {
                window,
                element: identity(scope, focused, 4).element,
            }));
            projections.push(LazyProjection {
                scope,
                window,
                kind,
                layout: Default::default(),
                semantics: Default::default(),
                input,
                profile: Some(Frame::default()),
            });
        }
    }
    for phase in 0..4 {
        if phase == 1 {
            records[1..].reverse();
        } else if phase == 3 {
            records.retain(|record| record.id != focused);
        }
        selection.reconcile(&records.iter().map(|record| record.id).collect::<Vec<_>>());
        for projection in &mut projections {
            let width = match (projection.window.0, phase >= 2) {
                (1, false) | (2, true) => 640.0,
                _ => 480.0,
            };
            projection.profile.as_mut().ok_or("profile disabled")?.width = width;
            // Keep the first changed frame rather than hiding transition cost in warmup.
            projection.frame(&records, &selection, focused, phase == 2, context)?;
            let transition = projection.profile.clone().ok_or("transition missing")?;
            for _ in 0..WARMUP {
                projection.frame(&records, &selection, focused, phase == 2, context)?;
            }
            let mut samples = Vec::with_capacity(SAMPLES);
            for _ in 0..SAMPLES {
                projection.frame(&records, &selection, focused, phase == 2, context)?;
                samples.push(projection.profile.clone().ok_or("profile frame missing")?);
            }
            emit(projection, phase, records.len(), &transition, &samples);
        }
    }
    println!(
        "collection_cpu_complete groups=40 transitions=40 warmup_frames=960 measured_frames=6400 verified_frames=7400 native=false gpu=false latency=false"
    );
    Ok(())
}
