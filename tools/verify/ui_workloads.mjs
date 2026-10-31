import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import {pathToFileURL} from 'node:url';

const modes = ['off', 'stages', 'detailed'];
const kinds = ['static', 'resize', 'edit', 'collection'];
const orders = [modes, ['detailed', 'stages', 'off'], ['stages', 'off', 'detailed'],
  ['detailed', 'off', 'stages'], ['off', 'detailed', 'stages'], ['stages', 'detailed', 'off']];
const equal = (a, b, why) => assert.deepEqual(a, b, why);
const nonnegative = (n, why) => assert(Number.isFinite(n) && n >= 0, why);
const positive = (n, why) => assert(Number.isSafeInteger(n) && n > 0, why);
const near = (a, b, why) => {
  nonnegative(a, why);
  nonnegative(b, why);
  assert(Math.abs(a - b) <= 1e-7 * Math.max(1, a, b), why);
};
function distribution(values) {
  const sorted = [...values].sort((a, b) => a - b);
  return {samples: values.length, mean: values.reduce((a, b) => a + b, 0) / values.length,
    p50: sorted[Math.ceil(values.length * .5) - 1], p95: sorted[Math.ceil(values.length * .95) - 1],
    max: sorted.at(-1)};
}

function verifyTrace(trace, batch, pid) {
  const meta = trace.gpui_trace;
  equal(meta.schema_version, 2, 'trace schema');
  equal(meta.detail, batch.mode, 'trace detail');
  equal(meta.clock, 'monotonic_elapsed_wall_time', 'trace clock');
  equal(meta.thread_ids, 'capture_local', 'thread identity');
  equal(meta.gpu_execution_timing, false, 'CPU timing only');
  for (const key of ['dropped_events', 'unfinished_spans', 'invalid_nesting']) {
    equal(meta[key], 0, `incomplete trace: ${key}`);
    equal(batch.capture[key], 0, `incomplete capture: ${key}`);
  }
  equal(meta.expired, false, 'unexpired trace');
  equal(batch.capture.expired, false, 'unexpired capture');
  equal(meta.unframed_events, 0, 'all workload events belong to a frame');
  equal(meta.max_events, 100000, 'same event budget');
  equal(meta.max_duration_ms, 30000, 'same deadline');
  nonnegative(meta.elapsed_us, 'capture elapsed');
  assert(meta.elapsed_us <= meta.max_duration_ms * 1000, 'deadline bound');
  equal(meta.events, trace.traceEvents.length, 'raw event count');
  equal(meta.events, batch.capture.events, 'capture event count');
  assert(meta.events > 0 && meta.events <= meta.max_events, 'event bound');
  const events = new Map();
  for (const [index, event] of trace.traceEvents.entries()) {
    equal(event.pid, pid, 'actual trace process');
    positive(event.tid, 'trace thread');
    equal(event.args.event_id, index + 1, 'unique event identity');
    nonnegative(event.ts, 'event time');
    assert(['X', 'C', 'i'].includes(event.ph), 'event kind');
    if (event.ph === 'X') {
      nonnegative(event.dur, 'event duration');
      equal(event.args.unfinished, false, 'complete scope');
    }
    events.set(index + 1, event);
  }
  equal(trace.gpui_frames.length, 16, 'typed frame count');
  equal(batch.capture.recorded_frame_ms.length, 16, 'recorded duration count');
  const frameIds = new Set(trace.gpui_frames.map(f => f.frame_id));
  equal(frameIds.size, 16, 'unique frame identity');
  for (const event of events.values()) {
    assert(frameIds.has(event.args.frame_id), 'event belongs to known frame');
    if (event.args.parent_id !== null) {
      const parent = events.get(event.args.parent_id);
      assert(parent?.ph === 'X' && parent.tid === event.tid &&
        parent.args.frame_id === event.args.frame_id && parent.args.event_id < event.args.event_id,
      'parent identity and frame');
    }
  }
  for (const [index, frame] of trace.gpui_frames.entries()) {
    equal(frame.ordinal, index + 1, 'frame order');
    equal(frame.name, 'Workload.Main', 'frame label');
    equal(frame.unfinished, false, 'complete frame');
    const event = events.get(frame.frame_id);
    assert(event?.args.frame && event.args.parent_id === null, 'real frame event');
    equal(event.name, frame.name, 'frame event name');
    near(event.ts, frame.start_us, 'frame start');
    near(event.dur, frame.duration_us, 'frame duration');
    near(frame.duration_us / 1000, batch.capture.recorded_frame_ms[index], 'recorded duration');
    assert(frame.duration_us / 1000 <= batch.frames[index].wall_ms + 1e-6, 'scope within wall sample');
    const calls = trace.traceEvents.filter(e => e.ph === 'X' && e.args.frame_id === frame.frame_id);
    equal(frame.calls.length, calls.length, 'all recorded calls retained');
    equal(new Set(frame.calls.map(c => c.event_id)).size, calls.length, 'unique recorded calls');
    for (const call of frame.calls) {
      const raw = events.get(call.event_id);
      assert(raw?.ph === 'X' && raw.args.frame_id === frame.frame_id, 'call event identity');
      equal([call.name, call.category, call.parent_id, call.thread, call.unfinished],
        [raw.name, raw.cat, raw.args.parent_id, raw.tid, false], 'call projection');
      near(call.inclusive_us, raw.dur, 'call duration');
      nonnegative(call.self_us, 'call self duration');
      assert(call.self_us <= call.inclusive_us + 1e-6, 'self duration bound');
    }
    for (const [name, key] of [['layout.nodes', 'layout_nodes'], ['projected.rows', 'projected_rows'],
      ['layout.passes', 'layout_passes']]) {
      const markers = frame.markers.filter(m => m.name === name);
      equal(markers.length, 1, 'one real work counter');
      equal(markers[0].value, batch.frames[index][key], 'work counter agrees with frame');
      assert(trace.traceEvents.some(e => e.ph === 'C' && e.args.frame_id === frame.frame_id &&
        e.name === name && e.args.value === markers[0].value), 'raw work counter');
    }
  }
}

