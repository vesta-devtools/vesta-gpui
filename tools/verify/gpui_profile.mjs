// Validate actual finite native samples before publishing CPU percentile evidence.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { verifyStandaloneProfile } from './gpui_profile/standalone.mjs';
import { verifyGpuProfile } from './gpui_profile/gpu.mjs';
import { verifyLifecycleProfile } from './gpui_profile/lifecycle.mjs';
import { correlateNativeInputGpu } from './gpui_profile/native_input.mjs';

const [log, outcome, ...extra] = process.argv.slice(2);
if (!log || !outcome || extra.length) throw new Error('Usage: node tools/verify/gpui_profile.mjs <stdout.log> <result.json>');
const decode = bytes => new TextDecoder('utf-8', { fatal: true }).decode(bytes);
const lines = decode(readFileSync(log)).split(/\r?\n/);
const records = prefix => lines.filter(line => line.startsWith(prefix)).map(line => JSON.parse(line.slice(prefix.length)));
const rows = records('native_profile_sample: ');
const summaries = records('native_profile_summary: ');
const stages = records('native_profile_stage: ');
const result = JSON.parse(decode(readFileSync(outcome)));
const hostCpu = lines.some(line => line.startsWith('ui_profile '));
const hostGpu = lines.some(line => line.startsWith('ui_gpu_capabilities '));
const hostLifecycle = lines.some(line => line.startsWith('ui_lifecycle_exit '));
if (hostCpu || hostGpu || hostLifecycle) {
  const verified = hostCpu ? verifyStandaloneProfile(lines, result) : {
    status: 'verified_standalone_diagnostic_observations',
    artifact_sha256: result.sha256_before,
    cpu_distribution_available: false,
    cpu_percentiles: null,
  };
  if (hostGpu) {
    verified.gpu = verifyGpuProfile(lines, result);
  }
  if (hostLifecycle) {
    verified.lifecycle = verifyLifecycleProfile(lines, result);
  }
  if (hostGpu && hostLifecycle) {
    verified.native_input_gpu = correlateNativeInputGpu(lines, verified.lifecycle, verified.gpu);
  }
  console.log(JSON.stringify(verified));
  process.exit(0);
}
assert.equal(result.exit_code, 0, 'native process must exit successfully');
assert.match(result.sha256_before, /^[A-Fa-f0-9]{64}$/);
assert.equal(result.sha256_before, result.sha256_after, 'consumer artifact changed');
assert.equal(summaries.length, 1);
const summary = summaries[0];
assert.equal(summary.warmup_frames, 24);
assert.equal(summary.samples, 96);
assert.equal(summary.completed_frames, 120);
assert.equal(summary.gpu_time, false);
assert.equal(summary.physical_input, false);
assert.equal(rows.length, summary.samples);
const names = ['readiness', 'targets', 'prepare', 'acquire', 'upload', 'record', 'commands', 'submit', 'finish'];
assert.deepEqual(stages.map(stage => stage.stage), names);
const totals = [];
for (const [index, row] of rows.entries()) {
  assert.equal(row.sample, index + 1);
  assert.equal(row.native_cpu_ms.length, names.length);
  assert.equal(row.finish_requested, false, 'profile must not force synchronous GPU finish');
  for (const value of [row.host_cpu_ms, ...row.native_cpu_ms]) {
    assert.ok(Number.isFinite(value) && value >= 0, 'invalid CPU sample');
  }
  const native = row.native_cpu_ms.reduce((sum, value) => sum + value, 0);
  assert.ok(row.host_cpu_ms + 0.000001 >= native, 'nested native intervals exceed host work');
  totals.push(native);
}
function verifyStats(values, reported) {
  const ordered = [...values].sort((a, b) => a - b);
  const expected = { mean: values.reduce((sum, v) => sum + v, 0) / values.length,
    p50: ordered[Math.ceil(0.50 * ordered.length) - 1],
    p95: ordered[Math.ceil(0.95 * ordered.length) - 1], max: ordered.at(-1) };
  for (const [name, value] of Object.entries(expected)) {
    assert.ok(Number.isFinite(reported[name]) && Math.abs(value - reported[name]) <= 0.000001,
      `${name} does not match raw samples`);
  }
}
verifyStats(rows.map(row => row.host_cpu_ms), summary.host_cpu_ms);
verifyStats(totals, summary.renderer_cpu_ms);
stages.forEach((stage, index) => verifyStats(rows.map(row => row.native_cpu_ms[index]), stage.cpu_ms));
assert.ok(lines.some(line => /^native_example_closed: \w+ frames=120 retirement_pending=false$/.test(line)),
  'normal exit/retirement result missing');
const range = key => { const values = rows.map(row => row[key]); return [Math.min(...values), Math.max(...values)]; };
console.log(JSON.stringify({ status: 'verified_native_cpu_samples', artifact_sha256: result.sha256_before,
  ...summary, prepared_range: range('prepared'), draw_calls_range: range('draw_calls'),
  pipeline_binds_range: range('pipeline_binds'), scissor_updates_range: range('scissor_updates'),
  bitmap_uploads: rows.reduce((sum, row) => sum + row.bitmap_uploads, 0),
  glyph_uploads: rows.reduce((sum, row) => sum + row.glyph_uploads, 0),
  bitmap_reuses_range: range('bitmap_reuses'), glyph_reuses_range: range('glyph_reuses') }));
