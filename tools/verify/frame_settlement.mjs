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
for (const step of steps) {
  const within = event => event.ts >= step.ts && event.ts <= step.ts + step.dur + .01;
  const counters = data.traceEvents.filter(e => e.ph === 'C' && within(e));
  const value = name => counters.find(e => e.name === name)?.args?.value;
  const source = value('reflow.source'), index = value('reflow.index');
  const content = value('reflow.case') ?? 0;
  assert.ok([0, 1, 2].includes(content), 'known case required');
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
  if (renderer !== 1 || passes < 1 || passes > 8) failures.push({source, index, renderer, passes});
}
assert.ok(results.every(row => row.steps === 64));
assert.equal(cases.size, 1, 'one isolated case per report');
const budget_skips = data.traceEvents.filter(e => e.name === 'frame.skipped.layout_budget').length;
const result = {status: failures.length || budget_skips ? 'failed' : 'passed', windows: results,
  native_pid: nativePid,
  case: ['inputs', 'documents', 'focus'][[...cases][0]],
  failures, budget_skips, physical_input: false, gpu_or_display_latency: false};
console.log(JSON.stringify(result, null, 2));
if (result.status !== 'passed') process.exitCode = 1;