// This verifies evidence completeness and work equivalence, never a speed threshold.
// The recorder remains the authority for call-tree/self-time analysis.
export function verifyWorkloads(report, loadTrace) {
  equal(report.schema_version, 1, 'workload schema');
  equal(report.scope, 'headless_public_api_cpu_workloads', 'workload scope');
  equal([report.native, report.gpu, report.physical_input], [false, false, false], 'headless CPU scope');
  positive(report.process_id, 'actual workload process');
  assert(typeof report.executable === 'string' && report.executable.length > 0, 'actual executable');
  assert(report.timing_scope?.includes('excludes setup, validation, object destruction and export'),
    'timing limitations recorded');
  assert(report.fonts.length >= 2, 'recorded font inputs');
  for (const font of report.fonts) positive(font.bytes, 'font byte length');
  equal(report.frames_per_batch, 16, 'same sample count');
  equal(report.orders, orders, 'balanced orders');
  equal(report.workloads.length, 72, 'complete workload matrix');
  const reference = new Map(), batches = new Map();
  let cursor = 0;
  for (const kind of kinds) for (const [round, order] of orders.entries()) for (const mode of order) {
    const batch = report.workloads[cursor++];
    equal([batch.workload, batch.round, batch.mode], [kind, round, mode], 'actual balanced execution');
    equal(batch.warmup_frames, 4, 'warmup');
    equal(batch.frames.length, 16, 'frame sample count');
    for (const [index, frame] of batch.frames.entries()) {
      equal(frame.step, index + 4, 'same input step');
      nonnegative(frame.wall_ms, 'wall sample');
      assert(/^[a-f0-9]{16}$/.test(frame.signature), 'geometry/work signature');
      for (const key of ['layout_nodes', 'layout_passes', 'cpu_batches', 'cpu_vertices']) positive(frame[key], key);
      assert(frame.layout_passes <= 8, 'bounded reconciliation');
      assert(Number.isSafeInteger(frame.projected_rows) && frame.projected_rows >= 0, 'projected rows');
      if (kind === 'collection') assert(frame.projected_rows > 0 && frame.projected_rows <= 28 &&
        frame.layout_nodes <= 31, 'bounded lazy collection');
      const {wall_ms, ...work} = frame;
      const key = `${kind}:${index}`;
      if (!reference.has(key)) reference.set(key, work);
      equal(work, reference.get(key), 'same geometry and work across modes/rounds');
    }
    const stats = distribution(batch.frames.map(f => f.wall_ms));
    equal(batch.wall_ms.samples, 16, 'summary count');
    for (const key of ['mean', 'p50', 'p95', 'max']) near(batch.wall_ms[key], stats[key], 'summary from raw samples');
    if (mode === 'off') equal(batch.capture, null, 'disabled capture');
    else {
      const file = `${kind}-r${round}-${mode}.trace.json`;
      equal(batch.capture.trace_file, file, 'expected local trace file');
      verifyTrace(loadTrace(file), batch, report.process_id);
    }
    batches.set(`${kind}:${round}:${mode}`, batch);
  }
  for (const kind of kinds) for (let round = 0; round < 6; round++) {
    assert(batches.get(`${kind}:${round}:detailed`).capture.events >=
      batches.get(`${kind}:${round}:stages`).capture.events, 'detail does not omit stage events');
  }
  equal(report.lifecycle, {bounded_events: 16, dropped_events: 1009, expired: true,
    idle_events_after: 0, idle_events_before: 0, recording_after_stop: false, unfinished_spans: 0},
  'bounds, idle and retirement');
  const hook = report.disabled_hook_cost;
  equal(hook.hooks_per_iteration, 4, 'disabled hook grouping');
  equal(hook.rows.length, 12, 'balanced disabled hook repetitions');
  const deltas = [];
  for (let round = 0; round < 6; round++) {
    const pair = hook.rows.slice(round * 2, round * 2 + 2);
    equal(pair.map(r => r.case), round % 2 ? ['disabled_four_hooks', 'control'] :
      ['control', 'disabled_four_hooks'], 'balanced hook execution');
    for (const row of pair) {
      equal(row.round, round, 'hook round');
      equal(row.iterations, 1000000, 'same hook iterations');
      nonnegative(row.elapsed_ns, 'raw hook duration');
    }
    deltas.push((pair.find(r => r.case === 'disabled_four_hooks').elapsed_ns -
      pair.find(r => r.case === 'control').elapsed_ns) / 4000000);
  }
  const summary = kinds.map(workload => {
    const wall_ms = Object.fromEntries(modes.map(mode => [mode, distribution(report.workloads
      .filter(b => b.workload === workload && b.mode === mode).flatMap(b => b.frames.map(f => f.wall_ms)))]));
    return {workload, wall_ms, p50_ratio_to_off: Object.fromEntries(modes.slice(1)
      .map(m => [m, wall_ms[m].p50 / wall_ms.off.p50]))};
  });
  return {scope: report.scope, frames: 1152, workloads: summary,
    disabled_hook_ns_delta_per_hook: {raw_rounds: deltas, ...distribution(deltas)}};
}

