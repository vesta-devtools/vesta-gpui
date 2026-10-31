// Actual owned offscreen label visibility evidence, separate from reflow/physical input.
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import assert from 'node:assert/strict';
import {inflateSync} from 'node:zlib';
import {fileURLToPath} from 'node:url';

const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const sha = value => typeof value === 'string' && /^[0-9a-f]{64}$/.test(value);
const utf8 = file => new TextDecoder('utf8', {fatal: true}).decode(fs.readFileSync(file));
function crc(bytes) {
  let value = 0xffffffff;
  for (const byte of bytes) {
    value ^= byte;
    for (let i = 0; i < 8; i++) value = (value >>> 1) ^ (value & 1 ? 0xedb88320 : 0);
  }
  return (value ^ 0xffffffff) >>> 0;
}
function png(file) {
  const bytes = fs.readFileSync(file);
  assert(bytes.length <= 65536 && bytes.subarray(0, 8).toString('hex') === '89504e470d0a1a0a', 'bounded PNG required');
  let cursor = 8, width, height, ended = false;
  const compressed = [];
  while (cursor < bytes.length) {
    assert(cursor + 12 <= bytes.length, 'complete PNG chunk');
    const length = bytes.readUInt32BE(cursor), end = cursor + 12 + length;
    assert(end <= bytes.length, 'complete PNG payload');
    const kind = bytes.subarray(cursor + 4, cursor + 8).toString('ascii');
    const data = bytes.subarray(cursor + 8, end - 4);
    assert.equal(crc(bytes.subarray(cursor + 4, end - 4)), bytes.readUInt32BE(end - 4), 'PNG CRC');
    if (kind === 'IHDR') {
      assert(width === undefined && length === 13, 'one complete PNG header');
      width = data.readUInt32BE(0); height = data.readUInt32BE(4);
      assert.equal(width, 32); assert.equal(height, 32);
      assert.deepEqual([...data.subarray(8)], [8, 6, 0, 0, 0], 'host RGBA8/noninterlaced PNG');
    } else if (kind === 'IDAT') {
      assert(width !== undefined && !ended); compressed.push(data);
    } else if (kind === 'IEND') {
      assert.equal(length, 0); ended = true;
    } else {
      assert((bytes[cursor + 4] & 32) !== 0, 'unsupported critical PNG chunk');
    }
    cursor = end;
    if (ended) break;
  }
  assert(ended && cursor === bytes.length && compressed.length, 'complete PNG stream');
  const stride = width * 4, expected = (stride + 1) * height;
  const raw = inflateSync(Buffer.concat(compressed), {maxOutputLength: expected});
  assert.equal(raw.length, expected, 'bounded scanlines');
  const rgba = Buffer.alloc(width * height * 4);
  for (let row = 0; row < height; row++) {
    const filter = raw[row * (stride + 1)]; assert(filter <= 4, 'PNG filter');
    for (let x = 0; x < stride; x++) {
      const at = row * stride + x, left = x >= 4 ? rgba[at - 4] : 0;
      const up = row ? rgba[at - stride] : 0, corner = row && x >= 4 ? rgba[at - stride - 4] : 0;
      let prediction = 0;
      if (filter === 1) prediction = left;
      if (filter === 2) prediction = up;
      if (filter === 3) prediction = Math.floor((left + up) / 2);
      if (filter === 4) {
        const p = left + up - corner, a = Math.abs(p - left), b = Math.abs(p - up), c = Math.abs(p - corner);
        prediction = a <= b && a <= c ? left : b <= c ? up : corner;
      }
      rgba[at] = (raw[row * (stride + 1) + 1 + x] + prediction) & 255;
    }
  }
  return {width, height, rgba, png_sha256: hash(bytes), rgba_sha256: hash(rgba)};
}
function changed(a, b) {
  let count = 0;
  for (let at = 0; at < a.length; at += 4) if (!a.subarray(at, at + 4).equals(b.subarray(at, at + 4))) count++;
  return count;
}

