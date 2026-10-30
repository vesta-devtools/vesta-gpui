// Join already validated native markers to actual same-window GPU submissions.
import assert from 'node:assert/strict';

export function correlateNativeInputGpu(lines, lifecycle, gpu) {
  if (!lifecycle.native_input_sample_count) {
    return {status: 'no_native_input_samples', frames: []};
  }
  assert.equal(lifecycle.fully_accounted, true, 'native input ledger is partial');
  for (const key of ['missing_context', 'context_evictions', 'unresolved', 'omitted_records']) {
    assert.equal(gpu.summary[key], 0, `GPU frame association is partial: ${key}`);
  }
  assert.ok(gpu.summary.streams.every(stream => stream.dropped === 0));
  const records = prefix => lines.filter(line => line.startsWith(prefix))
    .map(line => JSON.parse(line.slice(prefix.length)));
  const windows = new Map();
  for (const observation of records('ui_gpu_profile ')) {
    assert.ok(observation.context, 'missing original GPU context');
    if (observation.context.render_outcome.status !== 'presented') continue;
    const id = observation.context.window;
    if (!windows.has(id)) windows.set(id, []);
    windows.get(id).push(observation);
  }
  for (const counter of lifecycle.summary.windows) {
    const frames = windows.get(counter.window) ?? [];
    frames.sort((a, b) => BigInt(a.context.host_attempt) < BigInt(b.context.host_attempt) ? -1 : 1);
    assert.equal(frames.length, counter.successful_present_calls, 'GPU/Host successful-frame counts differ');
  }
  const frames = records('ui_native_input_presented ').map(marker => {
    const index = Number(BigInt(marker.host_present_index) - 1n);
    assert.ok(Number.isSafeInteger(index) && index >= 0);
    const observation = windows.get(marker.window)?.[index];
    assert.ok(observation, 'first applicable frame has no GPU submission');
    assert.equal(observation.status, 'available', 'first applicable GPU timing is unavailable');
    return {input_sequence: marker.sequence, window: marker.window, kind: marker.kind,
      host_present_index: marker.host_present_index, frame_input_watermark: marker.frame_input_watermark,
      receipt_to_present_return_ns: marker.receipt_to_present_return_ns,
      gpu_generation: observation.generation, gpu_submission: observation.submission,
      gpu_host_attempt: observation.context.host_attempt,
      gpu_command_elapsed_ns: observation.elapsed_ns, raw_gpu_ticks: observation.raw_ticks,
      original_frame_context: observation.context};
  });
  return {status: 'verified_native_input_gpu_frame_association', frames,
    rule: 'Each successful Host render callback increments the window present counter once. All original-context GPU attempts whose Host outcome is presented, sorted by Host attempt, have the identical per-window sequence. The qualified pre-build watermark consumes input on its first eligible callback.',
    boundary: 'Native receipt to successful present-return; the matching GPU command completion/timing is independently observed. GPU/CPU clocks are not subtracted or added; physical display remains unmeasured.'};
}
