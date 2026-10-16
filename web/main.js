// PointBlitz in the browser (P2, decisions 0028, 0029, 0030): the wasm viewer on a WebGPU canvas.
//
// URL parameters:
//   scenario = check | still | replay | cold | orbit
//     check   report the adapter only (P2.1)
//     still   load the last snapshot, then hold (fixed-viewpoint captures, P2.2)
//     replay  follow /events like the native client (decision 0026), chunks streamed as they arrive
//     cold    only the last snapshot of the manifest, as one full delivery
//     orbit   cold, then 120 synchronised frames at each fixed viewpoint (SPEC §6.2 frame_time)
//   speed    = replay speed factor (default 60)
//   view     = viewpoint to start at (default overview_sw)
//   backend  = auto | webgpu | webgl   (auto: WebGPU when available, otherwise WebGL2 — P2.4)
//   pkg      = full | webgpu           (webgpu: the WebGPU-only module, 0.37 MB instead of 4.25 MB — decision 0033)
//   viewpoints = file under /static/ (default bench/viewpoints/flight-01.json)
//   upload   = mirror | scatter        (slot streams, decision 0052 §2 A/B: method (a) CPU mirror, the
//                                       default, or (b) compute-shader scatter; WebGL2 always mirror)
//
// A replay server of a slot diff stream (`replay --stream`, decision 0052; "stream":"slots" in its
// manifest) announces `slots` events: each event's message (/slots/<seq>) is fetched as it is
// announced and applied whole to the viewer's slot scene, in seq order; one that cannot be fetched
// is replaced by the folded state through it. Cold fetches the folded state (/slots/state?seq=<last>). The marks are the
// same; `uploaded` carries upload_ms (apply time) and upload_bytes. Without `viewpoints` the camera
// frames the first state.
//
// Hooks on window.__pb (read by bench/web/*.mjs and the baseline harness): marks [{name, t, ...}],
// frames [t], longtasks [{t, ms}], info, gpu, setView(name), ready, done. Mark names follow the
// baseline and the native client (decision 0020): snapshot_received, fetch_start, headers,
// first_chunk, last_chunk_received, uploaded, submitted, presented, fetch_end; for a coarse-first
// delivery also first_pass_uploaded and first_presented (decision 0051: the new generation is shown).

import { ChunkSplitter } from './chunks.js';
import { glTimer } from './gltimer.js';

const params = new URLSearchParams(location.search);
const scenario = params.get('scenario') ?? 'still';
const speed = Number(params.get('speed') ?? 60);
const backend = params.get('backend') ?? 'auto';
const memoryHints = params.get('memory') ?? 'memory'; // A/B: memory | performance (decision 0034)
const pkgDir = params.get('pkg') === 'webgpu' ? './pkg-webgpu/' : './pkg/';
const slotUpload = params.get('upload') ?? 'mirror'; // A/B: mirror | scatter (decision 0052 §2)
let Viewer = null;
const pb = (window.__pb = { marks: [], frames: [], longtasks: [], syncFrames: [], stages: [], ready: false, done: false, setView });
const mark = (name, extra = {}) => pb.marks.push({ name, t: performance.now(), ...extra });
// Main-thread blocks over 50 ms (SPEC §6.2 main_thread_block), same observer as the baseline.
new PerformanceObserver((list) => {
  for (const e of list.getEntries()) pb.longtasks.push({ t: e.startTime, ms: e.duration });
}).observe({ type: 'longtask', buffered: true });

let viewer = null;
let viewpoints = [];
let fov = 50;
let dirty = false;
let waiters = [];
let toPresent = []; // [seq, 'first' | 'all'] in the scene but not drawn yet
let stopLoop = false; // orbit draws its own frames

// Draw only when something changed (decision 0027); the loop itself costs nothing when idle.
function frame(t) {
  pb.frames.push(t);
  viewer?.poll(); // completion callbacks on WebGL2 (decision 0031); no-op on WebGPU
  if (dirty && viewer && !stopLoop) {
    dirty = false;
    viewer.render();
    const seqs = toPresent;
    toPresent = [];
    for (const [seq, what] of seqs) if (what === 'all') mark('submitted', { seq });
    const w = waiters;
    waiters = [];
    // presented = the GPU finished the first frame that shows the whole snapshot (decision 0020).
    // Only asked for when someone waits: Chrome keeps memory per onSubmittedWorkDone after a frame
    // (decision 0033), so ordinary frames do not call it.
    if (seqs.length || w.length) {
      viewer.gpu_done().then(() => {
        for (const [seq, what] of seqs) mark(what === 'all' ? 'presented' : 'first_presented', { seq });
        for (const resolve of w) resolve();
      });
    }
  }
  requestAnimationFrame(frame);
}