export function verify(file, expectedPid, expectedSource, expectedExe) {
  assert(Number.isSafeInteger(expectedPid) && expectedPid > 0 && sha(expectedSource) && sha(expectedExe), 'external process/source/artifact binding required');
  const receipt = JSON.parse(utf8(file)), directory = path.dirname(file);
  assert.equal(receipt.schema_version, 1); assert.equal(receipt.native_pid, expectedPid);
  assert.equal(receipt.source_tree_sha256, expectedSource); assert.equal(receipt.executable_sha256, expectedExe);
  assert.equal(receipt.physical_input, false); assert.equal(receipt.visible, false); assert.equal(receipt.focused, false);
  assert.equal(receipt.trace, 'scene.html'); assert.equal(receipt.captures.length, 8);
  for (const [value, bits] of [[receipt.instance, 128], [receipt.window, 64]]) {
    assert(typeof value === 'string' && /^[1-9][0-9]*$/.test(value) && BigInt(value) < (1n << BigInt(bits)), 'source identity');
  }
  const html = utf8(path.join(directory, receipt.trace));
  const payload = html.match(/<script id="gpui-data" type="application\/json">([\s\S]*?)<\/script>/);
  assert(payload, 'native trace report required');
  const trace = JSON.parse(payload[1]); assert.equal(trace.gpui_trace.schema_version, 2);
  for (const key of ['dropped_events', 'unfinished_spans', 'invalid_nesting']) assert.equal(trace.gpui_trace[key], 0, key);
  assert.deepEqual([...new Set(trace.traceEvents.map(e => e.pid))], [expectedPid], 'trace process');
  const phases = trace.traceEvents.filter(e => e.name === 'viewport.capture_admitted'); assert.equal(phases.length, 8);
  const counters = span => trace.traceEvents.filter(e => e.ph === 'C' && e.ts >= span.ts && e.ts <= span.ts + span.dur + .01);
  for (let phase = 0; phase < 8; phase++) {
    const values = counters(phases[phase]);
    assert.equal(values.find(e => e.name === 'viewport.phase')?.args?.value, phase, 'ordered capture phase');
    assert.equal(values.find(e => e.name === 'viewport.projection')?.args?.value, Number(phase >= 4));
  }
  const complete = trace.traceEvents.filter(e => e.name === 'viewport.complete'); assert.equal(complete.length, 1);
  const finished = counters(complete[0]); assert.equal(finished.find(e => e.name === 'viewport.captures')?.args?.value, 8);
  assert(finished.find(e => e.name === 'viewport.later_presentations')?.args?.value >= 2);
  const last = phases.at(-1), later = trace.gpui_frames.filter(frame => frame.name === 'Standalone.Main'
    && frame.start_us > last.ts + last.dur && frame.markers.some(m => m.name === 'present.submitted'));
  assert(later.length >= 2, 'actual successful Main frames after captures');
  const images = receipt.captures.map((capture, phase) => {
    assert.equal(capture.phase, phase); assert.equal(capture.file, 'capture-' + phase + '.png');
    const m = capture.metadata; assert.equal(m.mode, 'offscreen'); assert.equal(m.instance, receipt.instance); assert.equal(m.window, receipt.window);
    assert.equal(m.resource_context, 'current_host_resources'); assert.equal(m.format, 'png'); assert.equal(m.color_space, 'srgb');
    assert.deepEqual(m.viewport, {width: 1280, height: 800, scale_factor: 1});
    assert.deepEqual(m.region, {x: 624, y: 384, width: 32, height: 32});
    assert.deepEqual(m.coverage, {gpui_render_target: true, native_child_surfaces: false, os_compositor: false});
    for (const value of [m.revision, m.renderer_frame]) assert(typeof value === 'string' && /^[1-9][0-9]*$/.test(value) && BigInt(value) < (1n << 64n), 'readback frame identity');
    const image = png(path.join(directory, capture.file)); assert.equal(image.png_sha256, capture.png_sha256, 'capture bytes');
    return image;
  });
  const failures = [], projections = [];
  for (const offset of [0, 4]) {
    const name = offset ? 'orthographic' : 'perspective';
    const differences = [1, 2, 3].map(i => changed(images[offset].rgba, images[offset + i].rgba));
    if (!differences[0]) failures.push(name + '.visible');
    if (differences[1]) failures.push(name + '.near');
    if (differences[2]) failures.push(name + '.far');
    projections.push({projection: name, visible_changed_pixels: differences[0], near_changed_pixels: differences[1], far_changed_pixels: differences[2]});
  }
  return {status: failures.length ? 'failed' : 'passed', native_pid: expectedPid, source_tree_sha256: expectedSource,
    executable_sha256: expectedExe, captures: 8, later_submitted_frames: later.length, projections, failures,
    images: images.map(({rgba, ...identity}) => identity), physical_input: false, compositor_or_display_latency: false};
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    const [file, pid, source, executable, extra] = process.argv.slice(2); assert(file && !extra, 'receipt/PID/source/executable required');
    const result = verify(file, Number(pid), source, executable); console.log(JSON.stringify(result, null, 2));
    if (result.status !== 'passed') process.exitCode = 1;
  } catch (error) { console.error(String(error)); process.exitCode = 1; }
}
