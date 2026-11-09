// Native public-API checkpoint for the real Standalone collection Host.
// Semantic commands/captures are not physical-input or screen-reader evidence.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createInterface } from 'node:readline';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
const name = node => node.name?.value;

export class Session {
  constructor(executable, output, { extraArgs = [] } = {}) {
    this.output = output;
    fs.mkdirSync(output, { recursive: true });
    this.executable = executable;
    this.executableHash = hash(fs.readFileSync(executable));
    this.next = 0;
    this.pending = null;
    this.arguments = ['--collections-fixture', '--ui-control-stdio', ...extraArgs];
    this.child = spawn(executable, this.arguments, {
      cwd: root, windowsHide: true, stdio: ['pipe', 'pipe', 'pipe'],
    });
    this.child.stderr.on('data', bytes => fs.appendFileSync(path.join(output, 'native.log'), bytes));
    this.exit = new Promise((resolve, reject) => {
      this.child.once('error', reject);
      this.child.once('exit', (code, signal) => {
        this.terminal = { code, signal };
        if (this.pending) clearTimeout(this.pending.timer);
        this.pending?.reject(new Error(`Host exited: ${code}/${signal}`));
        resolve(this.terminal);
      });
    });
    createInterface({ input: this.child.stdout }).on('line', line => {
      fs.appendFileSync(path.join(output, 'protocol.jsonl'), `${line}\n`);
      try {
        const reply = JSON.parse(line);
        if (!this.pending || reply.request_id !== this.pending.id) throw new Error('Unmatched native reply');
        const pending = this.pending;
        this.pending = null;
        clearTimeout(pending.timer);
        if (reply.error) pending.reject(new Error(`Host ${reply.error.code}`));
        else pending.resolve(reply.result);
      } catch (error) { this.pending?.reject(error); }
    });
  }

  request(body) {
    assert(!this.pending, 'one outstanding request');
    assert(!this.terminal, 'live Host required');
    const id = `collections-${++this.next}`;
    const request = { schema_version: 1, request_id: id, ...body };
    fs.appendFileSync(path.join(this.output, 'requests.jsonl'), `${JSON.stringify(request)}\n`);
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error(`Native reply deadline: ${body.op}`)), 12000);
      this.pending = { id, timer, resolve, reject };
      this.child.stdin.write(`${JSON.stringify(request)}\n`);
    });
  }

  async ready() {
    for (let attempt = 0; attempt < 30; attempt++) {
      const state = await this.request({ op: 'windows' });
      if (state.windows.length === 2 && state.windows.every(window => window.ready)) {
        this.instance = state.instance;
        this.windows = state.windows.map(window => window.window);
        return state;
      }
      await sleep(200);
    }
    throw new Error('Both native collection windows must publish');
  }

  async snapshot(window) {
    const first = await this.request({ op: 'snapshot', instance: this.instance, window, limit: 256 });
    const snapshot = { ...first, nodes: [...first.nodes] };
    assert(!first.coverage.node_limit && !first.coverage.depth_limit && !first.coverage.text_truncated);
    let after = first.next_after;
    while (after) {
      const page = await this.request({ op: 'snapshot', instance: this.instance, window,
        revision: first.revision, after, limit: 256 });
      snapshot.nodes.push(...page.nodes);
      after = page.next_after;
    }
    return snapshot;
  }

  async invoke(window, node, verb = 'activate') {
    const discovery = await this.request({ op: 'commands', instance: this.instance, window });
    const target = discovery.commands.find(command => command.element === node.id);
    assert(target?.commands.includes(verb), `Discover ${verb} for ${name(node)}`);
    return this.request({ op: 'invoke', instance: this.instance, window, element: node.id,
      owner: target.owner, command: verb });
  }

  async changed(window, prior) {
    await this.request({ op: 'wait', instance: this.instance, window, after_revision: prior, timeout_ms: 3000 });
    return this.snapshot(window);
  }

  async input(window, node, steps, modifiers = []) {
    const discovery = await this.request({ op: 'commands', instance: this.instance, window });
    const target = discovery.commands.find(command => command.element === node.id);
    assert(target && (target.input_target || target.window_target), 'discover input target');
    return this.request({ op: 'input', instance: this.instance, window, element: node.id,
      owner: target.owner, revision: discovery.revision, modifiers, steps });
  }

  async capture(window, snapshot, label, viewport = undefined) {
    const image = await this.request({ op: 'capture', instance: this.instance, window, revision: snapshot.revision,
      ...(viewport ? {viewport} : {}) });
    assert.equal(image.mode, viewport ? 'offscreen' : 'next_presented');
    assert.equal(image.format, 'png');
    const bytes = Buffer.from(image.data, 'base64');
    assert.equal(bytes.subarray(1, 4).toString(), 'PNG');
    const filename = `${label}.png`;
    fs.writeFileSync(path.join(this.output, filename), bytes);
    fs.writeFileSync(path.join(this.output, `${label}.json`), `${JSON.stringify({ ...image, data: undefined, file: filename, sha256: hash(bytes) }, null, 2)}\n`);
    return { file: filename, sha256: hash(bytes), viewport: image.viewport };
  }

  async close() {
    if (!this.terminal) {
      const snapshot = await this.snapshot('1');
      const close = snapshot.nodes.find(node => name(node) === 'Close window' && node.visible_bounds);
      assert(close, 'Main close command');
      try { await this.invoke('1', close); }
      catch (error) { if (!this.terminal) throw error; }
    }
    let timer;
    const terminal = await Promise.race([this.exit, new Promise((_, reject) => {
      timer = setTimeout(() => reject(new Error('Host did not retire')), 10000);
    })]).finally(() => clearTimeout(timer));
    assert.equal(terminal.code, 0);
    assert.equal(hash(fs.readFileSync(this.executable)), this.executableHash, 'qualified executable unchanged');
    return terminal;
  }
}