/** Resolves once the next drawn frame is finished on the GPU. */
function redraw() {
  dirty = true;
  return new Promise((resolve) => waiters.push(resolve));
}

let cam = null; // { eye, target, up } as plain arrays, for the interactive controls

async function setView(name) {
  const v = viewpoints.find((x) => x.name === name);
  if (!v) throw new Error(`unknown viewpoint ${name}`);
  cam = { eye: [...v.eye], target: [...v.target], up: [...v.up] };
  viewer.set_camera(new Float64Array(v.eye), new Float64Array(v.target), new Float64Array(v.up), fov);
  await redraw();
  mark('view', { view: name });
}

// Interactive controls (P4.7 demo), the same as the native viewer and the server: drag to orbit
// (0.005 rad per pixel), wheel to zoom (×0.9 per step), keys 1–8 for the fixed viewpoints. Only
// real input moves the camera, so measurement runs are unchanged.
const sub = (a, b) => a.map((x, i) => x - b[i]);
const add = (a, b) => a.map((x, i) => x + b[i]);
const scale = (a, s) => a.map((x) => x * s);
const dot = (a, b) => a.reduce((s, x, i) => s + x * b[i], 0);
const cross = (a, b) => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
const norm = (a) => scale(a, 1 / (Math.hypot(...a) || 1));
const rotate = (v, k, angle) => add(add(scale(v, Math.cos(angle)), scale(cross(k, v), Math.sin(angle))), scale(k, dot(k, v) * (1 - Math.cos(angle))));
function applyCamera() {
  viewer.set_camera(new Float64Array(cam.eye), new Float64Array(cam.target), new Float64Array(cam.up), fov);
  dirty = true;
}
function attachControls(canvas) {
  let drag = null;
  canvas.addEventListener('pointerdown', (e) => {
    drag = [e.clientX, e.clientY];
    canvas.setPointerCapture(e.pointerId);
  });
  canvas.addEventListener('pointerup', () => (drag = null));
  canvas.addEventListener('pointermove', (e) => {
    if (!drag || !cam) return;
    const [dx, dy] = [e.clientX - drag[0], e.clientY - drag[1]];
    drag = [e.clientX, e.clientY];
    const up = norm(cam.up);
    const yaw = rotate(sub(cam.eye, cam.target), up, -dx * 0.005);
    const right = norm(cross(yaw, up));
    const pitched = rotate(yaw, right, -dy * 0.005);
    cam.eye = add(cam.target, Math.abs(dot(norm(pitched), up)) < 0.995 ? pitched : yaw);
    applyCamera();
  });
  canvas.addEventListener('wheel', (e) => {
    if (!cam) return;
    e.preventDefault();
    cam.eye = add(cam.target, scale(sub(cam.eye, cam.target), 0.9 ** (e.deltaY < 0 ? 1 : -1)));
    applyCamera();
  }, { passive: false });
  addEventListener('keydown', (e) => {
    const v = viewpoints[Number(e.key) - 1];
    if (v) setView(v.name);
  });
}

let have = null; // [generation, points] the scene holds (decision 0026)

/** Fetches one snapshot's chunks and puts each into the scene as soon as it is complete. */
async function deliver(snap) {
  const q = have ? `?have=${have[0]}.${have[1]}` : '';
  mark('fetch_start', { seq: snap.seq });
  const res = await fetch(`/chunks/${snap.seq}${q}`, { cache: 'no-store' });
  if (!res.ok) throw new Error(`chunks ${snap.seq}: ${res.status}`);
  const delivery = res.headers.get('X-PB-Delivery');
  const generation = Number(res.headers.get('X-PB-Generation'));
  mark('headers', { seq: snap.seq });
  let bytes = 0;
  let first = true;
  let uploadMs = 0;
  let chunkMaxMs = 0; // longest single chunk on the main thread (PR #16 review: must stay < 50 ms)
  let lastSeen = false;
  const splitter = new ChunkSplitter((chunk, last, firstPass) => {
    const t0 = performance.now();
    if (first) {
      first = false;
      mark('first_chunk', { seq: snap.seq });
    }
    if (last) mark('last_chunk_received', { seq: snap.seq });
    viewer.insert(chunk);
    const ms = performance.now() - t0;
    uploadMs += ms;
    chunkMaxMs = Math.max(chunkMaxMs, ms);
    dirty = true; // progressive first generation and appends are visible right away
    if (firstPass && !last) {
      // The new generation is in the scene from here on (decision 0051): its first presented frame
      // is the snapshot's first reflection.
      mark('first_pass_uploaded', { seq: snap.seq, points: viewer.points() });
      toPresent.push([snap.seq, 'first']);
    }
    if (last) {
      lastSeen = true;
      mark('uploaded', { seq: snap.seq, points: viewer.points(), gpu_bytes: viewer.gpu_bytes(), upload_ms: uploadMs, chunk_max_ms: chunkMaxMs });
      toPresent.push([snap.seq, 'all']);
    }
  });
  const reader = res.body.getReader();
  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    bytes += value.length;
    splitter.push(value);
  }
  if (!lastSeen) throw new Error(`chunks ${snap.seq}: delivery ended without its last chunk`);
  have = [generation, snap.points];
  mark('fetch_end', { seq: snap.seq, bytes, delivery, generation });
  mark('delivered', { seq: snap.seq, bytes, delivery, generation });
}

