// Verify the explicit hidden Main/Float workload, not physical input or display latency.
import fs from 'node:fs';
import assert from 'node:assert/strict';

const report = process.argv[2];
assert.ok(report, 'usage: node tools/verify/frame_settlement.mjs <report.html>');
const html = new TextDecoder('utf8', {fatal: true}).decode(fs.readFileSync(report));
const payload = html.match(/<script id="gpui-data" type="application\/json">([\s\S]*?)<\/script>/);
assert.ok(payload, 'shared GPUI frame report required');
const data = JSON.parse(payload[1]);
assert.equal(data.gpui_trace.schema_version, 2);
for (const field of ['dropped_events', 'unfinished_spans', 'invalid_nesting']) {
  assert.equal(data.gpui_trace[field], 0, 'incomplete capture: ' + field);
}
assert.equal(new Set(data.traceEvents.map(e => e.pid)).size, 1);
const nativePid = data.traceEvents[0]?.pid;
if (process.argv[3] !== undefined) {
  const expected = Number(process.argv[3]);
  assert.ok(Number.isSafeInteger(expected) && expected > 0, 'valid owned native PID required');
  assert.equal(nativePid, expected, 'report must belong to the launched native process');
}
const steps = data.traceEvents.filter(e => e.name === 'reflow.step');
assert.equal(steps.length, 128, 'all Main and Float forward/reverse steps required');
const results = [{window: 'Main', steps: 0, renderer: 0, submitted: 0, max_passes: 0},
  {window: 'Float', steps: 0, renderer: 0, submitted: 0, max_passes: 0}];
const failures = [];
const cases = new Set();
let cpuReadinessSamples = 0;
let cursorSamples = 0;
for (const step of steps) {
  const within = event => event.ts >= step.ts && event.ts <= step.ts + step.dur + .01;
  const counters = data.traceEvents.filter(e => e.ph === 'C' && within(e));
  const value = name => counters.find(e => e.name === name)?.args?.value;
  const source = value('reflow.source'), index = value('reflow.index');
  const content = value('reflow.case') ?? 0;
  assert.ok([0, 1, 2, 3, 4].includes(content), 'known case required');
  cases.add(content);
  assert.ok(source === 0 || source === 1, 'source counter required');
  const row = results[source];
  assert.equal(index, row.steps, 'per-window sequence must be complete and ordered');
  row.steps++;
  const frames = data.gpui_frames.filter(frame => frame.start_us >= step.ts
    && frame.start_us + frame.duration_us <= step.ts + step.dur + .01);
  assert.equal(frames.length, 1, 'one CPU draw per workload step');
  const [frame] = frames;
  assert.equal(frame.name, 'Standalone.' + row.window);
  const renderer = frame.calls.filter(call => call.name === 'renderer.attempt').length;
  const passes = frame.calls.filter(call => call.name === 'layout.pass').length;
  row.renderer += renderer;
  row.submitted += Number(frame.markers.some(marker => marker.name === 'present.submitted'));
  row.max_passes = Math.max(row.max_passes, passes);
  if (renderer !== (content === 4 ? 0 : 1) || passes < 1 || passes > 8) failures.push({source, index, renderer, passes});
  if (content === 3 || content === 4) {
    if (content === 3) {
      assert.equal(value('reflow.geometry_agrees'), 1, 'deferred text and current hit bounds must agree');
      if (value('reflow.cpu_input_ready') !== undefined) {
        assert.equal(value('reflow.cpu_input_ready'), 1, 'stable CPU input remains usable during acquire deferral');
        cpuReadinessSamples++;
      }
    }
    if (content === 4) {
      assert.equal(passes, 1, 'isolated reduced budget required');
      assert.equal(data.traceEvents.filter(e => e.name === 'frame.skipped.layout_budget' && within(e)).length, 1);
      for (const name of ['reflow.input_blocked', 'reflow.capture_released', 'reflow.command_blocked', 'reflow.logical_blocked', 'reflow.lifecycle_allowed', 'reflow.focus_preserved']) assert.equal(value(name), 1, name);
      if (value('reflow.cursor_cached') !== undefined) {
        assert.equal(value('reflow.cursor_cached'), 1, 'blocked motion must preserve the latest cursor position');
        cursorSamples++;
      }
    }
    assert.equal(value('reflow.presented_unchanged'), 1, 'deferred CPU preparation cannot publish a presented snapshot');
    assert.ok(!frame.markers.some(marker => marker.name === 'present.submitted'), 'deferred draw cannot submit');
    const next = steps[steps.indexOf(step) + 1];
    assert.ok(data.gpui_frames.some(recovery => recovery.name === frame.name
      && recovery.start_us > frame.start_us + frame.duration_us
      && (!next || recovery.start_us < next.ts)
      && recovery.markers.some(marker => marker.name === 'present.submitted')), 'each deferred step must recover outside its draw');
  } else if (!frame.markers.some(marker => marker.name === 'present.submitted')) {
    failures.push({source, index, reason: 'missing successful submission'});
  }
}
assert.ok(results.every(row => row.steps === 64));
assert.equal(cases.size, 1, 'one isolated case per report');
assert.ok(cpuReadinessSamples === 0 || cpuReadinessSamples === 128, 'readiness observations must be complete when supplied');
assert.ok(cursorSamples === 0 || cursorSamples === 128, 'cursor observations must be complete when supplied');
const budget_skips = data.traceEvents.filter(e => e.name === 'frame.skipped.layout_budget').length;
const result = {status: failures.length || (budget_skips && !cases.has(4)) ? 'failed' : 'passed', windows: results,
  native_pid: nativePid,
  case: ['inputs', 'documents', 'focus', 'deferred', 'budget'][[...cases][0]],
  failures, budget_skips, cpu_input_readiness_observed: cpuReadinessSamples === 128,
  cursor_cache_observed: cursorSamples === 128,
  physical_input: false, gpu_or_display_latency: false};
console.log(JSON.stringify(result, null, 2));
if (result.status !== 'passed') process.exitCode = 1;
