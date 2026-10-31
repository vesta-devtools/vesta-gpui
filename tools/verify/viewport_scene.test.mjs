import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import {deflateSync} from 'node:zlib';
import {fileURLToPath} from 'node:url';
import {verify} from './viewport_scene.mjs';

// Synthetic verifier contracts only, never a substitute for actual native pixels.
test('scene evidence rejects source, phase, PNG and clipping substitutions', () => {
  const base = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../target/research');
  fs.mkdirSync(base, {recursive: true});
  const directory = fs.mkdtempSync(path.join(base, 'viewport-scene-unit-'));
  const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
  const chunk = (kind, data) => {
    const b = Buffer.alloc(data.length + 12); b.writeUInt32BE(data.length, 0); b.write(kind, 4); data.copy(b, 8);
    let crc = 0xffffffff;
    for (const byte of b.subarray(4, -4)) { crc ^= byte; for (let i = 0; i < 8; i++) crc = (crc >>> 1) ^ (crc & 1 ? 0xedb88320 : 0); }
    b.writeUInt32BE((crc ^ 0xffffffff) >>> 0, b.length - 4); return b;
  };
  const image = visible => {
    const ihdr = Buffer.alloc(13); ihdr.writeUInt32BE(32, 0); ihdr.writeUInt32BE(32, 4); ihdr[8] = 8; ihdr[9] = 6;
    const rows = Buffer.alloc(32 * 129);
    for (let row = 0; row < 32; row++) for (let x = 0; x < 32; x++) {
      const at = row * 129 + 1 + x * 4; rows.set(visible && row === 16 && x === 16 ? [255,255,255,255] : [17,23,31,255], at);
    }
    return Buffer.concat([Buffer.from('89504e470d0a1a0a', 'hex'), chunk('IHDR', ihdr), chunk('IDAT', deflateSync(rows)), chunk('IEND', Buffer.alloc(0))]);
  };
  const events = [], pid = 123, source = 'a'.repeat(64), executable = 'b'.repeat(64);
  for (let phase = 0; phase < 8; phase++) events.push(
    {name:'viewport.capture_admitted', ph:'X', pid, ts:phase*10, dur:1},
    {name:'viewport.phase', ph:'C', pid, ts:phase*10+.1, args:{value:phase}},
    {name:'viewport.projection', ph:'C', pid, ts:phase*10+.2, args:{value:Number(phase>=4)}});
  events.push({name:'viewport.complete', ph:'X', pid, ts:100, dur:1},
    {name:'viewport.captures', ph:'C', pid, ts:100.1, args:{value:8}},
    {name:'viewport.later_presentations', ph:'C', pid, ts:100.2, args:{value:2}});
  const trace = {gpui_trace:{schema_version:2,dropped_events:0,unfinished_spans:0,invalid_nesting:0},traceEvents:events,
    gpui_frames:[90,92].map(start_us=>({name:'Standalone.Main',start_us,duration_us:1,markers:[{name:'present.submitted'}]}))};
  const receipt = {schema_version:1,native_pid:pid,source_tree_sha256:source,executable_sha256:executable,
    physical_input:false,visible:false,focused:false,trace:'scene.html',instance:'9',window:'1',captures:[]};
  for (let phase = 0; phase < 8; phase++) {
    const bytes = image(phase % 4 === 1), file = 'capture-'+phase+'.png'; fs.writeFileSync(path.join(directory,file),bytes);
    receipt.captures.push({phase,file,png_sha256:sha(bytes),metadata:{mode:'offscreen',instance:'9',window:'1',revision:'1',renderer_frame:'2',
      resource_context:'current_host_resources',format:'png',color_space:'srgb',viewport:{width:1280,height:800,scale_factor:1},region:{x:624,y:384,width:32,height:32},
      coverage:{gpui_render_target:true,native_child_surfaces:false,os_compositor:false}}});
  }
  const file = path.join(directory,'scene.json');
  const write = () => {fs.writeFileSync(file,JSON.stringify(receipt));fs.writeFileSync(path.join(directory,'scene.html'),'<script id="gpui-data" type="application/json">'+JSON.stringify(trace)+'</script>');};
  try {
    write(); assert.equal(verify(file,pid,source,executable).status,'passed');
    assert.throws(()=>verify(file,pid+1,source,executable)); assert.throws(()=>verify(file,pid,'c'.repeat(64),executable));
    trace.traceEvents[1].args.value=1;write();assert.throws(()=>verify(file,pid,source,executable));trace.traceEvents[1].args.value=0;
    receipt.captures[0].file='../capture-0.png';write();assert.throws(()=>verify(file,pid,source,executable));receipt.captures[0].file='capture-0.png';
    for (const phase of [2,3,6,7]) {
      const original=fs.readFileSync(path.join(directory,receipt.captures[phase].file)), bytes=image(true);
      fs.writeFileSync(path.join(directory,receipt.captures[phase].file),bytes);receipt.captures[phase].png_sha256=sha(bytes);write();
      assert.equal(verify(file,pid,source,executable).status,'failed','outside-depth pixels cannot become valid evidence');
      fs.writeFileSync(path.join(directory,receipt.captures[phase].file),original);receipt.captures[phase].png_sha256=sha(original);
    }
    const bytes=fs.readFileSync(path.join(directory,'capture-0.png'));bytes[20]^=1;fs.writeFileSync(path.join(directory,'capture-0.png'),bytes);receipt.captures[0].png_sha256=sha(bytes);write();
    assert.throws(()=>verify(file,pid,source,executable),/CRC/);
  } finally {
    assert(path.resolve(directory).startsWith(base + path.sep));fs.rmSync(directory,{recursive:true,force:true});
  }
});
