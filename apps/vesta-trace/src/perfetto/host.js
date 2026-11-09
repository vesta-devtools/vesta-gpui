'use strict';
(() => {
  const frame = document.getElementById('perfetto');
  const error = document.getElementById('error');
  const notice = document.getElementById('notice');
  const picker = document.getElementById('picker');
  const base = location.pathname.replace(/\/$/, '');
  let revision = 0;
  let loading = false;
  let ready;
  let disposed = false;
  let busy = false;
  let configured = false;
  let currentTarget;
  function analysis(enabled) {
    document.body.classList.toggle('analysis', enabled);
    document.getElementById('analysis-tools').hidden = !enabled;
    document.getElementById('diagnostics').hidden = true;
    document.getElementById('quality').setAttribute('aria-expanded', 'false');
    document.getElementById('analyze').hidden = !window.vestaCapture;
  }

  function notify(value) { window.ipc?.postMessage(JSON.stringify(value)); }
  function fail(message) { error.textContent = message; notify({type: 'error', message}); }
  function controls(state) {
    currentTarget = state.target;
    if (!configured) { document.getElementById('executable').value = state.executable; configured = true; }
    const phase = state.target?.state ?? 'disconnected';
    document.getElementById('target-state').textContent = phase === 'recording' || phase === 'expired'
      ? `${phase === 'expired' ? 'Capture limit reached' : 'Recording'} · ${(state.target.elapsed_ms / 1000).toFixed(1)}s · ${state.target.events.toLocaleString()} events`
      : phase === 'idle' ? `Connected · PID ${state.target.process_id ?? ''}` : phase;
    document.getElementById('launch').disabled = busy || !!state.target;
    document.getElementById('connect').disabled = busy || !!state.target;
    document.getElementById('disconnect').disabled = busy || !state.target;
    document.getElementById('start').disabled = busy || phase !== 'idle';
    document.getElementById('stop').disabled = busy || !['recording', 'expired'].includes(phase);
    if (state.target?.message) error.textContent = state.target.message;
    if (phase === 'expired' && !busy) void action('stop');
  }
  async function action(operation) {
    if (busy) return;
    busy = true;
    if (operation === 'start') analysis(false);
    try {
      const body = operation === 'launch' ? {executable: document.getElementById('executable').value}
        : operation === 'connect' ? {descriptor: document.getElementById('descriptor').value}
        : operation === 'start' ? {duration_seconds: Number(document.getElementById('duration').value), max_events: 100000, detail: 'stages'} : {};
      const response = await fetch(`${base}/${operation}`, {method: 'POST', headers: {'Content-Type': 'application/json'}, body: JSON.stringify(body)});
      const state = await response.json();
      if (!response.ok) throw new Error(`${state.error?.code ?? 'trace_control'}: ${state.error?.message ?? 'Capture control failed'}`);
      notify({type: 'control', operation, state: state.target?.state});
    } catch (reason) { fail(reason.message); }
    finally { busy = false; await refresh(); }
  }
  function handshake() {
    return new Promise((resolve, reject) => {
      const timer = setInterval(() => frame.contentWindow.postMessage('PING', location.origin), 100);
      const deadline = setTimeout(() => finish(new Error('Perfetto did not become ready. Check the local tools bundle, then Reload.')), 45000);
      function finish(reason) {
        clearInterval(timer); clearTimeout(deadline); window.removeEventListener('message', receive);
        reason ? reject(reason) : resolve();
      }
      function receive(event) {
        if (event.source === frame.contentWindow && event.origin === location.origin && event.data === 'PONG') finish();
      }
      window.addEventListener('message', receive);
    });
  }
  async function refresh() {
    const response = await fetch(`${base}/status`, {cache: 'no-store'});
    if (!response.ok) throw new Error('Could not read the local capture state.');
    const state = await response.json();
    error.textContent = state.error ? `${state.error.code}: ${state.error.message}` : '';
    controls(state);
    document.getElementById('native').disabled = !state.capture?.path || busy;
    document.getElementById('storage').textContent = state.capture?.path ? `Capture file: ${state.capture.path}` : '';
    if (!state.capture || state.revision === revision || loading) return;
    loading = true;
    try {
      const capture = state.capture;
      document.getElementById('file').textContent = `Loading ${capture.title}…`;
      frame.hidden = false;
      document.getElementById('welcome').hidden = true;
      ready ??= handshake();
      await ready;
      const data = await fetch(`${base}/capture/${state.revision}`, {cache: 'no-store'});
      if (!data.ok) throw new Error('Capture changed while loading; retrying.');
      const buffer = await data.arrayBuffer();
      // This is Perfetto's documented API. Large captures never go through native evaluate/IPC.
      frame.contentWindow.postMessage({perfetto: {buffer, title: capture.title, keepApiOpen: true,
        shareable: false, downloadable: false}}, location.origin, [buffer]);
      revision = state.revision;
      document.getElementById('file').textContent = `${capture.title} · ${capture.events.toLocaleString()} events · ${capture.frames.toLocaleString()} frames`;
      const warnings = capture.diagnostics.map(d => `${d.code}: ${d.message}`);
      if (capture.gpu_submissions) warnings.push(`${capture.gpu_submissions} GPU submissions are preserved in the capture. Use native details or Agent queries to inspect GPU evidence; this timeline shows CPU work.`);
      notice.textContent = warnings.join('\n');
      window.vestaCapture = {revision, ...capture};
      const quality = document.getElementById('quality');
      quality.textContent = warnings.length ? `Diagnostics · ${warnings.length}` : 'Details';
      quality.classList.toggle('has-notices', warnings.length > 0);
      analysis(true);
      notify({type: 'posted', revision, events: capture.events, source: capture.source.sha256});
    } finally { loading = false; }
  }
  async function open(file) {
    if (!file) return;
    if (file.size > 256 * 1024 * 1024) { fail('Capture exceeds the 256 MiB input limit.'); return; }
    document.getElementById('file').textContent = `Reading ${file.name}…`;
    try {
      const response = await fetch(`${base}/capture`, {method: 'POST', body: file});
      const state = await response.json();
      if (!response.ok) throw new Error(`${state.error?.code ?? 'trace_import'}: ${state.error?.message ?? 'Import failed'}`);
      await refresh();
    } catch (reason) { fail(reason.message); }
  }
  document.getElementById('open').onclick = () => picker.click();
  for (const operation of ['launch', 'connect', 'disconnect', 'start', 'stop', 'native']) document.getElementById(operation).onclick = () => { void action(operation); };
  picker.onchange = () => { void open(picker.files[0]); picker.value = ''; };
  document.getElementById('reload').onclick = () => location.reload();
  document.getElementById('capture-controls').onclick = () => analysis(false);
  document.getElementById('analyze').onclick = () => analysis(true);
  document.getElementById('quality').onclick = () => {
    const panel = document.getElementById('diagnostics');
    panel.hidden = !panel.hidden;
    document.getElementById('quality').setAttribute('aria-expanded', String(!panel.hidden));
  };
  document.getElementById('close-diagnostics').onclick = () => {
    document.getElementById('diagnostics').hidden = true;
    document.getElementById('quality').setAttribute('aria-expanded', 'false');
    document.getElementById('quality').focus();
  };
  window.addEventListener('dragover', event => event.preventDefault());
  window.addEventListener('drop', event => { event.preventDefault(); void open(event.dataTransfer.files[0]); });
  window.addEventListener('pagehide', () => { disposed = true; });
  async function poll() {
    try { await refresh(); } catch (reason) { fail(reason.message); }
    if (!disposed) setTimeout(poll, 500);
  }
  // Test-only probes are injected by the finite native acceptance mode, not by production pages.
  window.vestaRefresh = refresh;
  window.vestaAction = action;
  window.vestaTarget = () => currentTarget;
  notify({type: 'ready'});
  void poll();
})();
