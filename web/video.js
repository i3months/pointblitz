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
const pb = (window.__pb = {
  marks: [],
  frames: [],
  longtasks: [],
  syncFrames: [],
  inputs: [],
  decodeMs: [], // message received → decoded frame out, per frame
  cycles: { total: 0, missed: 0 },
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

function onFrame(frame) {
  const meta = metaByFrame.get(frame.timestamp);
  if (meta) pb.decodeMs.push(performance.now() - meta.rx);
  metaByFrame.delete(frame.timestamp);
  ctx.drawImage(frame, 0, 0, canvas.width, canvas.height);
  frame.close();
  const t = performance.now();
  drawnSinceCycle = true;
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
  for (const seq of meta.visible) mark('presented', { seq, frame: meta.frame });
  for (const inp of pb.inputs) if (inp.shown == null && inp.id <= meta.input) inp.shown = t;
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
  metaByFrame.set(meta.frame, meta);
  decoder.decode(new EncodedVideoChunk({ type: meta.idr ? 'key' : 'delta', timestamp: meta.frame, data: au }));
}

function cycle(t) {
  pb.frames.push(t);
  if (streaming) {
    pb.cycles.total++;
    if (!drawnSinceCycle) pb.cycles.missed++;
  }
  drawnSinceCycle = false;
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
ws.onmessage = onMessage;
ws.onerror = () => mark('error', { message: 'websocket error' });
ws.onclose = async () => {
  streaming = false;
  if (decoder && decoder.state === 'configured') await decoder.flush().catch(() => {});
  mark('fetch_end', { bytes });
  pb.done = true;
};
