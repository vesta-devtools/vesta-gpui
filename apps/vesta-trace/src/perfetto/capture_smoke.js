// Real target process and framework scopes; finite command-origin acceptance.
(async () => {
  const send = value => window.ipc.postMessage(JSON.stringify(value));
  const pause = ms => new Promise(resolve => setTimeout(resolve, ms));
  const wait = async predicate => {
    const deadline = Date.now() + 20000;
    while (!predicate()) { if (Date.now() > deadline) throw new Error('Live capture state timed out'); await pause(100); }
  };
  try {
    await wait(() => document.getElementById('executable').value);
    document.getElementById('launch').click();
    await wait(() => window.vestaTarget()?.state === 'idle');
    await pause(2000);
    const captures = [];
    for (let index = 0; index < 2; index++) {
      if (!document.getElementById('capture-controls').hidden) document.getElementById('capture-controls').click();
      const previous = document.getElementById('perfetto').contentWindow.app?.trace;
      const oldRevision = window.vestaCapture?.revision ?? 0;
      document.getElementById('start').click();
      await wait(() => window.vestaTarget()?.state === 'recording');
      await pause(1500);
      document.getElementById('stop').click();
      await wait(() => (window.vestaCapture?.revision ?? 0) > oldRevision);
      const evidence = await window.vestaCheckTrace(previous);
      if (!window.vestaCapture.frames) throw new Error('Target produced no actual GPUI frames');
      captures.push({...evidence, frames: window.vestaCapture.frames, gpu_submissions: window.vestaCapture.gpu_submissions});
    }
    document.getElementById('disconnect').click();
    await wait(() => !window.vestaTarget());
    send({type: 'accepted', mode: 'live', captures, repeated_start_stop: true, disconnected: true});
  } catch (reason) { send({type: 'error', message: reason.message}); }
})();
