// PointBlitz server video client (P3.3, decisions 0010, 0036, 0037): WebSocket → WebCodecs → canvas.
//
// URL parameters:
//   ws       = server URL (default ws://127.0.0.1:8720)
//   inputs   = 1 to send a numbered orbit input every 500 ms (input → display latency)
//   scenario = recorded in the marks only (replay | cold | orbit — the server decides what it does)
//
// Hooks on window.__pb, the same shape the measurement harness reads (decision 0020): marks
// [{name, t, ...}] with snapshot_received / presented / fetch_end, frames [rAF t], longtasks,
// plus inputs [{id, sent, shown}] and cycles {total, missed} (SPEC §7.1 frame drop: 60 Hz cycles
// in which no new video frame was drawn).

const params = new URLSearchParams(location.search);
const url = params.get('ws') ?? 'ws://127.0.0.1:8720';
const sendInputs = params.get('inputs') === '1';
// Frame pacing (decisions 0039–0041). 'immediate' (default) draws each frame as it is decoded.
// 'buffer' draws one queued frame per display cycle with one spare, so the phase between the
// server tick and vsync no longer shows as empty/double cycles; 'adaptive' is 'buffer' that drops
// the spare once it has gone unused for TRIM_WINDOW cycles. Both smooth the stutter but cost
// latency: adaptive missed the acceptance bound in replay under C1–C4 (45.1 > 40 ms), so the
// default stayed immediate (supervisor rule, PR #37 review).
const pacing = ['buffer', 'adaptive'].includes(params.get('pacing')) ? params.get('pacing') : 'immediate';
const SLACK = 2; // frames queued before drawing starts (head + one spare); more are dropped oldest-first
const TRIM_WINDOW = 120; // cycles (2 s at 60 Hz) in a row with the spare unused before it is dropped
const pb = (window.__pb = {
  marks: [],
  frames: [],
  longtasks: [],
  syncFrames: [],
  inputs: [],
  decodeMs: [], // message received → decoded frame out, per frame
  // P4.3 diagnosis: per frame [server tick ms, received, drawn] and frames drawn per display cycle.
  frameTimes: [],
  cycleDraws: [],
  cycles: { total: 0, missed: 0 },
  pacing: { mode: pacing, dropped: 0, trimmed: 0 },
  ready: false,
  done: false,
  view,
});
const mark = (name, extra = {}) => pb.marks.push({ name, t: performance.now(), ...extra });
new PerformanceObserver((list) => {
  for (const e of list.getEntries()) pb.longtasks.push({ t: e.startTime, ms: e.duration });
}).observe({ type: 'longtask', buffered: true });

const canvas = document.getElementById('view');
const ctx = canvas.getContext('2d', { alpha: false });
const metaByFrame = new Map();
let decoder = null;
let bytes = 0;
let drawnSinceCycle = false;
let drawnThisCycle = 0;
let streaming = false;
let drawnCount = 0;
const viewWaiters = [];
let viewId = 1_000_000; // separate from the latency inputs

/**
 * Fixed-viewpoint capture hook (P3.4): asks the server for a viewpoint and resolves once a frame
 * showing it has been drawn and 30 more frames followed (steady quality, not the first P frame
 * after the jump).
 */
function view(name) {
  const id = viewId++;
  ws.send(JSON.stringify({ id, type: 'view', name }));
  return new Promise((resolve) => viewWaiters.push({ id, resolve, settleUntil: null }));
}
let orbitStarted = false;

/** avc1.PPCCLL from the first SPS (NAL type 7) in an Annex B access unit. */
function codecFromSps(b) {
  for (let i = 0; i + 6 < b.length; i++) {
    if (b[i] === 0 && b[i + 1] === 0 && b[i + 2] === 1 && (b[i + 3] & 0x1f) === 7) {
      const hex = (x) => x.toString(16).padStart(2, '0');
      return `avc1.${hex(b[i + 4])}${hex(b[i + 5])}${hex(b[i + 6])}`;
    }
  }
  return null;
}

