import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import {verifyRun, verifyWorkloads} from './ui_workloads.mjs';

// Synthetic complete envelopes exercise corruption rejection; they are never
// performance/native evidence and do not reimplement the recorder's analyzer.
function fixture() {
  const modes = ['off', 'stages', 'detailed'];
  const orders = [modes, ['detailed', 'stages', 'off'], ['stages', 'off', 'detailed'],
    ['detailed', 'off', 'stages'], ['off', 'detailed', 'stages'], ['stages', 'detailed', 'off']];
  const report = {schema_version: 1, scope: 'headless_public_api_cpu_workloads',
    native: false, gpu: false, physical_input: false, process_id: 42, executable: 'fixture.exe',
    timing_scope: 'excludes setup, validation, object destruction and export',
    fonts: [{path: 'ui.ttf', bytes: 100}, {path: 'cjk.ttc', bytes: 200}],
    frames_per_batch: 16, orders, workloads: [], lifecycle: {bounded_events: 16,
      dropped_events: 1009, expired: true, idle_events_after: 0, idle_events_before: 0,
      recording_after_stop: false, unfinished_spans: 0},
    disabled_hook_cost: {hooks_per_iteration: 4, rows: []}};
  const traces = {};
  for (const kind of ['static', 'resize', 'edit', 'collection']) {
    for (const [round, order] of orders.entries()) for (const mode of order) {
      const frames = Array.from({length: 16}, (_, i) => ({step: i + 4, wall_ms: .2,
        signature: '0123456789abcdef', layout_nodes: 8, layout_passes: 1,
        cpu_batches: 4, cpu_vertices: 24, projected_rows: kind === 'collection' ? 5 : 0}));
      const batch = {workload: kind, round, mode, warmup_frames: 4, frames,
        wall_ms: {samples: 16, mean: .2, p50: .2, p95: .2, max: .2}, capture: null};
      if (mode !== 'off') {
        const traceEvents = [], typed = [];
        for (const [index, frame] of frames.entries()) {
          const id = traceEvents.length + 1, ts = index * 200;
          traceEvents.push({pid: 42, tid: 1, ts, dur: 100, ph: 'X', cat: 'gpui.frame',
            name: 'Workload.Main', args: {event_id: id, parent_id: null, frame_id: id,
              frame: true, unfinished: false}});
          const markers = [['layout.nodes', frame.layout_nodes], ['projected.rows', frame.projected_rows],
            ['layout.passes', frame.layout_passes]].map(([name, value]) => ({name, value, at_us: ts + 1}));
          for (const marker of markers) traceEvents.push({pid: 42, tid: 1, ts: marker.at_us,
            ph: 'C', cat: 'workload', name: marker.name, args: {event_id: traceEvents.length + 1,
              frame_id: id, parent_id: id, frame: false, value: marker.value}});
          typed.push({ordinal: index + 1, frame_id: id, name: 'Workload.Main', thread: 1,
            start_us: ts, duration_us: 100, unfinished: false, markers,
            calls: [{event_id: id, parent_id: null, name: 'Workload.Main', category: 'gpui.frame',
              thread: 1, start_us: ts, inclusive_us: 100, self_us: 100, unfinished: false}]});
        }
        const file = `${kind}-r${round}-${mode}.trace.json`;
        batch.capture = {trace_file: file, events: 64, dropped_events: 0, unfinished_spans: 0,
          invalid_nesting: 0, expired: false, recorded_frame_ms: frames.map(() => .1)};
        traces[file] = {traceEvents, gpui_frames: typed, gpui_trace: {schema_version: 2,
          detail: mode, clock: 'monotonic_elapsed_wall_time', thread_ids: 'capture_local',
          gpu_execution_timing: false, dropped_events: 0, unfinished_spans: 0,
          invalid_nesting: 0, expired: false, unframed_events: 0, max_events: 100000,
          max_duration_ms: 30000, elapsed_us: 3200, events: 64}};
      }
      report.workloads.push(batch);
    }
  }
  for (let round = 0; round < 6; round++) {
    for (const name of round % 2 ? ['disabled_four_hooks', 'control'] : ['control', 'disabled_four_hooks']) {
      report.disabled_hook_cost.rows.push({round, case: name, iterations: 1000000,
        elapsed_ns: name === 'control' ? 2000000 : 6000000});
    }
  }
  return {report, traces, verify() { return verifyWorkloads(this.report, f => this.traces[f]); }};
}

