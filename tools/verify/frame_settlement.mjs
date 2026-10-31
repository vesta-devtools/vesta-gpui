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
const attempts = data.traceEvents.filter(e => e.name === 'reflow.attempt');
const retryOnceInjections = data.traceEvents.filter(e => e.name === 'reflow.retry_once_injected').length;
assert.ok(retryOnceInjections <= 1 && (!retryOnceInjections || attempts.length > 0), 'one-shot injection requires its retained deferred attempt');
for (const attempt of attempts) {
  const within = event => event.ts >= attempt.ts && event.ts <= attempt.ts + attempt.dur + .01;
  const counters = data.traceEvents.filter(e => e.ph === 'C' && within(e));
  const value = name => counters.find(e => e.name === name)?.args?.value;
  const source = value('reflow.source'), index = value('reflow.index'), content = value('reflow.case');
  assert.ok([0, 1].includes(source) && Number.isInteger(index) && index >= 0 && index < 64 && [0, 1, 2, 6].includes(content), 'ordinary deferred attempt identity required');
  assert.equal(data.traceEvents.filter(e => e.name === 'reflow.await_submission' && within(e)).length, 1, 'deferred attempt must remain explicit');
  const frames = data.gpui_frames.filter(f => f.start_us >= attempt.ts && f.start_us + f.duration_us <= attempt.ts + attempt.dur + .01);
  assert.equal(frames.length, 1, 'one CPU draw per deferred attempt');
  assert.equal(frames[0].calls.filter(c => c.name === 'renderer.attempt').length, 1, 'defer after a renderer attempt');
  const passes = frames[0].calls.filter(c => c.name === 'layout.pass').length;
  assert.ok(passes >= 1 && passes <= 8, 'deferred attempt still needs complete bounded CPU preparation');
  assert.ok(!frames[0].markers.some(m => m.name === 'present.submitted'), 'a submitted frame must not be hidden as a deferred attempt');
  const next = steps.find(step => step.ts > attempt.ts + attempt.dur);
  assert.ok(next, 'deferred attempt needs a later accepted step');
  const own = data.traceEvents.filter(e => e.ph === 'C' && e.ts >= next.ts && e.ts <= next.ts + next.dur + .01);
  assert.deepEqual([own.find(e => e.name === 'reflow.source')?.args?.value,
    own.find(e => e.name === 'reflow.index')?.args?.value,
    own.find(e => e.name === 'reflow.case')?.args?.value], [source, index, content],
  'the next accepted step must retain the deferred logical identity');
}
const cases = new Set();
let cpuReadinessSamples = 0;
let cursorSamples = 0;
let cancellationSamples = 0;
for (const step of steps) {
  const within = event => event.ts >= step.ts && event.ts <= step.ts + step.dur + .01;
  const counters = data.traceEvents.filter(e => e.ph === 'C' && within(e));
  const value = name => counters.find(e => e.name === name)?.args?.value;
  const source = value('reflow.source'), index = value('reflow.index');
  const content = value('reflow.case') ?? 0;
  assert.ok([0, 1, 2, 3, 4, 5, 6].includes(content), 'known case required');
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
  const budget = content === 4 || content === 5;
  if (renderer !== (budget ? 0 : 1) || passes < 1 || passes > 8) failures.push({source, index, renderer, passes});
  if (content === 3 || budget) {
    if (content === 3) {
      assert.equal(value('reflow.geometry_agrees'), 1, 'deferred text and current hit bounds must agree');
      if (value('reflow.cpu_input_ready') !== undefined) {
        assert.equal(value('reflow.cpu_input_ready'), 1, 'stable CPU input remains usable during acquire deferral');
        cpuReadinessSamples++;
      }
    }
    if (budget) {
      assert.equal(passes, 1, 'isolated reduced budget required');
      assert.equal(data.traceEvents.filter(e => e.name === 'frame.skipped.layout_budget' && within(e)).length, 1);
      for (const name of ['reflow.input_blocked', 'reflow.capture_released', 'reflow.command_blocked', 'reflow.logical_blocked', 'reflow.lifecycle_allowed', 'reflow.focus_preserved']) assert.equal(value(name), 1, name);
      if (value('reflow.cursor_cached') !== undefined) {
        assert.equal(value('reflow.cursor_cached'), 1, 'blocked motion must preserve the latest cursor position');
        cursorSamples++;
      }
    }
    if (content === 5) {
      for(const name of ['reflow.cancel_click_scoped','reflow.cancel_value_restored','reflow.cancel_other_pending'])assert.equal(value(name),1,name);
      cancellationSamples++;
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
let captureOutcomes;
if (cases.has(6)) {
  const captures = data.traceEvents.filter(e => e.name === 'reflow.capture_result');
  assert.equal(captures.length, 4, 'both windows need a rejected and a fresh capture');
  captureOutcomes = [[], []];
  for (const capture of captures) {
    const counters = data.traceEvents.filter(e => e.ph === 'C'
      && e.ts >= capture.ts && e.ts <= capture.ts + capture.dur + .01);
    const source = counters.find(e => e.name === 'reflow.source')?.args?.value;
    const outcome = counters.find(e => e.name === 'reflow.capture_outcome')?.args?.value;
    assert.ok([0, 1].includes(source), 'capture window identity required');
    captureOutcomes[source].push(outcome);
  }
  assert.deepEqual(captureOutcomes, [[0, 1], [0, 1]],
    'a rejected presentation must fail capture before a fresh request succeeds');
}
assert.ok(cpuReadinessSamples === 0 || cpuReadinessSamples === 128, 'readiness observations must be complete when supplied');
assert.ok(cursorSamples === 0 || cursorSamples === 128, 'cursor observations must be complete when supplied');
const budget_skips = data.traceEvents.filter(e => e.name === 'frame.skipped.layout_budget').length;
const result = {status: failures.length || (budget_skips && !cases.has(4) && !cases.has(5)) ? 'failed' : 'passed', windows: results,
  native_pid: nativePid,
  deferred_attempts: attempts.length,
  retry_once_injections: retryOnceInjections,
  ...(captureOutcomes ? {capture_outcomes: captureOutcomes} : {}),
  case: ['inputs', 'documents', 'focus', 'deferred', 'budget', 'cancellation', 'capture'][[...cases][0]],
  failures, budget_skips, cpu_input_readiness_observed: cpuReadinessSamples === 128,
  cursor_cache_observed: cursorSamples === 128,
  cancellation_observed: cancellationSamples === 128,
  physical_input: false, gpu_or_display_latency: false};
console.log(JSON.stringify(result, null, 2));
if (result.status !== 'passed') process.exitCode = 1;