const queue = [];
let primed = false;
let spareUnused = 0; // consecutive draws that found the spare already there (adaptive)
// When a drawn frame reaches the screen: a draw inside the rAF callback shows in that rendering
// update (its timestamp), a draw outside it in the next one. Inputs and marks drawn outside rAF
// wait here for the next cycle's timestamp (decision 0040: input → screen, not input → draw).
let rafT = null;
const awaitingScreen = [];
const onScreen = (o) => (rafT != null ? (o.screen = rafT) : awaitingScreen.push(o));
// Phase report (decision 0042): where decoded frames land in the display cycle, as the circular
// mean of 30 frames (the first after 10); with `serve --phase-lock on` the server moves its tick so
// they land at φ = 0.65 — a frame drawn outside rAF shows at the next rendering update, so this is
// ~6 ms before it, clear of the boundary. Frames that show a new snapshot (a swap: the server is late)
// and decodes over 3 ms are left out so a passing delay does not move the tick. ?phase=0: no reports.
const reportPhase = params.get('phase') !== '0';
const PHASE_TARGET = 0.65;
const phaseSum = { c: 0, s: 0, n: 0, reports: 0 };
pb.phaseReports = [];
function notePhase(t, meta) {
  const f = pb.frames;
  if (!reportPhase || f.length < 10) return;
  if (!meta || meta.visible.length || t - meta.rx > 3) return;
  const d = [];
  for (let i = Math.max(1, f.length - 60); i < f.length; i++) d.push(f[i] - f[i - 1]);
  d.sort((a, b) => a - b);
  const period = d[d.length >> 1];
  const phi = (((t - f[f.length - 1]) / period) % 1 + 1) % 1;
  phaseSum.c += Math.cos(2 * Math.PI * phi);
  phaseSum.s += Math.sin(2 * Math.PI * phi);
  if (++phaseSum.n < (phaseSum.reports ? 30 : 10)) return;
  phaseSum.reports++;
  const mean = ((Math.atan2(phaseSum.s, phaseSum.c) / (2 * Math.PI)) % 1 + 1) % 1;
  // Not wrapped: φ̄ is in [0, 1), so moving by (target − φ̄) never crosses the refresh boundary (φ = 0/1),
  // where frames would fall into the neighbouring cycle on the way.
  const err = (PHASE_TARGET - mean) * period;
  phaseSum.c = phaseSum.s = phaseSum.n = 0;
  pb.phaseReports.push([t, mean, err]);
  if (ws.readyState === WebSocket.OPEN) ws.send(JSON.stringify({ type: 'phase', err_ms: err, period_ms: period }));
}
function onFrame(frame) {
  const meta = metaByFrame.get(frame.timestamp);
  if (meta) pb.decodeMs.push(performance.now() - meta.rx);
  metaByFrame.delete(frame.timestamp);
  notePhase(performance.now(), meta);
  if (pacing === 'immediate') return present(frame, meta);
  queue.push({ frame, meta });
  while (queue.length > SLACK) {
    queue.shift().frame.close();
    pb.pacing.dropped++;
  }
}
function presentQueued() {
  if (!primed && queue.length >= SLACK) primed = true;
  if (!primed) return;
  // Adaptive: the spare was never needed for TRIM_WINDOW draws in a row — drop the head and draw
  // the newer frame, one cycle less waiting. It comes back by re-priming after the next dry cycle.
  spareUnused = queue.length >= 2 ? spareUnused + 1 : 0;
  if (pacing === 'adaptive' && spareUnused >= TRIM_WINDOW) {
    queue.shift().frame.close();
    pb.pacing.trimmed++;
    spareUnused = 0;
  }
  const next = queue.shift();
  // Ran dry: wait for the spare again so one late arrival does not leave an empty cycle.
  if (!next) return void (primed = false);
  present(next.frame, next.meta);
}
function present(frame, meta) {
  ctx.drawImage(frame, 0, 0, canvas.width, canvas.height);
  frame.close();
  const t = performance.now();
  drawnSinceCycle = true;
  drawnThisCycle++;
  if (meta) pb.frameTimes.push([meta.t_ms, meta.rx, t, meta.size]);
  if (!meta) return;
  // Frame-drop window (P3.4, supervisor decision): from the first drawn frame to the end of the
  // orbit — the server marks frames after it as phase "done".
  if (meta.phase === 'orbit' && !orbitStarted) {
    orbitStarted = true;
    mark('orbit_start', { frame: meta.frame });
  }
  if (meta.phase === 'done' && streaming) {
    streaming = false;
    mark('orbit_end', { frame: meta.frame });
  }
  // A snapshot is presented when the first frame that shows it is drawn.
  for (const seq of meta.visible) {
    mark('presented', { seq, frame: meta.frame });
    onScreen(pb.marks[pb.marks.length - 1]);
  }
  for (const inp of pb.inputs) {
    if (inp.shown == null && inp.id <= meta.input) {
      inp.shown = t;
      onScreen(inp);
    }
  }
  drawnCount++;
  for (const w of viewWaiters.splice(0)) {
    if (meta.input >= w.id) {
      if (w.settleUntil == null) w.settleUntil = drawnCount + 30;
    }
    if (w.settleUntil != null && drawnCount >= w.settleUntil) w.resolve();
    else viewWaiters.push(w);
  }
}

