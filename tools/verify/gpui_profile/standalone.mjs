// Verifies existing Standalone CPU evidence; never starts a process or sends input.
import assert from 'node:assert/strict';

const stages = ['build', 'route', 'layout', 'reconcile', 'paint', 'hit',
  'renderer', 'text_views', 'accessibility', 'ime'];
const nativeStages = ['readiness', 'targets', 'prepare', 'acquire', 'upload',
  'record', 'commands', 'submit', 'finish'];
const finite = (value, name) => assert.ok(Number.isFinite(value) && value >= 0, `invalid ${name}`);
function percentile(values, fraction) {
  if (!values.length) return null;
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.ceil(fraction * sorted.length) - 1] / 1_000_000;
}
function verifyReported(values, reported, name) {
  if (!values.length) {
    assert.equal(reported, undefined, `${name} must be absent for unavailable renderer samples`);
    return;
  }
  finite(reported, name);
  assert.ok(Math.abs(reported - percentile(values, name.endsWith('p50_ms') ? 0.5 : 0.95)) <= 0.000001,
    `${name} does not match raw samples`);
}
function parse(line) {
  const fields = [...line.matchAll(/(?:^|\s)([a-z0-9_]+)=([^\s]+)/g)];
  assert.equal(new Set(fields.map(match => match[1])).size, fields.length, 'duplicate profile field');
  return Object.fromEntries(fields.map(([, key, raw]) => {
    const value = raw.startsWith('[') ? JSON.parse(raw) : raw === 'null' ? null
      : raw === 'true' ? true : raw === 'false' ? false
        : Number.isFinite(Number(raw)) ? Number(raw) : raw;
    return [key, value];
  }));
}
export function verifyStandaloneProfile(lines, result) {
  assert.equal(result.terminal, true, 'native process must be terminal');
  assert.equal(result.code, 0, 'native process must exit successfully');
  assert.match(result.sha256_before, /^[A-Fa-f0-9]{64}$/);
  assert.equal(result.sha256_before, result.sha256_after, 'consumer artifact changed');
  assert.deepEqual(result.drift, [], 'source/font inputs changed during the run');
  const retirementMarker = lines.some(line => line ===
    'UI-R07 exit: all GPU owners retired on worker; pending=false');
  assert.ok(Array.isArray(result.args), 'native launch arguments required');
  if (result.args.includes('--smoke')) {
    assert.ok(retirementMarker, 'smoke GPU retirement result missing');
  }
  const lifecycle = lines.filter(line => line.startsWith('Standalone frame lifecycle: '));
  assert.equal(lifecycle.length, 1, 'one native lifecycle required');
  const match = lifecycle[0].match(/^Standalone frame lifecycle: (\d+) completed main frames$/);
  assert.ok(match, 'invalid native lifecycle count');
  const profiles = lines.filter(line => line.startsWith('ui_profile ')).map(parse);
  assert.ok(profiles.length > 0 && profiles.length <= 64, 'invalid context bound');
  const contexts = new Set();
  let profiledMainFrames = 0;
  const summaries = profiles.map(profile => {
    assert.ok(Number.isSafeInteger(profile.window) && profile.window > 0, 'invalid window ID');
    assert.ok(['2d', '3d'].includes(profile.mode), 'unknown mode');
    assert.equal(typeof profile.text_focus, 'boolean');
    assert.equal(typeof profile.active, 'boolean');
    const context = [profile.window, profile.mode, profile.text_focus, profile.active].join('/');
    assert.ok(!contexts.has(context), 'duplicate frame context');
    contexts.add(context);
    assert.equal(profile.warmup, 24);
    assert.equal(profile.distribution_limit, 256);
    assert.equal(profile.distribution_window, 'first_completed_post_warmup');
    assert.ok(Number.isSafeInteger(profile.frames) && profile.frames > 0, 'invalid frame count');
    if (profile.window === 1) profiledMainFrames += profile.frames + profile.warmup;
    const host = profile.cpu_samples_ns, native = profile.native_cpu_samples_ns;
    assert.ok(Array.isArray(host) && Array.isArray(native), 'raw sample arrays required');
    assert.equal(host.length, Math.min(profile.frames, 256));
    assert.equal(profile.cpu_distribution_frames, host.length);
    assert.equal(native.length, host.length, 'renderer nulls must retain frame positions');
    host.forEach((value, index) => {
      assert.ok(Number.isSafeInteger(value) && value >= 0, 'invalid host duration');
      const nested = native[index];
      if (nested !== null) {
        assert.ok(Number.isSafeInteger(nested) && nested >= 0, 'invalid nested renderer duration');
        assert.ok(nested <= value, 'nested renderer interval exceeds host frame');
      }
    });
    const renderer = native.filter(value => value !== null);
    assert.equal(profile.native_cpu_distribution_frames, renderer.length);
    verifyReported(host, profile.cpu_p50_ms, 'cpu_p50_ms');
    verifyReported(host, profile.cpu_p95_ms, 'cpu_p95_ms');
    verifyReported(renderer, profile.native_cpu_p50_ms, 'native_cpu_p50_ms');
    verifyReported(renderer, profile.native_cpu_p95_ms, 'native_cpu_p95_ms');
    if (renderer.length || profile.native_frames !== undefined) {
      assert.ok(Number.isSafeInteger(profile.native_frames) && profile.native_frames > 0,
        'invalid all-frame renderer availability');
      assert.ok(profile.native_frames >= renderer.length &&
        profile.native_frames <= profile.frames - host.length + renderer.length,
      'all-frame renderer availability contradicts the first sample window');
      assert.ok(Number.isSafeInteger(profile.native_finish_frames) &&
        profile.native_finish_frames >= 0 && profile.native_finish_frames <= profile.native_frames,
      'invalid renderer finish count');
      finite(profile.native_cpu_mean_ms, 'renderer all-frame mean');
      let nativeMean = 0;
      for (const stage of nativeStages) {
        const mean = profile[`native_${stage}_mean_ms`], max = profile[`native_${stage}_max_ms`];
        finite(mean, `native ${stage} mean`);
        finite(max, `native ${stage} max`);
        assert.ok(max >= mean, `native ${stage} max below mean`);
        nativeMean += mean;
      }
      assert.ok(Math.abs(nativeMean - profile.native_cpu_mean_ms) <= 0.00001,
        'nested renderer stage means do not sum to the native mean');
    }
    finite(profile.cpu_mean_ms, 'cpu mean');
    finite(profile.cpu_max_ms, 'cpu max');
    assert.ok(profile.cpu_max_ms >= profile.cpu_mean_ms, 'host max below mean');
    assert.ok(host.every(value => value / 1_000_000 <= profile.cpu_max_ms + 0.000001),
      'all-frame host max excludes a raw sample');
    let stageMean = 0;
    for (const stage of stages) {
      const mean = profile[`${stage}_mean_ms`], max = profile[`${stage}_max_ms`];
      finite(mean, `${stage} mean`);
      finite(max, `${stage} max`);
      assert.ok(max >= mean, `${stage} max below mean`);
      stageMean += mean;
    }
    assert.ok(Math.abs(stageMean - profile.cpu_mean_ms) <= 0.00001,
      'host stage means do not sum to completed-frame mean');
    return { window: profile.window, mode: profile.mode, text_focus: profile.text_focus,
      active: profile.active, completed_post_warmup_frames: profile.frames,
      first_distribution_frames: host.length, renderer_available_frames: renderer.length,
      host_cpu_p50_ms: profile.cpu_p50_ms, host_cpu_p95_ms: profile.cpu_p95_ms,
      renderer_cpu_p50_ms: profile.native_cpu_p50_ms ?? null,
      renderer_cpu_p95_ms: profile.native_cpu_p95_ms ?? null,
      all_frame_stage_means_ms: Object.fromEntries(stages.map(stage => [stage, profile[`${stage}_mean_ms`]])) };
  });
  assert.ok(profiledMainFrames <= Number(match[1]), 'context counts exceed completed main lifecycle');
  return { status: 'verified_standalone_cpu_samples', artifact_sha256: result.sha256_before,
    completed_main_frames: Number(match[1]), contexts: summaries,
    gpu_retirement: retirementMarker ? 'explicit_smoke_marker' : 'not_separately_logged',
    gpu_time: false, physical_input: false, input_latency: false,
    scope: 'CPU sample integrity only; scene/controller completion and physical input require separate evidence' };
}
