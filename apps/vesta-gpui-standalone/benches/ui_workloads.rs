//! Bounded real CPU work; no window, Device, author data or native latency claim.
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    error::Error,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};
use vesta_gpui::trace::{self, TraceCapture, TraceConfig, TraceDetail};
#[path = "ui_workloads/fixture.rs"]
mod fixture;
type AnyResult<T> = Result<T, Box<dyn Error>>;
const FRAMES: usize = 16;
const ORDERS: [[Mode; 3]; 6] = [
    [Mode::Off, Mode::Stages, Mode::Detailed],
    [Mode::Detailed, Mode::Stages, Mode::Off],
    [Mode::Stages, Mode::Off, Mode::Detailed],
    [Mode::Detailed, Mode::Off, Mode::Stages],
    [Mode::Off, Mode::Detailed, Mode::Stages],
    [Mode::Stages, Mode::Detailed, Mode::Off],
];

struct FontSet {
    blobs: Vec<(String, Arc<Vec<u8>>)>,
    manifest: Vec<Value>,
}
impl FontSet {
    fn load(required: bool) -> AnyResult<Self> {
        let mut fonts = Self {
            blobs: Vec::new(),
            manifest: Vec::new(),
        };
        for path in [
            r"C:\Windows\Fonts\segoeui.ttf",
            r"C:\Windows\Fonts\msyh.ttc",
            r"C:\Windows\Fonts\consola.ttf",
        ] {
            if let Ok(bytes) = fs::read(path) {
                fonts
                    .manifest
                    .push(json!({"path":path,"bytes":bytes.len()}));
                fonts.blobs.push((path.to_owned(), Arc::new(bytes)));
            }
        }
        if required && fonts.blobs.len() < 2 {
            return Err("workload_requires_recorded_windows_ui_and_cjk_fonts".into());
        }
        Ok(fonts)
    }
}

#[derive(Clone, Copy)]
enum Mode {
    Off,
    Stages,
    Detailed,
}
impl Mode {
    fn name(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Stages => "stages",
            Self::Detailed => "detailed",
        }
    }
    fn start(self) -> AnyResult<Option<TraceCapture>> {
        Ok(match self {
            Self::Off => None,
            Self::Stages => Some(TraceCapture::start_with_detail(
                TraceConfig::default(),
                TraceDetail::Stages,
            )?),
            Self::Detailed => Some(TraceCapture::start_with_detail(
                TraceConfig::default(),
                TraceDetail::Detailed,
            )?),
        })
    }
}

fn write_new(path: &Path, value: &str) -> AnyResult<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(value.as_bytes())?;
    Ok(())
}
fn stats(values: &[f64]) -> Value {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let percentile = |p: f64| sorted[(p * sorted.len() as f64).ceil() as usize - 1];
    json!({"samples":values.len(),"mean":values.iter().sum::<f64>()/values.len() as f64,
        "p50":percentile(0.5),"p95":percentile(0.95),"max":sorted.last()})
}

fn batch(
    kind: fixture::Kind,
    mode: Mode,
    round: usize,
    frames: usize,
    fonts: &FontSet,
    out: Option<&Path>,
) -> AnyResult<(Value, Vec<String>)> {
    assert!(!trace::is_recording(), "previous capture retired");
    let mut workload = fixture::Fixture::new(kind, fonts)?;
    for step in 0..fixture::WARMUP {
        workload.frame(step)?;
    }
    let capture = mode.start()?;
    let mut results = Vec::with_capacity(frames);
    for step in fixture::WARMUP..fixture::WARMUP + frames {
        results.push(workload.frame(step)?);
    }
    let capture_metadata = if let Some(capture) = capture {
        let snapshot = capture.stop();
        let s = snapshot.stats();
        assert_eq!(
            s.dropped_events, 0,
            "complete cost comparison requires all events"
        );
        assert_eq!(s.unfinished_spans, 0);
        assert_eq!(s.invalid_nesting, 0);
        assert!(!s.expired);
        let profiles = snapshot.frames();
        assert_eq!(profiles.len(), frames);
        let mut profile_ms = Vec::new();
        for (profile, frame) in profiles.iter().zip(&results) {
            assert_eq!(profile.name, "Workload.Main");
            assert!(!profile.unfinished);
            assert!(profile.duration.as_secs_f64() * 1000.0 <= frame.ms + 0.000001);
            for (name, value) in [
                ("layout.nodes", frame.nodes),
                ("projected.rows", frame.rows),
                ("layout.passes", frame.passes),
            ] {
                assert!(
                    profile
                        .markers
                        .iter()
                        .any(|m| m.name == name && m.value == Some(value as f64)),
                    "real frame counter missing"
                );
            }
            profile_ms.push(profile.duration.as_secs_f64() * 1000.0);
        }
        let file = format!("{}-r{round}-{}.trace.json", kind.name(), mode.name());
        if let Some(out) = out {
            write_new(&out.join(&file), &snapshot.to_chrome_json()?)?;
        }
        json!({"events":s.events,"dropped_events":s.dropped_events,"unfinished_spans":s.unfinished_spans,
            "invalid_nesting":s.invalid_nesting,"expired":s.expired,"trace_file":file,"recorded_frame_ms":profile_ms})
    } else {
        Value::Null
    };
    assert!(
        !trace::is_recording(),
        "stopped owner releases active recording"
    );
    let signatures = results.iter().map(|f| f.signature.clone()).collect();
    let times = results.iter().map(|f| f.ms).collect::<Vec<_>>();
    let record = json!({"workload":kind.name(),"mode":mode.name(),"round":round,"warmup_frames":fixture::WARMUP,
        "frames":results.iter().enumerate().map(|(i,f)|f.json(fixture::WARMUP+i)).collect::<Vec<_>>(),
        "wall_ms":stats(&times),"capture":capture_metadata});
    Ok((record, signatures))
}