test('complete evidence retains all samples and rejects work/order/statistic substitution', () => {
  const good = fixture().verify();
  assert.equal(good.frames, 1152);
  assert.equal(good.workloads[0].wall_ms.off.samples, 96);
  assert.equal(good.disabled_hook_ns_delta_per_hook.p50, 1);
  for (const corrupt of [
    x => x.report.workloads.pop(),
    x => x.report.workloads.reverse(),
    x => x.report.workloads[1].frames[0].signature = 'abcdef0123456789',
    x => x.report.workloads[1].frames[0].cpu_vertices++,
    x => x.report.workloads[0].wall_ms.p95 = .01,
    x => x.report.workloads.at(-1).frames[0].projected_rows = 100000,
    x => x.report.disabled_hook_cost.rows.pop(),
    x => x.report.lifecycle.recording_after_stop = true,
  ]) {
    const data = fixture(); corrupt(data);
    assert.throws(() => data.verify());
  }
});

test('capture evidence rejects missing data, false process identity and typed/raw disagreement', () => {
  for (const corrupt of [
    x => x.report.workloads[1].capture.trace_file = '../foreign.json',
    x => x.traces['static-r0-stages.trace.json'].gpui_trace.dropped_events = 1,
    x => x.traces['static-r0-stages.trace.json'].gpui_trace.detail = 'detailed',
    x => x.traces['static-r0-stages.trace.json'].traceEvents[0].pid++,
    x => x.traces['static-r0-stages.trace.json'].traceEvents.pop(),
    x => x.traces['static-r0-stages.trace.json'].gpui_frames[0].calls.pop(),
    x => x.traces['static-r0-stages.trace.json'].gpui_frames[0].markers[0].value++,
    x => x.traces['static-r0-stages.trace.json'].gpui_frames[0].duration_us = 50,
    x => x.traces['static-r0-stages.trace.json'].traceEvents[1].args.parent_id = 99999,
  ]) {
    const data = fixture(); corrupt(data);
    assert.throws(() => data.verify());
  }
});

test('disk evidence binds the terminal command, unchanged source, report and artifact receipts', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'gpui-workload-verifier-'));
  try {
    const {report, traces} = fixture();
    fs.mkdirSync(path.join(dir, 'measurement'));
    const bytes = JSON.stringify(report);
    fs.writeFileSync(path.join(dir, 'measurement/report.json'), bytes);
    for (const [name, value] of Object.entries(traces)) fs.writeFileSync(path.join(dir, 'measurement', name), JSON.stringify(value));
    const receipt = {status: 'passed', source_unchanged: true, source_before: [{path: 'fixture', sha256: 'a'.repeat(64)}],
      source_after: [{path: 'fixture', sha256: 'a'.repeat(64)}], commands: [{exit_code: 0}],
      measurement: {pid: 42, executable: 'fixture.exe', batches: 72,
        sha256: crypto.createHash('sha256').update(bytes).digest('hex'), exe_sha256: 'b'.repeat(64)},
      fonts: report.fonts.map(f => ({...f, sha256: 'c'.repeat(64)}))};
    const save = value => fs.writeFileSync(path.join(dir, 'qualification.json'), JSON.stringify(value));
    save(receipt);
    assert.equal(verifyRun(dir).frames, 1152);
    for (const change of [r => r.status = 'running', r => r.commands[0].exit_code = null,
      r => r.source_after[0].sha256 = 'd'.repeat(64), r => r.measurement.pid++,
      r => r.measurement.sha256 = 'e'.repeat(64)]) {
      const bad = structuredClone(receipt); change(bad); save(bad);
      assert.throws(() => verifyRun(dir));
    }
  } finally {
    fs.rmSync(dir, {recursive: true});
  }
});
