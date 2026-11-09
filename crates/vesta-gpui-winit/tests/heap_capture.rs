//! Isolated actual allocator integration: no process-global test recorder races.
use std::{
    process::{Command, Stdio},
    time::Duration,
};
use vesta_gpui::trace::{self, TraceConfig, TraceDetail, memory::MemoryConfig};
#[global_allocator]
static ALLOCATOR: vesta_gpui_winit::UiTraceAllocator = vesta_gpui_winit::UiTraceAllocator;

#[test]
fn heap_capture_child() {
    let Some(directory) = std::env::var_os("VESTA_HEAP_CAPTURE_CHILD") else {
        return;
    };
    let directory = std::path::PathBuf::from(directory);
    let baseline = vec![3u8; 63];
    let baseline_address = baseline.as_ptr() as usize;
    let recording = vesta_gpui_winit::UiTraceRecording::start_with_memory(
        directory.join("heap.json"),
        TraceConfig {
            max_events: 5000,
            max_duration: Duration::from_secs(10),
        },
        TraceDetail::Stages,
        MemoryConfig {
            max_events: 10000,
            stack_frames: 32,
        },
    )
    .expect("memory recording");
    let _frame = trace::frame("HeapFixture.Main");
    let mut bytes = Vec::<u8>::with_capacity(4096);
    bytes.resize(4096, 7);
    let original = bytes.as_ptr() as usize;
    bytes.reserve_exact(8192);
    let grown = bytes.as_ptr() as usize;
    assert_eq!(bytes[0], 7);
    let zero = vec![0u8; 777];
    let zero_address = zero.as_ptr() as usize;
    assert!(zero.iter().all(|b| *b == 0));
    let cross = Box::new([1u8; 311]);
    let cross_address = cross.as_ptr() as usize;
    std::thread::Builder::new()
        .name("HeapFixture.Worker".into())
        .spawn(move || {
            let _work = trace::span("fixture", "memory.worker");
            drop(cross);
        })
        .expect("worker")
        .join()
        .expect("worker completed");
    drop(baseline);
    drop(zero);
    drop(bytes);
    let survivor = vec![9u8; 913];
    let survivor_address = survivor.as_ptr() as usize;
    std::hint::black_box(&survivor);
    drop(_frame);
    recording
        .finish()
        .expect("frozen memory export and local symbols");
    drop(survivor);
    let expected = serde_json::json!({"baseline":format!("0x{baseline_address:x}"),"original":format!("0x{original:x}"),"grown":format!("0x{grown:x}"),
        "zero":format!("0x{zero_address:x}"),"cross":format!("0x{cross_address:x}"),"survivor":format!("0x{survivor_address:x}")});
    std::fs::write(directory.join("expected.json"), expected.to_string())
        .expect("independent actual addresses");
    // An old frozen export/late free never enters the successor capture.
    let successor = vesta_gpui_winit::UiTraceRecording::start_with_memory(
        directory.join("successor.json"),
        TraceConfig::default(),
        TraceDetail::Stages,
        MemoryConfig {
            max_events: 1000,
            stack_frames: 0,
        },
    )
    .expect("successor");
    let block = vec![4u8; 231];
    std::hint::black_box(&block);
    drop(block);
    successor.finish().expect("successor export");
    // Diagnostic contract simulation (not an actual allocator/OOM failure):
    // callback recursion is suppressed and a callback panic preserves forwarding.
    use trace::memory::{AllocatorOperation, allocator_operation};
    let simulated = trace::TraceCapture::start_with_memory(
        TraceConfig::default(),
        TraceDetail::Stages,
        Some(MemoryConfig {
            max_events: 1,
            stack_frames: 1,
        }),
    )
    .expect("bounded simulated hook contract");
    let value = allocator_operation(
        AllocatorOperation::Alloc { size: 8 },
        || 0x1234,
        |_, frames| {
            assert_eq!(
                allocator_operation(
                    AllocatorOperation::Alloc { size: 4 },
                    || 0x5678,
                    |_, _| panic!("nested hook must be suppressed")
                ),
                0x5678
            );
            frames[0] = 0x1000;
            1
        },
    );
    assert_eq!(value, 0x1234);
    assert_eq!(
        simulated
            .memory_stats()
            .expect("admitted memory fixture")
            .events,
        1
    );
    assert!(
        simulated
            .memory_stats()
            .expect("admitted memory fixture")
            .limit_reached
    );
    let snapshot = simulated.stop();
    assert!(
        snapshot
            .memory()
            .expect("admitted memory fixture")
            .event_limit_reached
    );
    assert_eq!(
        snapshot
            .memory()
            .expect("admitted memory fixture")
            .events
            .len(),
        1
    );
    let panic_capture = trace::TraceCapture::start_with_memory(
        TraceConfig::default(),
        TraceDetail::Stages,
        Some(MemoryConfig {
            max_events: 10,
            stack_frames: 1,
        }),
    )
    .expect("admitted memory fixture");
    assert_eq!(
        allocator_operation(
            AllocatorOperation::Alloc { size: 8 },
            || 0x1234,
            |_, _| panic!("simulated diagnostic failure")
        ),
        0x1234
    );
    assert_eq!(
        panic_capture
            .memory_stats()
            .expect("admitted memory fixture")
            .hook_errors,
        1
    );
    assert!(
        panic_capture
            .stop()
            .memory()
            .expect("admitted memory fixture")
            .events
            .is_empty()
    );
    let expired = trace::TraceCapture::start_with_memory(
        TraceConfig {
            max_events: 100,
            max_duration: Duration::from_millis(2),
        },
        TraceDetail::Stages,
        Some(MemoryConfig {
            max_events: 10,
            stack_frames: 0,
        }),
    )
    .expect("admitted memory fixture");
    std::thread::sleep(Duration::from_millis(10));
    assert_eq!(
        allocator_operation(AllocatorOperation::Alloc { size: 8 }, || 0x1234, |_, _| 0),
        0x1234
    );
    let snapshot = expired.stop();
    assert!(snapshot.stats().expired);
    assert!(
        snapshot
            .memory()
            .expect("admitted memory fixture")
            .events
            .is_empty()
    );
    assert!(
        snapshot
            .memory()
            .expect("admitted memory fixture")
            .elapsed_ns
            <= 2_000_000
    );
    // Contract simulation: bound unique stacks without losing heap operations.
    let stack_budget = trace::TraceCapture::start_with_memory(
        TraceConfig::default(),
        TraceDetail::Stages,
        Some(MemoryConfig {
            max_events: 9000,
            stack_frames: 1,
        }),
    )
    .expect("stack budget capture");
    for index in 0..8193usize {
        allocator_operation(
            AllocatorOperation::Alloc { size: 8 },
            || 0x100000 + index,
            |_, frames| {
                frames[0] = 0x200000 + index;
                1
            },
        );
    }
    let snapshot = stack_budget.stop();
    let memory = snapshot.memory().expect("bounded stack capture");
    assert_eq!(memory.events.len(), 8193);
    assert_eq!(memory.stacks.len(), 8192);
    assert_eq!(memory.stack_observations_dropped, 1);
    assert!(
        memory
            .events
            .last()
            .expect("last operation")
            .stack
            .is_none()
    );
}