let fitPending = false; // slot stream without viewpoints: frame the first state

/** Frames the slot scene from the south-west and above (like overview_sw, decision 0017). */
function fitCamera() {
  const b = viewer.slot_bounds();
  if (b.length !== 6) return;
  fitPending = false;
  const c = [0, 1, 2].map((a) => 0.5 * (b[a] + b[a + 3]));
  const r = Math.max(1, 0.5 * Math.hypot(b[3] - b[0], b[4] - b[1], b[5] - b[2]));
  const d = r / Math.sin(((fov / 2) * Math.PI) / 180);
  cam = { eye: add(c, scale(norm([-1, -1, 1.4]), d)), target: c, up: [0, 0, 1] };
  applyCamera();
}

/** GET as bytes; throws on a failed request. */
async function getBytes(url) {
  const res = await fetch(url, { cache: 'no-store' });
  if (!res.ok) throw new Error(`${url}: ${res.status}`);
  return new Uint8Array(await res.arrayBuffer());
}

const slotAttempts = 3; // tries per event before falling back to the folded state

/**
 * Fetches one slot message (an event, or `state` = everything through it) and applies it whole.
 * Events are fetched as they are announced, so they may complete out of order: the viewer holds a
 * message until the ones before it are in. An event that cannot be fetched after retries is
 * replaced by the folded state through it, applied with apply_slots_state (otherwise every later
 * event would wait for it, and without event 1 the scene would never start).
 */
async function deliverSlots(ev, state = false) {
  mark('fetch_start', { seq: ev.seq });
  let bytes = null;
  for (let k = 0; !state && k < slotAttempts && !bytes; k++) {
    if (k > 0) await new Promise((r) => setTimeout(r, 100));
    bytes = await getBytes(`/slots/${ev.seq}`).catch((e) => (mark('slots_fetch_error', { seq: ev.seq, message: String(e) }), null));
  }
  if (!bytes) {
    state = true;
    bytes = await getBytes(`/slots/state?seq=${ev.seq}`);
  }
  const delivery = state ? 'slots_state' : 'slots';
  mark('headers', { seq: ev.seq });
  mark('last_chunk_received', { seq: ev.seq });
  mark('fetch_end', { seq: ev.seq, bytes: bytes.length, delivery });
  mark('delivered', { seq: ev.seq, bytes: bytes.length, delivery });
  const t0 = performance.now();
  const r = state ? viewer.apply_slots_state(bytes) : viewer.apply_slots(bytes);
  const ms = performance.now() - t0;
  if (r === 'held' || r === 'stale') {
    mark('slots_' + r, { seq: ev.seq });
    return;
  }
  const { applied } = JSON.parse(r);
  mark('uploaded', { seq: ev.seq, points: viewer.points(), gpu_bytes: viewer.gpu_bytes(), upload_ms: ms, upload_bytes: viewer.slot_upload_bytes(), upload: viewer.slot_upload(), applied });
  for (const seq of applied) toPresent.push([seq, 'all']);
  if (fitPending) fitCamera();
  dirty = true;
}

async function replaySlots() {
  // Each event is fetched the moment it is announced (events 8–10 of theater30 come 70 ms apart);
  // the slot scene applies them in seq order.
  const deliveries = [];
  await new Promise((resolve, reject) => {
    const es = new EventSource(`/events?speed=${speed}`);
    es.addEventListener('slots', (m) => {
      const ev = JSON.parse(m.data);
      mark('snapshot_received', { seq: ev.seq, kind: ev.kind });
      const d = deliverSlots(ev);
      d.catch(reject);
      deliveries.push(d);
    });
    es.addEventListener('end', () => {
      es.close();
      Promise.all(deliveries).then(resolve, reject);
    });
    es.onerror = () => {
      if (es.readyState === EventSource.CLOSED) reject(new Error('event stream closed'));
    };
  });
  await allSlotsPresented();
}

