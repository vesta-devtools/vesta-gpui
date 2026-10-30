// ADR-0314: counts and native receipt/present-return markers, never display latency.
import assert from 'node:assert/strict';

const count = (value, name) => assert.ok(Number.isSafeInteger(value) && value >= 0, `invalid ${name}`);
const integer = (value, name) => {
  assert.equal(typeof value, 'string', `${name} must be a decimal string`);
  assert.match(value, /^(0|[1-9]\d*)$/, `invalid ${name}`);
  return BigInt(value);
};
const percentile = (values, fraction) => values.length
  ? [...values].sort((a, b) => a - b)[Math.ceil(values.length * fraction) - 1] / 1_000_000 : null;

export function verifyLifecycleProfile(lines, result) {
  assert.ok(result.args.includes('--profile-ui'));
  assert.equal(result.terminal, true);
  assert.equal(result.code, 0);
  assert.match(result.sha256_before, /^[A-Fa-f0-9]{64}$/);
  assert.equal(result.sha256_before, result.sha256_after);
  assert.deepEqual(result.drift, []);
  const records = prefix => lines.filter(line => line.startsWith(prefix))
    .map(line => JSON.parse(line.slice(prefix.length)));
  const exits = records('ui_lifecycle_exit ');
  const received = records('ui_native_input_received ');
  const presented = records('ui_native_input_presented ');
  assert.equal(exits.length, 1, 'one lifecycle exit required');
  const summary = exits[0];
  assert.equal(summary.schema, 1);
  const elapsed = integer(summary.elapsed_ns, 'elapsed_ns');
  for (const name of ['event_loop_batches', 'timer_wakeups', 'cancelled_waits', 'poll_batches', 'about_to_wait',
    'native_input_received', 'native_input_completed', 'native_input_pending', 'native_input_evicted',
    'omitted_receipt_logs', 'omitted_completion_logs', 'omitted_window_events']) count(summary[name], name);
  assert.ok(summary.event_loop_batches >= summary.timer_wakeups + summary.cancelled_waits + summary.poll_batches);
  assert.ok(summary.windows.length <= 64 && summary.native_input_pending <= 64);
  const windows = new Map();
  for (const window of summary.windows) {
    integer(window.window, 'window');
    assert.equal(windows.has(window.window), false, 'duplicate window counter');
    for (const name of ['native_events', 'redraw_deliveries', 'successful_present_calls']) count(window[name], name);
    assert.ok(window.redraw_deliveries <= window.native_events);
    windows.set(window.window, window);
  }
  assert.ok(received.length <= 256 && presented.length <= 256);
  assert.equal(summary.native_input_received, received.length + summary.omitted_receipt_logs);
  assert.equal(summary.native_input_completed, presented.length + summary.omitted_completion_logs);
  assert.equal(summary.native_input_received,
    summary.native_input_completed + summary.native_input_pending + summary.native_input_evicted);
  const receipts = new Map();
  let lastSequence = 0n;
  for (const receipt of received) {
    assert.equal(receipt.schema, 1);
    const sequence = integer(receipt.sequence, 'sequence');
    assert.ok(sequence > lastSequence, 'native receipts reordered or duplicated');
    lastSequence = sequence;
    assert.equal(receipt.origin, 'winit_native', 'command/synthetic input is not native evidence');
    assert.ok(['pointer_press', 'pointer_release', 'wheel', 'key_press'].includes(receipt.kind));
    assert.ok(integer(receipt.received_ns, 'received_ns') <= elapsed);
    assert.ok(windows.has(receipt.window) || summary.omitted_window_events > 0);
    receipts.set(receipt.sequence, receipt);
  }
  const completed = new Set(), lastPresent = new Map(), groups = new Map();
  for (const completion of presented) {
    assert.equal(completion.schema, 1);
    assert.equal(completed.has(completion.sequence), false, 'input credited twice');
    completed.add(completion.sequence);
    const receipt = receipts.get(completion.sequence);
    assert.ok(receipt, 'completion has no recorded native receipt');
    assert.equal(completion.window, receipt.window, 'foreign window consumed input');
    assert.equal(completion.kind, receipt.kind);
    assert.equal(completion.received_ns, receipt.received_ns);
    const start = integer(completion.received_ns, 'received_ns');
    const end = integer(completion.present_return_ns, 'present_return_ns');
    assert.ok(end >= start && end <= elapsed);
    const duration = integer(completion.receipt_to_present_return_ns, 'receipt_to_present_return_ns');
    assert.equal(duration, end - start, 'latency interval differs from raw monotonic markers');
    const watermark = integer(completion.frame_input_watermark, 'frame_input_watermark');
    assert.ok(watermark >= integer(completion.sequence, 'sequence'), 'input arrived after frame build');
    assert.ok(watermark <= BigInt(summary.native_input_received));
    const index = integer(completion.host_present_index, 'host_present_index');
    assert.ok(index > 0n && index <= BigInt(windows.get(completion.window).successful_present_calls));
    assert.ok(index >= (lastPresent.get(completion.window) ?? 0n), 'present markers reordered');
    lastPresent.set(completion.window, index);
    const group = `${completion.window}/${completion.kind}`;
    if (!groups.has(group)) groups.set(group, []);
    groups.get(group).push(Number(duration));
  }
  return {
    status: 'verified_lifecycle_and_native_present_return_markers', summary,
    native_input_sample_count: presented.length,
    native_input_proven: presented.length > 0,
    fully_accounted: summary.native_input_pending === 0 && summary.native_input_evicted === 0
      && summary.omitted_receipt_logs === 0 && summary.omitted_completion_logs === 0,
    contexts: [...groups].map(([window_kind, values]) => ({window_kind, samples: values.length,
      receipt_to_present_return_p50_ms: percentile(values, 0.5), receipt_to_present_return_p95_ms: percentile(values, 0.95)})),
    boundary: 'Instrumented Host native entry to first same-window watermark-applicable successful present-return marker; source qualification proves first-applicable consumption. No OS pre-dispatch, GPU completion or physical-display measurement.',
  };
}