#[test]
fn real_heap_events_preserve_reallocation_cross_thread_free_stacks_and_survivors() {
    let directory = tempfile::tempdir().expect("owned capture directory");
    let result = Command::new(std::env::current_exe().expect("fixture executable"))
        .args(["--exact", "heap_capture_child", "--nocapture"])
        .env("VESTA_HEAP_CAPTURE_CHILD", directory.path())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("isolated recorder process");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(
        &std::fs::read(directory.path().join("heap.json")).expect("capture"),
    )
    .expect("JSON");
    let expected: serde_json::Value = serde_json::from_slice(
        &std::fs::read(directory.path().join("expected.json")).expect("addresses"),
    )
    .expect("JSON");
    let memory = &value["gpui_memory"];
    let events = memory["events"].as_array().expect("actual operations");
    assert_eq!(memory["baseline_known"], false);
    assert_eq!(memory["hook_errors"], 0);
    assert_eq!(memory["event_limit_reached"], false);
    assert!(events.iter().any(|e| e["operation"]["op"] == "alloc"
        && e["operation"]["address"] == expected["original"]
        && e["operation"]["size"] == 4096));
    assert!(events.iter().any(|e| e["operation"]["op"] == "realloc"
        && e["operation"]["old_address"] == expected["original"]
        && e["operation"]["address"] == expected["grown"]));
    assert!(events.iter().any(|e| e["operation"]["op"] == "alloc"
        && e["operation"]["address"] == expected["zero"]
        && e["operation"]["size"] == 777));
    let baseline_free = events
        .iter()
        .position(|e| {
            e["operation"]["op"] == "free" && e["operation"]["address"] == expected["baseline"]
        })
        .expect("pre-capture allocation is freed during recording");
    assert!(
        !events[..baseline_free]
            .iter()
            .any(|e| e["operation"]["op"] == "alloc"
                && e["operation"]["address"] == expected["baseline"])
    );
    let worker = value["traceEvents"]
        .as_array()
        .expect("CPU work")
        .iter()
        .find(|e| e["name"] == "memory.worker")
        .expect("worker span")["tid"]
        .clone();
    assert!(events.iter().any(|e| e["thread"] == worker
        && e["operation"]["op"] == "free"
        && e["operation"]["address"] == expected["cross"]));
    assert!(
        memory["threads"]
            .as_array()
            .expect("thread metadata")
            .iter()
            .any(|t| t["thread"] == worker && t["name"] == "HeapFixture.Worker")
    );
    // System may reuse an earlier freed address for this still-live block.
    // Check its own lifetime, rather than every prior operation at that address.
    let survivor_alloc = events
        .iter()
        .rposition(|e| {
            e["operation"]["op"] == "alloc"
                && e["operation"]["address"] == expected["survivor"]
                && e["operation"]["size"] == 913
        })
        .expect("actual survivor allocation");
    assert!(!events[survivor_alloc + 1..].iter().any(|e| {
        (e["operation"]["op"] == "free" && e["operation"]["address"] == expected["survivor"])
            || (e["operation"]["op"] == "realloc"
                && e["operation"]["old_address"] == expected["survivor"])
    }));
    #[cfg(windows)]
    {
        let stacks = memory["stacks"].as_array().expect("actual stacks");
        assert!(!stacks.is_empty());
        assert!(
            stacks
                .iter()
                .flat_map(|s| s["frames"].as_array().expect("frames"))
                .any(|f| f["function"]
                    .as_str()
                    .is_some_and(|n| n.contains("heap_capture_child"))),
            "allocation caller must be symbolized: {memory}"
        );
    }
    let successor: serde_json::Value = serde_json::from_slice(
        &std::fs::read(directory.path().join("successor.json")).expect("successor"),
    )
    .expect("JSON");
    assert!(
        successor["gpui_memory"]["events"]
            .as_array()
            .expect("new memory events")
            .iter()
            .any(|e| e["operation"]["op"] == "alloc" && e["operation"]["size"] == 231)
    );
    assert!(
        successor["gpui_memory"]["stacks"]
            .as_array()
            .expect("stacks disabled")
            .is_empty()
    );
}
