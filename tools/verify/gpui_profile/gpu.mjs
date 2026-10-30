// ADR-0577: validate raw device timestamps without rounding u64 ticks in JSON.
import assert from 'node:assert/strict';

const integer = (value, name) => {
  assert.equal(typeof value, 'string', `${name} must preserve integer precision`);
  assert.match(value, /^(0|[1-9]\d*)$/, `invalid ${name}`);
  const decoded = BigInt(value);
  assert.ok(decoded <= 0xffffffffffffffffn, `${name} exceeds u64`);
  return decoded;
};
const key = record => `${integer(record.generation, 'generation')}/${integer(record.submission, 'submission')}`;
const count = (value, name) => assert.ok(Number.isSafeInteger(value) && value >= 0, `invalid ${name}`);
const finite = (value, name) => assert.ok(Number.isFinite(value) && value > 0, `invalid ${name}`);
const percentile = (values, fraction) => values.length
  ? [...values].sort((a, b) => a - b)[Math.ceil(values.length * fraction) - 1] / 1_000_000 : null;

export function verifyGpuProfile(lines, result) {
  assert.ok(result.args.includes('--profile-gpu'), 'GPU flag absent from actual launch');
  assert.equal(result.terminal, true);
  assert.equal(result.code, 0);
  assert.match(result.sha256_before, /^[A-Fa-f0-9]{64}$/);
  assert.deepEqual(result.drift, []);
  assert.equal(result.sha256_before, result.sha256_after);
  const records = prefix => lines.filter(line => line.startsWith(prefix))
    .map(line => JSON.parse(line.slice(prefix.length)));
  const capabilities = records('ui_gpu_capabilities ');
  const observations = records('ui_gpu_profile ');
  const unresolved = records('ui_gpu_unresolved ');
  const summaries = records('ui_gpu_summary ');
  assert.equal(summaries.length, 1, 'one final GPU summary required');
  assert.ok(capabilities.length > 0 && capabilities.length <= 64);
  const caps = new Map();
  for (const capability of capabilities) {
    integer(capability.generation, 'generation');
    assert.equal(capability.schema, 1);
    assert.equal(caps.has(capability.generation), false, 'duplicate GPU generation');
    if (capability.unavailable === null) {
      assert.ok(Number.isInteger(capability.valid_bits) && capability.valid_bits >= 1 && capability.valid_bits <= 64);
      finite(capability.period_ns, 'period_ns');
    }
    caps.set(capability.generation, capability);
  }
  const seen = new Set(), ordinals = new Set(), groups = new Map();
  let samples = 0, unavailable = 0, missing = 0;
  for (const record of [...observations, ...unresolved]) {
    assert.equal(record.schema, 1);
    const identity = key(record);
    assert.equal(seen.has(identity), false, 'duplicate observation identity');
    seen.add(identity);
    const capability = caps.get(record.generation);
    assert.ok(capability, 'unknown GPU generation');
    if (record.context === null) {
      missing++;
    } else {
      const context = record.context;
      const ordinal = integer(context.host_attempt, 'host_attempt');
      assert.equal(ordinals.has(String(ordinal)), false, 'Host context attached twice');
      ordinals.add(String(ordinal));
      integer(context.window, 'window');
      assert.ok(['2d', '3d'].includes(context.mode));
      assert.equal(typeof context.active, 'boolean');
      assert.equal(typeof context.text_focus, 'boolean');
      assert.equal(context.logical_size.length, 2);
      context.logical_size.forEach(value => finite(value, 'logical_size'));
      finite(context.scale_factor, 'scale_factor');
      assert.ok(['presented', 'deferred', 'failed'].includes(context.render_outcome.status));
      const groupKey = JSON.stringify([context.window, context.mode, context.text_focus,
        context.active, context.logical_size, context.scale_factor]);
      if (!groups.has(groupKey)) groups.set(groupKey, []);
      groups.get(groupKey).push({ ordinal, record });
    }
    if (record.status === 'available') {
      samples++;
      assert.equal(record.valid_bits, capability.valid_bits);
      assert.equal(record.period_ns, capability.period_ns);
      assert.equal(record.raw_ticks.length, 2);
      const start = integer(record.raw_ticks[0], 'raw_ticks[0]');
      const end = integer(record.raw_ticks[1], 'raw_ticks[1]');
      const elapsed = integer(record.elapsed_ns, 'elapsed_ns');
      const upper = integer(record.completion_upper_bound_ns, 'completion_upper_bound_ns');
      const mask = (1n << BigInt(record.valid_bits)) - 1n;
      const decoded = Number((end - start) & mask) * record.period_ns;
      assert.ok(Math.abs(decoded - Number(elapsed)) <= 1, 'GPU elapsed differs from raw ticks');
      assert.ok(elapsed <= upper, 'GPU span exceeds submission/completion upper bound');
      assert.ok(Number(upper) < 2 ** record.valid_bits * record.period_ns, 'ambiguous counter wrap');
    } else {
      assert.equal(record.status, 'unknown');
      assert.equal(record.elapsed_ns, null, 'unavailable timing cannot become zero');
      assert.equal(typeof record.reason, 'string');
      if (record.raw_ticks !== null && record.raw_ticks !== undefined) {
        assert.equal(record.raw_ticks.length, 2);
        record.raw_ticks.forEach(value => integer(value, 'unknown raw tick'));
      }
      if (!unresolved.includes(record)) unavailable++;
    }
  }
  const summary = summaries[0];
  for (const name of ['attempts', 'samples', 'unavailable', 'missing_context', 'context_evictions',
    'unresolved', 'records', 'omitted_records']) count(summary[name], name);
  assert.equal(summary.schema, 1);
  assert.equal(summary.retirement_pending, false, 'GPU owners remain pending');
  assert.equal(summary.retirement_error, null, 'retirement failed');
  assert.equal(summary.records, observations.length);
  assert.ok(summary.records <= 4096);
  assert.equal(summary.unresolved, unresolved.length);
  assert.equal(summary.samples + summary.unavailable, summary.records + summary.omitted_records);
  assert.equal(summary.attempts,
    summary.samples + summary.unavailable - summary.missing_context + summary.unresolved + summary.context_evictions);
  assert.ok(samples <= summary.samples && unavailable <= summary.unavailable && missing <= summary.missing_context);
  if (!summary.omitted_records) {
    assert.equal(samples, summary.samples);
    assert.equal(unavailable, summary.unavailable);
    assert.equal(missing, summary.missing_context);
  }
  assert.equal(summary.streams.length, capabilities.length);
  assert.equal(new Set(summary.streams.map(stream => stream.generation)).size, capabilities.length);
  for (const stream of summary.streams) {
    assert.ok(caps.has(stream.generation));
    count(stream.dropped, 'stream dropped');
  }
  const contexts = [...groups].map(([context, rows]) => {
    rows.sort((a, b) => a.ordinal < b.ordinal ? -1 : a.ordinal > b.ordinal ? 1 : 0);
    const durations = rows.slice(24).filter(row => row.record.status === 'available')
      .slice(0, 256).map(row => Number(row.record.elapsed_ns));
    return { context: JSON.parse(context), recorded_attempts: rows.length,
      warmup_attempts: Math.min(24, rows.length), first_available_post_warmup_samples: durations.length,
      device_command_p50_ms: percentile(durations, 0.5), device_command_p95_ms: percentile(durations, 0.95) };
  });
  return { status: 'verified_gpu_command_observations', gpu_time: summary.samples > 0, capabilities, summary, contexts,
    timing_scope: 'device commands including dependency/scheduling stalls; not isolated GPU busy or display latency' };
}