function find(snapshot, label, role = 'Button') {
  const matches = snapshot.nodes.filter(node => name(node) === label && node.role === role && node.visible_bounds);
  assert.equal(matches.length, 1, `one visible ${label}`);
  return matches[0];
}

function messageAnchor(snapshot) {
  const list = find(snapshot, 'Messages', 'List');
  const parents = new Map(snapshot.nodes.map(node => [node.id, node.parent]));
  const belongs = node => {
    for (let parent = node.parent; parent; parent = parents.get(parent)) {
      if (parent === list.id) return true;
    }
    return false;
  };
  const rows = snapshot.nodes.filter(node => node.role === 'ListItem' &&
    node.visible_bounds && belongs(node)).sort((a, b) => a.bounds.y - b.bounds.y);
  assert(rows.length, 'a visible message');
  return { id: rows[0].id, name: name(rows[0]), inset: list.bounds.y - rows[0].bounds.y };
}

async function until(session, window, predicate) {
  for (let attempt = 0; attempt < 30; attempt++) {
    const snapshot = await session.snapshot(window);
    if (predicate(snapshot)) return snapshot;
    await sleep(100);
  }
  throw new Error('Native collection state did not settle');
}

async function resize(session, window, width, height) {
  const snapshot = await session.snapshot(window);
  await session.input(window, snapshot.nodes.find(node => node.parent === null),
    [{ kind: 'window', action: 'resize', width, height }]);
  return until(session, window, next => next.viewport.width === width && next.viewport.height === height);
}

async function matrix(session, floatingId, captures) {
  let main = await resize(session, '1', 920, 760);
  let floating = await resize(session, floatingId, 320, 760);
  for (const scale of [1, 1.5, 2]) {
    captures.push(await session.capture('1', main, `main-920-scale-${scale}`,
      { width: 920, height: 760, scale_factor: scale }));
  }
  await session.input('1', find(main, 'Messages', 'List'),
    [{ kind: 'move', x: 60, y: 80 }, { kind: 'wheel', x: 0, y: 600 }]);
  main = await until(session, '1', next => next.nodes.some(node =>
    name(node)?.includes('Reading history')));
  const anchor = messageAnchor(main);
  for (const action of ['Earlier', 'Append']) {
    await session.invoke('1', find(main, action));
    main = await session.changed('1', main.revision);
    assert.deepEqual(messageAnchor(main), anchor, `${action} preserves reader anchor`);
  }
  floating = await until(session, floatingId, next => next.nodes.some(node => name(node) === 'Open message 2001'));
  assert(floating.nodes.some(node => name(node)?.includes('Following latest')), 'Float still follows');
  main = await resize(session, '1', 400, 760);
  assert.deepEqual(messageAnchor(main), anchor, 'minimum-width reflow preserves reader anchor');
  find(main, 'Freeze paused');
  captures.push(await session.capture('1', main, 'main-400-reader'));
  const mainFocus = main.focus;
  await session.invoke(floatingId, find(floating, 'Asset 0, field 1: 0:1'), 'focus');
  floating = await session.changed(floatingId, floating.revision);
  await session.invoke(floatingId, find(floating, 'RTL'));
  floating = await session.changed(floatingId, floating.revision);
  await session.input(floatingId, find(floating, 'Asset 0, field 1: 0:1'),
    [{ kind: 'key', key: 'ArrowLeft' }]);
  floating = await session.changed(floatingId, floating.revision);
  assert.equal(floating.focus, find(floating, 'Asset 0, field 2: 0:2').id);
  await session.input(floatingId, find(floating, 'Asset 0, field 2: 0:2'),
    [{ kind: 'key', key: 'ArrowLeft', modifiers: ['shift'] }]);
  floating = await session.changed(floatingId, floating.revision);
  assert.equal(floating.focus, find(floating, 'Asset 0, field 3: 0:3').id);
  assert(find(floating, '0:2', 'Cell').selected && find(floating, '0:3', 'Cell').selected);
  assert.equal((await session.snapshot('1')).focus, mainFocus, 'Float navigation preserves Main focus');
  captures.push(await session.capture(floatingId, floating, 'float-320-rtl-range'));
  // Focus/reorder checks that follow use the ordinary wider Main pane.
  await resize(session, '1', 1280, 800);
  return { reader_anchor: anchor, native_sizes: [[920, 760], [400, 760], [320, 760]],
    offscreen_scales: [1, 1.5, 2], float_rtl_range: true };
}

