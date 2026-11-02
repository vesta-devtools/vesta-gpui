// Maintainer fixtures and receipt checks, never a product analysis engine.
import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {fileURLToPath} from 'node:url';
const sha=p=>createHash('sha256').update(fs.readFileSync(p)).digest('hex');

export function prepare(directory) {
  const out=path.resolve(directory);assert.ok(!fs.existsSync(out),'fixture directory must be new');
  fs.mkdirSync(out,{recursive:true});const manifest=[];
  for(const [label,pid] of [['A',501],['B',502]]) {
    const events=[];let eid=0;
    for(let lane=1;lane<=12;lane++) {
      const frame=++eid,start=100+lane*10;
      events.push({pid,tid:lane,ts:start,dur:1000,ph:'X',cat:'fixture.frame',name:`Frame ${label} ${lane}`,args:{event_id:frame,frame_id:frame,parent_id:null,frame:true,unfinished:false}});
      for(let i=0;i<18;i++) {
        const name=i<10?`Common ${label}`:`Unique ${label} ${lane}-${i-10}`;
        events.push({pid,tid:lane,ts:start+10+i*35,dur:20,ph:'X',cat:'fixture.scope',name,args:{event_id:++eid,frame_id:frame,parent_id:frame,frame:false,unfinished:false}});
      }
    }
    const data={fixture:{kind:'synthetic',purpose:'keyboard reachability and source identity; not measured timing'},gpui_trace:{schema_version:2,clock:'monotonic_elapsed_wall_time',thread_ids:'capture_local',gpu_execution_timing:false,detail:'stages',events:events.length,dropped_events:0,unfinished_spans:0,invalid_nesting:0,unframed_events:0,expired:false},traceEvents:events};
    const file=path.join(out,`synthetic-focus-${label}.json`);fs.writeFileSync(file,JSON.stringify(data,null,2)+'\n',{flag:'wx'});
    manifest.push({path:file,sha256:sha(file),events:events.length,lanes:12,origin:'synthetic, not measured timing'});
  }
  fs.writeFileSync(path.join(out,'manifest.json'),JSON.stringify(manifest,null,2)+'\n',{flag:'wx'});
  return {status:'prepared',manifest:path.join(out,'manifest.json'),captures:manifest};
}

export function verify(manifestPath,logPath) {
  const inputs=JSON.parse(fs.readFileSync(manifestPath,'utf8'));assert.equal(inputs.length,2);
  for(const input of inputs)assert.equal(sha(input.path),input.sha256);
  const log=fs.readFileSync(logPath,'utf8');
  for(const flag of ['search_owns_keys=true','statistics_tab_page_arrows_end=true','busy_keeps_logical_focus=true','tracks_tab_page_end=true','wheel_keeps_page_history=true','steps=10 keyboard_views=true','old_source_targets_retired=true','capture_retired=true','new_source_action=true','trace_closed gpu_retired=true worker_joined=true'])assert.ok(log.includes(flag),flag);
  const records=prefix=>log.split(/\r?\n/).filter(l=>l.startsWith(prefix)).map(l=>JSON.parse(l.slice(prefix.length)));
  const switches=records('trace_focus_source '),selected=records('trace_focus_data ');assert.equal(switches.length,1);assert.equal(selected.length,1);
  const change=switches[0],data=selected[0];assert.equal(change.schema_version,1);assert.equal(data.schema_version,1);
  assert.equal(change.source_a,inputs[0].sha256);assert.equal(change.source_b,inputs[1].sha256);assert.equal(data.source_sha256,change.source_b);assert.equal(data.analysis_version,change.analysis_version);
  for(const key of ['old_node','new_node','reused_element'])assert.match(change[key],/^\d+$/);
  assert.notEqual(change.old_node,change.new_node);assert.equal(change.old_target_accepted,false);assert.equal(change.old_capture_retained,false);
  return {status:'passed',origin:'synthetic, not measured timing',manifest:path.resolve(manifestPath),manifest_sha256:sha(manifestPath),log:path.resolve(logPath),log_sha256:sha(logPath),verifier_sha256:sha(fileURLToPath(import.meta.url)),source_switch:change,selected_timer:data.timer};
}

if(process.argv[1]&&path.resolve(process.argv[1])===fileURLToPath(import.meta.url)) {
  try {
    const [mode,a,b,...extra]=process.argv.slice(2);assert.equal(extra.length,0);
    if(mode==='--prepare'&&a&&!b)console.log(JSON.stringify(prepare(a)));
    else if(mode==='--verify'&&a&&b)console.log(JSON.stringify(verify(a,b)));
    else throw Error('Usage: trace_focus.mjs --prepare <new directory> | --verify <manifest.json> <native stdout log>');
  }catch(error){console.error(JSON.stringify({status:'failed',error:String(error)}));process.exitCode=1;}
}