export function verifyRun(directory) {
  const read = p => JSON.parse(fs.readFileSync(p, 'utf8'));
  const sha = p => crypto.createHash('sha256').update(fs.readFileSync(p)).digest('hex');
  const receipt = read(path.join(directory, 'qualification.json'));
  equal(receipt.status, 'passed', 'successful controller');
  equal(receipt.source_unchanged, true, 'unchanged source');
  equal(receipt.source_before, receipt.source_after, 'source receipt equality');
  assert(receipt.commands.length > 0 && receipt.commands.every(c => c.exit_code === 0), 'terminal successful commands');
  const file = path.join(directory, 'measurement', 'report.json'), report = read(file);
  equal(sha(file), receipt.measurement.sha256, 'report hash');
  equal([report.process_id, report.executable, report.workloads.length],
    [receipt.measurement.pid, receipt.measurement.executable, receipt.measurement.batches], 'artifact receipt');
  assert(/^[a-f0-9]{64}$/.test(receipt.measurement.exe_sha256), 'recorded executable hash');
  equal(receipt.fonts.map(({sha256, ...font}) => font), report.fonts, 'recorded font manifest');
  assert(receipt.fonts.every(f => /^[a-f0-9]{64}$/.test(f.sha256)), 'recorded font hashes');
  return {artifact: receipt.measurement, ...verifyWorkloads(report,
    name => read(path.join(directory, 'measurement', name)))};
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  const [mode, input, extra] = process.argv.slice(2);
  assert(['--report', '--qualified-run'].includes(mode) && input && !extra,
    'Usage: node tools/verify/ui_workloads.mjs --report <report.json> | --qualified-run <run-directory>');
  const location = path.resolve(input);
  const result = mode === '--qualified-run' ? verifyRun(location) : {
    artifact_receipt_verified: false,
    ...verifyWorkloads(JSON.parse(fs.readFileSync(location, 'utf8')),
      name => JSON.parse(fs.readFileSync(path.join(path.dirname(location), name), 'utf8'))),
  };
  console.log(JSON.stringify(result, null, 2));
}
