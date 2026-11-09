// Finite acceptance only. Private probes are pinned to the bundled Perfetto version.
window.vestaCheckTrace = async previous => {
    const frame = document.getElementById('perfetto');
    const deadline = Date.now() + 45000;
    let trace;
    while (Date.now() < deadline) {
      trace = frame.contentWindow.app?.trace;
      if (trace && trace !== previous && !frame.contentWindow.app.isLoadingTrace && frame.contentDocument.querySelector('.pf-track')) break;
      await new Promise(resolve => setTimeout(resolve, 100));
    }
    if (!trace || trace === previous) throw new Error('Perfetto did not parse the capture');
    const result = await trace.engine.query('select count(*) as slices from slice');
    const slices = Number(result.firstRow({}).get('slices'));
    if (slices < 1 || slices !== window.vestaCapture.slices) throw new Error(`Perfetto slice count ${slices} differs from captured spans/markers ${window.vestaCapture.slices}`);
    const tracks = frame.contentDocument.querySelectorAll('.pf-track').length;
    if (!tracks) throw new Error('Perfetto did not render any tracks');
    const bounds = frame.getBoundingClientRect();
    if (!document.body.classList.contains('analysis') || bounds.x !== 0 || bounds.y !== 0 || Math.abs(bounds.width - innerWidth) > 1 || Math.abs(bounds.height - innerHeight) > 1) throw new Error('Analyzer does not fill the content area');
    return {type: 'accepted', slices, tracks, events: window.vestaCapture.events,
      source: window.vestaCapture.source.sha256, version: 'v59.1', fills_window: true};
};
if (!window.vestaLiveProbe) {
  window.vestaCheckTrace().then(value => window.ipc.postMessage(JSON.stringify(value)))
    .catch(reason => window.ipc.postMessage(JSON.stringify({type: 'error', message: reason.message})));
}