/**
 * Resolves when every received event has been presented. Counting uploads (allPresented) does not
 * work here: a held event has no upload of its own and is presented with the one that releases it.
 */
function allSlotsPresented() {
  return new Promise((resolve) => {
    const check = () => {
      const presented = new Set(pb.marks.filter((m) => m.name === 'presented').map((m) => m.seq));
      const missing = pb.marks.filter((m) => m.name === 'snapshot_received' && !presented.has(m.seq));
      if (missing.length === 0 && toPresent.length === 0) resolve();
      else requestAnimationFrame(check);
    };
    check();
  });
}

async function coldSlots(events) {
  const last = events[events.length - 1];
  mark('snapshot_received', { seq: last.seq, kind: last.kind });
  await deliverSlots(last, true);
  await allSlotsPresented();
}

/** Resolves when every delivered snapshot has been presented. */
function allPresented() {
  return new Promise((resolve) => {
    const check = () => {
      const want = pb.marks.filter((m) => m.name === 'uploaded').length;
      const got = pb.marks.filter((m) => m.name === 'presented').length;
      if (got >= want && toPresent.length === 0) resolve();
      else requestAnimationFrame(check);
    };
    check();
  });
}

async function replay() {
  // Snapshots are fetched strictly in order, like the native client; event times are stamped the
  // moment the message arrives, so a delivery in progress does not delay them.
  let chain = Promise.resolve();
  await new Promise((resolve, reject) => {
    const es = new EventSource(`/events?speed=${speed}`);
    es.addEventListener('snapshot', (m) => {
      const snap = JSON.parse(m.data);
      mark('snapshot_received', { seq: snap.seq, kind: snap.kind });
      chain = chain.then(() => deliver(snap));
      chain.catch(reject);
    });
    es.addEventListener('end', () => {
      es.close();
      chain.then(resolve, reject);
    });
    es.onerror = () => {
      if (es.readyState === EventSource.CLOSED) reject(new Error('event stream closed'));
    };
  });
  await allPresented();
}

async function cold(manifest) {
  const last = manifest[manifest.length - 1];
  mark('snapshot_received', { seq: last.seq, kind: last.kind });
  await deliver(last);
  await allPresented();
}

// Synchronised frames (decision 0020), like the baseline and native orbit, drawn offscreen like
// native (decision 0033). cpu = render_offscreen() call (encode + submit); total = until the GPU is
// done: the frame's first pixel is copied from the offscreen target to a small buffer and mapped
// (the map resolves only after the GPU finished the frame) — on WebGPU and WebGL2 alike (decision
// 0049). The WebGL2 path used to readPixels the canvas instead, which never waits for an offscreen
// frame: the first ~480 frames looked like 0.1 ms until the queue filled (P4.6 to p48). The
// asynchronous resolution makes total an upper bound.
async function orbit() {
  const framesPerView = Number(params.get('frames') ?? 120);
  const batch = Math.max(1, Number(params.get('batch') ?? 1));
  for (let i = 0; i < 3; i++) await redraw(); // let the swapped snapshot reach the screen first
  stopLoop = true;
  // glsync=canvas: the old WebGL2 sync, for the before/after check only (decision 0049).
  const gl = pb.info.backend === 'Gl' && params.get('glsync') === 'canvas' ? document.getElementById('view').getContext('webgl2') : null;
  const syncPixel = new Uint8Array(4);
  // GPU timestamps per batch (decision 0049): pass timestamps on WebGPU, EXT_disjoint_timer_query_webgl2
  // on WebGL2 (wgpu's GL backend has no timestamp queries but issues its GL calls inside submit).
  const glt = pb.info.backend === 'Gl' ? glTimer(document.getElementById('view').getContext('webgl2')) : null;
  const passTimer = !glt && viewer.gpu_timer_start(batch);
  pb.gpuTimer = glt ? 'EXT_disjoint_timer_query_webgl2 TIME_ELAPSED around the batch' : passTimer ? 'WebGPU timestamp-query, render pass begin/end' : 'none: no timestamp query on this device';
  mark('sync_start');
  for (const v of viewpoints) {
    viewer.set_camera(new Float64Array(v.eye), new Float64Array(v.target), new Float64Array(v.up), fov);
    // batch=N (P4.1 diagnosis): N frames back to back, one sync, per-frame averages.
    for (let k = 0; k < framesPerView; k += batch) {
      const frame = { view: v.name, batch };
      if (passTimer) viewer.gpu_timer_start(batch);
      const query = glt?.begin();
      const t0 = performance.now();
      for (let j = 0; j < batch; j++) viewer.render_offscreen();
      const t1 = performance.now();
      if (query) glt.end(query, batch, frame);
      if (gl) {
        // glsync=canvas only: read the default framebuffer, then restore wgpu's binding.
        const bound = gl.getParameter(gl.READ_FRAMEBUFFER_BINDING);
        gl.bindFramebuffer(gl.READ_FRAMEBUFFER, null);
        gl.readPixels(0, 0, 1, 1, gl.RGBA, gl.UNSIGNED_BYTE, syncPixel);
        gl.bindFramebuffer(gl.READ_FRAMEBUFFER, bound);
      } else {
        await viewer.readback_done();
      }
      const t2 = performance.now();
      frame.cpu = (t1 - t0) / batch;
      frame.ms = (t2 - t0) / batch;
      if (passTimer) frame.gpu = await viewer.gpu_timer_read(); // after t2: not part of ms
      pb.syncFrames.push(frame);
      if (params.get('stages') === '1') pb.stages.push(viewer.last_stages()); // P4.2: last frame of the batch
    }
  }
  mark('sync_end');
  if (glt && !(await glt.finish())) pb.gpuTimer += ' (some results never arrived)';
  stopLoop = false;
}