fn bounds_and_idle() -> AnyResult<Value> {
    let idle = TraceCapture::start(TraceConfig::default())?;
    let before = idle.stats().events;
    std::thread::sleep(Duration::from_millis(20));
    let after = idle.stats().events;
    assert_eq!(before, after, "idle recorder cannot invent work");
    drop(idle);
    assert!(!trace::is_recording());
    let capture = TraceCapture::start(TraceConfig {
        max_events: 16,
        max_duration: Duration::from_secs(30),
    })?;
    {
        let _frame = trace::frame("Bounds.Main");
        for _ in 0..1024 {
            trace::instant("workload.probe", "bounded");
        }
    }
    let snapshot = capture.stop();
    let bounded = snapshot.stats();
    assert_eq!(bounded.events, 16);
    assert_eq!(bounded.dropped_events, 1009);
    assert_eq!(bounded.unfinished_spans, 0);
    assert_eq!(bounded.invalid_nesting, 0);
    drop(snapshot);
    assert!(!trace::is_recording());
    let expires = TraceCapture::start(TraceConfig {
        max_events: 16,
        max_duration: Duration::from_millis(10),
    })?;
    std::thread::sleep(Duration::from_millis(20));
    assert!(!expires.is_recording());
    let expired = expires.stop().stats().expired;
    assert!(expired);
    Ok(
        json!({"idle_events_before":before,"idle_events_after":after,"bounded_events":bounded.events,
        "dropped_events":bounded.dropped_events,"unfinished_spans":bounded.unfinished_spans,"expired":expired,"recording_after_stop":trace::is_recording()}),
    )
}

fn hook_cost() -> Value {
    const ITERATIONS: usize = 1_000_000;
    let mut rows = Vec::new();
    for round in 0..6 {
        for enabled in if round % 2 == 0 {
            [false, true]
        } else {
            [true, false]
        } {
            assert!(!trace::is_recording());
            let start = Instant::now();
            let mut value = 0_u64;
            for index in 0..ITERATIONS {
                if enabled {
                    let _frame = trace::frame("Disabled.Main");
                    let _span = trace::span("workload.probe", "disabled");
                    trace::instant("workload.probe", "disabled");
                    trace::counter("workload.probe", "disabled", 1.0);
                }
                value = value.wrapping_add(std::hint::black_box(index as u64));
            }
            std::hint::black_box(value);
            rows.push(
                json!({"round":round,"case":if enabled{"disabled_four_hooks"}else{"control"},
                "iterations":ITERATIONS,"elapsed_ns":start.elapsed().as_nanos() as u64}),
            );
        }
    }
    json!({"hooks_per_iteration":4,"rows":rows})
}

fn main() -> AnyResult<()> {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    let explicit_smoke = args.iter().any(|arg| arg == "--test");
    let paths = args
        .iter()
        .filter(|arg| *arg != "--test" && *arg != "--bench" && *arg != "--nocapture")
        .collect::<Vec<_>>();
    // Cargo test --all-targets invokes harness=false benches without arguments.
    let smoke = explicit_smoke || paths.is_empty();
    if paths.len() > 1 || (explicit_smoke && !paths.is_empty()) {
        return Err("Usage: cargo bench --profile dev -p vesta-gpui-standalone --bench ui_workloads -- <new-directory>".into());
    }
    let output = if smoke {
        None
    } else {
        let path = PathBuf::from(paths[0]);
        fs::create_dir(&path)?;
        Some(path)
    };
    let fonts = FontSet::load(!smoke)?;
    let mut records = Vec::new();
    let mut reference = BTreeMap::new();
    for kind in fixture::Kind::ALL {
        for (round, order) in ORDERS
            .iter()
            .enumerate()
            .take(if smoke { 1 } else { ORDERS.len() })
        {
            for mode in order {
                let (record, signatures) = batch(
                    kind,
                    *mode,
                    round,
                    if smoke { 2 } else { FRAMES },
                    &fonts,
                    output.as_deref(),
                )?;
                let expected = reference
                    .entry(kind.name())
                    .or_insert_with(|| signatures.clone());
                assert_eq!(
                    &signatures, expected,
                    "trace mode/order must not change work or geometry"
                );
                records.push(record);
            }
        }
    }
    let lifecycle = bounds_and_idle()?;
    if let Some(output) = output {
        let report = json!({"schema_version":1,"scope":"headless_public_api_cpu_workloads","package_version":env!("CARGO_PKG_VERSION"),
            "process_id":std::process::id(),"executable":std::env::current_exe()?,
            "timing_scope":"tree build, retained layout/reconciliation, edit, route/hit, paint and CPU preparation including trace entry/exit; excludes setup, validation, object destruction and export",
            "debug_assertions":cfg!(debug_assertions),"native":false,"gpu":false,"physical_input":false,"fonts":fonts.manifest,
            "orders":ORDERS.iter().map(|order|order.iter().map(|m|m.name()).collect::<Vec<_>>()).collect::<Vec<_>>(),
            "frames_per_batch":FRAMES,"workloads":records,"lifecycle":lifecycle,"disabled_hook_cost":hook_cost()});
        write_new(
            &output.join("report.json"),
            &serde_json::to_string_pretty(&report)?,
        )?;
        println!(
            "ui_workloads_complete batches={} frames={}",
            report["workloads"].as_array().expect("records").len(),
            FRAMES * ORDERS.len() * 3 * 4
        );
    } else {
        println!(
            "ui_workloads_smoke_passed workloads=4 modes=3 geometry_equal=true pid={}",
            std::process::id()
        );
    }
    Ok(())
}
