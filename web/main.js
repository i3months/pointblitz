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

const params = new URLSearchParams(location.search);
const scenario = params.get('scenario') ?? 'still';
const speed = Number(params.get('speed') ?? 60);
const backend = params.get('backend') ?? 'auto';
const pkgDir = params.get('pkg') === 'webgpu' ? './pkg-webgpu/' : './pkg/';
let Viewer = null;
const pb = (window.__pb = { marks: [], frames: [], longtasks: [], syncFrames: [], ready: false, done: false, setView });
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

async function setView(name) {
  const v = viewpoints.find((x) => x.name === name);
  if (!v) throw new Error(`unknown viewpoint ${name}`);
  viewer.set_camera(new Float64Array(v.eye), new Float64Array(v.target), new Float64Array(v.up), fov);
  await redraw();
  mark('view', { view: name });
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
// done. WebGL2: a 1-pixel readPixels on the same context, exactly the baseline's sync (a WebGL fence
// only reports at frame boundaries, ~16.7 ms). WebGPU: the frame's first pixel is copied to a
// small buffer and mapped (mapAsync resolves only after the GPU finished the frame) — the WebGPU counterpart of
// readPixels; the asynchronous resolution makes total an upper bound.
async function orbit() {
  const framesPerView = Number(params.get('frames') ?? 120);
  for (let i = 0; i < 3; i++) await redraw(); // let the swapped snapshot reach the screen first
  stopLoop = true;
  const gl = pb.info.backend === 'Gl' ? document.getElementById('view').getContext('webgl2') : null;
  const syncPixel = new Uint8Array(4);
  mark('sync_start');
  for (const v of viewpoints) {
    viewer.set_camera(new Float64Array(v.eye), new Float64Array(v.target), new Float64Array(v.up), fov);
    for (let k = 0; k < framesPerView; k++) {
      const t0 = performance.now();
      viewer.render_offscreen();
      const t1 = performance.now();
      if (gl) {
        // Read from the default framebuffer, then restore wgpu's binding.
        const bound = gl.getParameter(gl.READ_FRAMEBUFFER_BINDING);
        gl.bindFramebuffer(gl.READ_FRAMEBUFFER, null);
        gl.readPixels(0, 0, 1, 1, gl.RGBA, gl.UNSIGNED_BYTE, syncPixel);
        gl.bindFramebuffer(gl.READ_FRAMEBUFFER, bound);
      } else {
        await viewer.readback_done();
      }
      const t2 = performance.now();
      pb.syncFrames.push({ view: v.name, cpu: t1 - t0, ms: t2 - t0 });
    }
  }
  mark('sync_end');
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
  viewer = await Viewer.create(document.getElementById('view'), backend);
  mark('viewer_create_end');
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