async function main() {
  // Start-up split into marks (PR #18 review): module download + compile + instantiate, then the
  // viewer (context, device, pipeline).
  mark('wasm_init_start', { pkg: pkgDir });
  const mod = await import(`${pkgDir}pointblitz_web.js`);
  await mod.default();
  Viewer = mod.Viewer;
  mark('wasm_init_end');
  mark('viewer_create_start');
  viewer = await Viewer.create(document.getElementById('view'), backend, memoryHints);
  mark('viewer_create_end');
  if (params.get('glcopy') === '0') viewer.set_readback_copy(false); // P4.2 A/B only
  pb.info = JSON.parse(viewer.info());
  pb.slotUpload = { asked: slotUpload, used: viewer.set_slot_upload(slotUpload) };
  // WebGPU does not give wgpu the adapter name; the browser's GPUAdapter.info says which GPU it is.
  const a = navigator.gpu && pb.info.backend === 'BrowserWebGpu' ? await navigator.gpu.requestAdapter({ powerPreference: 'high-performance' }) : null;
  pb.gpu = a && { vendor: a.info.vendor, architecture: a.info.architecture, device: a.info.device, description: a.info.description };
  mark('viewer', { info: pb.info, gpu: pb.gpu, slot_upload: pb.slotUpload });
  requestAnimationFrame(frame);
  if (scenario === 'check') return;

  const served = await (await fetch('/manifest.json')).json();
  const slots = served.stream === 'slots';
  const manifest = served.events;
  if (slots && !params.get('viewpoints')) {
    // No fixed viewpoints for this data given: frame the first state (fitCamera).
    fitPending = true;
    cam = { eye: [0, -10, 10], target: [0, 0, 0], up: [0, 0, 1] };
  } else {
    const vp = await (await fetch(`/static/${params.get('viewpoints') ?? 'bench/viewpoints/flight-01.json'}`)).json();
    viewpoints = vp.viewpoints;
    fov = vp.camera.fov_y_deg;
    await setView(params.get('view') ?? 'overview_sw');
  }
  if (scenario !== 'orbit') attachControls(document.getElementById('view'));
  mark('start', { scenario, stream: slots ? 'slots' : 'snapshots' });
  pb.ready = true;

  if (slots && scenario === 'replay') await replaySlots();
  else if (slots && (scenario === 'cold' || scenario === 'still')) await coldSlots(manifest);
  else if (slots && scenario === 'orbit') {
    await coldSlots(manifest);
    await orbit();
  }
  else if (scenario === 'replay') await replay();
  else if (scenario === 'cold' || scenario === 'still') await cold(manifest);
  else if (scenario === 'orbit') {
    await cold(manifest);
    await orbit();
  }
  else throw new Error(`unknown scenario ${scenario}`);
}

main()
  .catch((e) => {
    pb.error = String(e);
    mark('error', { message: pb.error });
    console.error(e);
  })
  .finally(() => {
    pb.ready = pb.done = true;
  });