function onMessage(ev) {
  if (typeof ev.data === 'string') {
    const m = JSON.parse(ev.data);
    if (m.type === 'snapshot') mark('snapshot_received', { seq: m.seq, kind: m.kind });
    return;
  }
  const view = new DataView(ev.data);
  const len = view.getUint32(0, true);
  const meta = JSON.parse(new TextDecoder().decode(new Uint8Array(ev.data, 4, len)));
  const au = new Uint8Array(ev.data, 4 + len);
  bytes += au.length;
  if (!decoder) {
    if (!meta.idr) return; // join at an IDR
    decoder = new VideoDecoder({ output: onFrame, error: (e) => mark('error', { message: String(e) }) });
    decoder.configure({ codec: codecFromSps(au), optimizeForLatency: true, hardwareAcceleration: 'prefer-hardware' });
    streaming = true;
    mark('first_frame_received', { frame: meta.frame });
  }
  meta.rx = performance.now();
  meta.size = ev.data.byteLength;
  metaByFrame.set(meta.frame, meta);
  decoder.decode(new EncodedVideoChunk({ type: meta.idr ? 'key' : 'delta', timestamp: meta.frame, data: au }));
}

function cycle(t) {
  pb.frames.push(t);
  for (const o of awaitingScreen.splice(0)) o.screen = t;
  if (streaming) {
    pb.cycles.total++;
    if (!drawnSinceCycle) pb.cycles.missed++;
    pb.cycleDraws.push(drawnThisCycle);
  }
  drawnSinceCycle = false;
  drawnThisCycle = 0;
  rafT = t;
  if (pacing !== 'immediate') presentQueued();
  rafT = null;
  requestAnimationFrame(cycle);
}

const ws = new WebSocket(url);
ws.binaryType = 'arraybuffer';
ws.onopen = () => {
  mark('start', { scenario: params.get('scenario') ?? 'replay' });
  pb.ready = true;
  requestAnimationFrame(cycle);
  if (sendInputs) {
    let id = 1;
    setInterval(() => {
      if (!streaming || ws.readyState !== WebSocket.OPEN) return;
      ws.send(JSON.stringify({ id, type: 'orbit', dx: 4, dy: 0 }));
      pb.inputs.push({ id, sent: performance.now(), shown: null });
      id++;
    }, 500);
  }
};
// Interactive controls (P4.7 demo): the server moves its camera, as in the native viewer — drag to
// orbit, wheel to zoom, keys 1–8 for the fixed viewpoints. Ids share the capture hook's range, apart
// from the latency inputs above; only real input sends anything.
const viewNames = fetch('/static/bench/viewpoints/flight-01.json')
  .then((r) => r.json())
  .then((v) => v.viewpoints.map((p) => p.name))
  .catch(() => []);
const sendUser = (msg) => ws.readyState === WebSocket.OPEN && ws.send(JSON.stringify({ id: viewId++, ...msg }));
{
  let drag = null;
  canvas.addEventListener('pointerdown', (e) => {
    drag = [e.clientX, e.clientY];
    canvas.setPointerCapture(e.pointerId);
  });
  canvas.addEventListener('pointerup', () => (drag = null));
  canvas.addEventListener('pointermove', (e) => {
    if (!drag) return;
    sendUser({ type: 'orbit', dx: e.clientX - drag[0], dy: e.clientY - drag[1] });
    drag = [e.clientX, e.clientY];
  });
  canvas.addEventListener('wheel', (e) => {
    e.preventDefault();
    sendUser({ type: 'zoom', steps: e.deltaY < 0 ? 1 : -1 });
  }, { passive: false });
  addEventListener('keydown', async (e) => {
    const name = (await viewNames)[Number(e.key) - 1];
    if (name) sendUser({ type: 'view', name });
  });
}
ws.onmessage = onMessage;
ws.onerror = () => mark('error', { message: 'websocket error' });
ws.onclose = async () => {
  streaming = false;
  if (decoder && decoder.state === 'configured') await decoder.flush().catch(() => {});
  mark('fetch_end', { bytes });
  pb.done = true;
};
