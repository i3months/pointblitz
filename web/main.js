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
//
// Hooks on window.__pb (read by bench/web/*.mjs and the baseline harness): marks [{name, t, ...}],
// frames [t], longtasks [{t, ms}], info, gpu, setView(name), ready, done. Mark names follow the
// baseline and the native client (decision 0020): snapshot_received, fetch_start, headers,
// first_chunk, last_chunk_received, uploaded, submitted, presented, fetch_end.

import { ChunkSplitter } from './chunks.js';
import { glTimer } from './gltimer.js';

const params = new URLSearchParams(location.search);
const scenario = params.get('scenario') ?? 'still';
const speed = Number(params.get('speed') ?? 60);
const backend = params.get('backend') ?? 'auto';
const memoryHints = params.get('memory') ?? 'memory'; // A/B: memory | performance (decision 0034)
const pkgDir = params.get('pkg') === 'webgpu' ? './pkg-webgpu/' : './pkg/';
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
let toPresent = []; // snapshots whose last chunk is in the scene but not drawn yet
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
    for (const seq of seqs) mark('submitted', { seq });
    const w = waiters;
    waiters = [];
    // presented = the GPU finished the first frame that shows the whole snapshot (decision 0020).
    // Only asked for when someone waits: Chrome keeps memory per onSubmittedWorkDone after a frame
    // (decision 0033), so ordinary frames do not call it.
    if (seqs.length || w.length) {
      viewer.gpu_done().then(() => {
        for (const seq of seqs) mark('presented', { seq });
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
  const splitter = new ChunkSplitter((chunk, last) => {
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
    if (last) {
      lastSeen = true;
      mark('uploaded', { seq: snap.seq, points: viewer.points(), gpu_bytes: viewer.gpu_bytes(), upload_ms: uploadMs, chunk_max_ms: chunkMaxMs });
      toPresent.push(snap.seq);
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
  // WebGPU does not give wgpu the adapter name; the browser's GPUAdapter.info says which GPU it is.
  const a = navigator.gpu && pb.info.backend === 'BrowserWebGpu' ? await navigator.gpu.requestAdapter({ powerPreference: 'high-performance' }) : null;
  pb.gpu = a && { vendor: a.info.vendor, architecture: a.info.architecture, device: a.info.device, description: a.info.description };
  mark('viewer', { info: pb.info, gpu: pb.gpu });
  requestAnimationFrame(frame);
  if (scenario === 'check') return;

  const vp = await (await fetch('/static/bench/viewpoints/flight-01.json')).json();
  viewpoints = vp.viewpoints;
  fov = vp.camera.fov_y_deg;
  await setView(params.get('view') ?? 'overview_sw');
  if (scenario !== 'orbit') attachControls(document.getElementById('view'));
  const manifest = (await (await fetch('/manifest.json')).json()).events;
  mark('start', { scenario });
  pb.ready = true;

  if (scenario === 'replay') await replay();
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
