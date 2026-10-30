// Synthetic contract fixtures only; these are not native timing measurements.
import assert from 'node:assert/strict';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

test('profile CLI loads its validators and rejects corrupt evidence', () => {
  const root = mkdtempSync(path.join(os.tmpdir(), 'vesta-profile-contract-'));
  const tool = fileURLToPath(new URL('./gpui_profile.mjs', import.meta.url));
  const log = path.join(root, 'synthetic.log');
  const result = path.join(root, 'synthetic-result.json');
  const stats = value => ({ mean: value, p50: value, p95: value, max: value });
  const names = ['readiness', 'targets', 'prepare', 'acquire', 'upload',
    'record', 'commands', 'submit', 'finish'];
  const rows = Array.from({ length: 96 }, (_, index) => ({
    sample: index + 1, native_cpu_ms: names.map(() => 0.1), host_cpu_ms: 1.2,
    finish_requested: false, prepared: 1, draw_calls: 1, pipeline_binds: 1,
    scissor_updates: 1, bitmap_uploads: 0, glyph_uploads: 0,
    bitmap_reuses: 1, glyph_reuses: 1,
  }));
  const summary = { warmup_frames: 24, samples: 96, completed_frames: 120,
    gpu_time: false, physical_input: false,
    host_cpu_ms: stats(1.2), renderer_cpu_ms: stats(0.9) };
  const lines = [
    ...rows.map(row => `native_profile_sample: ${JSON.stringify(row)}`),
    `native_profile_summary: ${JSON.stringify(summary)}`,
    ...names.map(stage => `native_profile_stage: ${JSON.stringify({ stage, cpu_ms: stats(0.1) })}`),
    'native_example_closed: fixture frames=120 retirement_pending=false',
  ];
  const receipt = { exit_code: 0, sha256_before: 'a'.repeat(64), sha256_after: 'a'.repeat(64) };
  const run = () => spawnSync(process.execPath, [tool, log, result], { encoding: 'utf8' });
  try {
    writeFileSync(log, lines.join('\n') + '\n');
    writeFileSync(result, JSON.stringify(receipt));
    const valid = run();
    assert.equal(valid.status, 0, valid.stderr);
    assert.equal(JSON.parse(valid.stdout).status, 'verified_native_cpu_samples');
    writeFileSync(result, JSON.stringify({ ...receipt, sha256_after: 'b'.repeat(64) }));
    const drift = run();
    assert.notEqual(drift.status, 0);
    assert.match(drift.stderr, /consumer artifact changed/);
    writeFileSync(result, JSON.stringify(receipt));
    const changed = readFileSync(log, 'utf8').replace('"p95":0.9', '"p95":9');
    assert.notEqual(changed, lines.join('\n') + '\n');
    writeFileSync(log, changed);
    const corrupt = run();
    assert.notEqual(corrupt.status, 0);
    assert.match(corrupt.stderr, /p95 does not match raw samples/);
  } finally {
    assert.equal(path.dirname(path.resolve(root)), path.resolve(os.tmpdir()));
    assert.ok(path.basename(root).startsWith('vesta-profile-contract-'));
    rmSync(root, { recursive: true, force: true });
  }
});