async function main() {
  const executable = path.join(root, 'target/debug/vesta-gpui-standalone.exe');
  const output = path.join(root, 'target/research/native-collections-agent', new Date().toISOString().replaceAll(':', '-'));
  const session = new Session(executable, output);
  const receipt = { status: 'incomplete', executable_sha256: session.executableHash, pid: session.child.pid,
    scope: 'native Standalone public semantic commands, complete snapshots and next-presented GPU captures',
    physical_input: false, screen_reader: false };
  try {
    const windows = await session.ready();
    let main = await session.snapshot('1');
    const floatingId = session.windows.find(window => window !== '1');
    let floating = await session.snapshot(floatingId);
    for (const snapshot of [main, floating]) {
      find(snapshot, 'Messages', 'List'); find(snapshot, 'Assets', 'Table');
      assert(snapshot.nodes.length < 1000, 'bounded realized collection tree');
    }
    const captures = [];
    captures.push(await session.capture('1', main, 'main-initial'));
    captures.push(await session.capture(floatingId, floating, 'float-initial'));
    const prior = main.revision;
    await session.invoke('1', find(main, 'Append'));
    main = await session.changed('1', prior);
    floating = await session.changed(floatingId, floating.revision);
    find(main, 'Open message 2000'); find(floating, 'Open message 2000');
    const mainTable = find(main, 'Assets', 'Table');
    const floatTable = find(floating, 'Assets', 'Table');
    assert.notEqual(mainTable.bounds.width, floatTable.bounds.width, 'window-specific widths');
    receipt.matrix = await matrix(session, floatingId, captures);
    main = await session.snapshot('1');
    await session.invoke('1', find(main, 'Asset 0, field 0: Asset 0'), 'focus');
    main = await session.changed('1', main.revision);
    assert.equal(main.focus, find(main, 'Asset 0, field 0: Asset 0').id);
    const nativeFocus = main.focus;
    await session.invoke('1', find(main, 'Reverse'));
    main = await session.changed('1', main.revision);
    assert(main.nodes.some(node => node.id === nativeFocus), 'focused cell retained after reorder');
    await session.invoke('1', find(main, 'RTL'));
    main = await session.changed('1', main.revision);
    captures.push(await session.capture('1', main, 'main-rtl'));
    receipt.windows = windows;
    receipt.captures = captures;
    receipt.native_focus = nativeFocus;
    receipt.status = 'checks_passed';
  } catch (error) {
    receipt.status = 'failed'; receipt.error = String(error.stack ?? error);
    throw error;
  } finally {
    try {
      receipt.terminal = await session.close();
      if (receipt.status === 'checks_passed') receipt.status = 'passed';
    }
    catch (error) {
      receipt.status = 'failed'; receipt.retirement_error = String(error);
      if (!session.terminal) { session.child.kill(); await session.exit; }
    }
    fs.writeFileSync(path.join(output, 'receipt.json'), `${JSON.stringify(receipt, null, 2)}\n`);
    if (receipt.status !== 'passed') process.exitCode = 1;
  }
  if (receipt.status === 'passed') console.log(JSON.stringify({ status: receipt.status, output, captures: receipt.captures.length }));
}

if (typeof process !== 'undefined' && process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main().catch(error => { console.error(error); process.exitCode = 1; });
}
