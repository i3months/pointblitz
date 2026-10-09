// three.js baselines (decisions 0005, 0019, 0048). One page, three ways to show the same flight:
//
//   mode=full (B0, default) — decision 0005: every snapshot is downloaded whole, parsed on the main
//       thread with PLYLoader, and the previous THREE.Points is thrown away; drawn every rAF.
//   mode=b1 — three.js on PointBlitz's own data path: the /chunks stream (GPU-layout binary, preview
//       snapshots as deltas) goes to the GPU without parsing, chunk by chunk; a refined snapshot
//       becomes visible when its last chunk arrived; drawn only when something changed.
//   mode=b2 — SkyLens-style increments: the PLY (preview snapshots: only the appended records), parsed
//       in a Web Worker, appended or (refined) replaced; drawn only when something changed.
//
// URL parameters:
//   scenario = replay | cold | orbit   (SPEC §6.1); memtest | memspike = memory metric self-checks
//   speed    = replay speed factor (default 60)
//   view     = viewpoint name to hold (default overview_sw)
//   frames   = frames per viewpoint in orbit (default 120)
//   mode     = full | b1 | b2
//   renderer = webgl (default) | webgpu   (b1 only: three.js WebGPURenderer, decision 0048)
//   upload   = a (default) | b            (b1 upload, chosen by smoke — decision 0048)
//   forcewebgl = 1                        (renderer=webgpu only, diagnosis: the same instanced-quad
//                                          drawing on three.js's WebGL2 backend — #72 review)
//
// Measurement hooks live on window.__pb (read by the harness):
//   marks   [{name, t, ...}]  t = performance.now() in ms
//   frames  [t, ...]          requestAnimationFrame timestamps
//   setView(name), ready, done

import { ChunkSplitter } from '/static/web/chunks.js';
import { glTimer } from '/static/web/gltimer.js';

const WIDTH = 1920;
const HEIGHT = 1080;
const POINT_SIZE_PX = 2;

const params = new URLSearchParams(location.search);
const scenario = params.get('scenario') ?? 'replay';
const speed = Number(params.get('speed') ?? 60);
const framesPerView = Number(params.get('frames') ?? 120);
const batch = Math.max(1, Number(params.get('batch') ?? 1));
const mode = params.get('mode') ?? 'full';
const webgpu = params.get('renderer') === 'webgpu';
const upload = params.get('upload') ?? 'a';
if (!['full', 'b1', 'b2'].includes(mode)) throw new Error(`unknown mode ${mode}`);
if (webgpu && mode !== 'b1') throw new Error('renderer=webgpu is only for mode=b1');

// three.webgpu.js is a superset of three.module.js; the two builds must not be mixed on one page.
const THREE = webgpu ? await import('three/webgpu') : await import('three');
const TSL = webgpu ? await import('three/tsl') : null;

const pb = (window.__pb = { marks: [], frames: [], longtasks: [], syncFrames: [], ready: false, done: false, setView, mode });
// Main-thread blocks over 50 ms (SPEC §6.2 main_thread_block).
new PerformanceObserver((list) => {
  for (const e of list.getEntries()) pb.longtasks.push({ t: e.startTime, ms: e.duration });
}).observe({ type: "longtask", buffered: true });
const mark = (name, extra = {}) => pb.marks.push({ name, t: performance.now(), ...extra });

const renderer = webgpu
  ? new THREE.WebGPURenderer({ antialias: false, trackTimestamp: scenario === 'orbit', forceWebGL: params.get('forcewebgl') === '1' }) // timestamps: decision 0049
  : new THREE.WebGLRenderer({ antialias: false, preserveDrawingBuffer: true });
renderer.setPixelRatio(1);
renderer.setSize(WIDTH, HEIGHT);
document.body.appendChild(renderer.domElement);
if (webgpu) {
  await renderer.init();
  const info = renderer.backend.adapter?.info ?? {};
  pb.gpu = { vendor: info.vendor ?? '?', architecture: info.architecture ?? '?' };
  pb.info = { format: renderer.backend.isWebGLBackend ? 'three.js WebGPURenderer, WebGL2 backend (forceWebGL)' : 'three.js WebGPURenderer' };
}
// B1/B2 hand three.js the snapshot's sRGB bytes as they are. With the default sRGB output they
// would be treated as linear and brightened, so the output transfer is switched off instead of
// converting every colour on the CPU (decision 0048; checked by SSIM against B0).
if (mode !== 'full') renderer.outputColorSpace = THREE.LinearSRGBColorSpace;

const scene = new THREE.Scene();
scene.background = new THREE.Color(0x000000);
const camera = new THREE.PerspectiveCamera(50, WIDTH / HEIGHT, 0.1, 5000);
camera.up.set(0, 0, 1); // data is ENU: z is up (decision 0013)

// Round points: PointsMaterial draws squares, so a disk texture + alphaTest cuts them to circles
// (decision 0019). This is how three.js users commonly get round points.
function diskTexture() {
  const c = document.createElement('canvas');
  c.width = c.height = 64;
  const g = c.getContext('2d');
  g.fillStyle = '#fff';
  g.beginPath();
  g.arc(32, 32, 32, 0, Math.PI * 2);
  g.fill();
  const tex = new THREE.CanvasTexture(c);
  // No mipmaps (decision 0025): at 2 px the mip chain averages the disk with its transparent
  // (black) corners and darkens every point by ~10 %. The base level keeps the PLY colour exact.
  if (params.get('mipmaps') === '1') return tex; // A/B only: the old, darkening behaviour
  tex.generateMipmaps = false;
  tex.minFilter = THREE.NearestFilter;
  tex.magFilter = THREE.NearestFilter;
  return tex;
}

// WebGPU has 1-pixel point primitives only, so three.js draws sized points as one instanced quad
// per point (PointsNodeMaterial on a non-Points object) — as PointBlitz does on WebGPU. One shared
// material reads the per-point attributes by name, so new chunks never compile a new pipeline.
function webgpuMaterial() {
  const { attribute, uv, vec4, float, step, length } = TSL;
  const m = new THREE.PointsNodeMaterial({ sizeAttenuation: false });
  m.size = POINT_SIZE_PX;
  // Normalized u16 position (decision 0051), four components because WebGPU has no unorm16x3.
  m.positionNode = attribute('instancePosition', 'vec4').xyz;
  m.colorNode = vec4(attribute('instanceColor', 'vec4').rgb, float(1));
  m.opacityNode = step(length(uv().sub(0.5)), float(0.5)); // 1 inside the disk, 0 outside
  m.alphaTest = 0.5;
  m.alphaToCoverage = false;
  return m;
}

const material = webgpu
  ? webgpuMaterial()
  : new THREE.PointsMaterial({
      size: POINT_SIZE_PX,
      sizeAttenuation: false,
      vertexColors: true,
      map: diskTexture(),
      alphaTest: 0.5,
    });

let points = null; // B0: the one THREE.Points
let viewpoints = [];
let dirty = true; // B1/B2 draw only when something changed

function setView(name) {
  const v = viewpoints.find((x) => x.name === name);
  if (!v) throw new Error(`unknown viewpoint ${name}`);
  camera.position.set(...v.eye);
  camera.up.set(...v.up);
  camera.lookAt(...v.target);
  camera.updateMatrixWorld();
  dirty = true;
  mark('view', { view: name });
}

const loader = mode === 'full' ? new (await import('three/addons/loaders/PLYLoader.js')).PLYLoader() : null;

// B0: one snapshot, the way decision 0005 defines it: whole file, main-thread parse, full replace.
async function loadSnapshotFull(ev) {
  mark('fetch_start', { seq: ev.seq });
  const res = await fetch(ev.url, { cache: 'no-store' });
  const buf = await res.arrayBuffer();
  mark('fetch_end', { seq: ev.seq, bytes: buf.byteLength });
  const geometry = loader.parse(buf);
  const attributeNames = Object.keys(geometry.attributes);
  mark('parse_end', {
    seq: ev.seq,
    points: geometry.getAttribute('position').count,
    attributes: attributeNames.length,
    attributeNames: attributeNames.join(','),
  });
  if (points) {
    scene.remove(points);
    points.geometry.dispose();
  }
  points = new THREE.Points(geometry, material);
  scene.add(points);
  mark('scene_swap', { seq: ev.seq });
  // The swap is visible in the next rendered frame.
  pendingPresent.push(ev.seq);
}

// ---- B1 / B2: a scene made of pieces, appended (preview) or swapped as a generation (refined).

let have = null; // [generation, points] the shown scene holds (same rule as PointBlitz, decision 0026)
let shown = null; // THREE.Group on screen
let shownPoints = 0;

function sphereFromBox(min, max) {
  const center = new THREE.Vector3((min[0] + max[0]) / 2, (min[1] + max[1]) / 2, (min[2] + max[2]) / 2);
  const r = Math.hypot(max[0] - min[0], max[1] - min[1], max[2] - min[2]) / 2;
  return new THREE.Sphere(center, r);
}

// A drawable piece from per-point positions / colours (typed arrays or interleaved views). The
// bounding sphere comes with the data, so three.js never walks the points to compute it.
function piece(position, color, count, sphere) {
  let obj;
  if (webgpu) {
    const g = new THREE.InstancedBufferGeometry();
    // The quad every point is drawn with (PointsNodeMaterial offsets it by the point size).
    g.setAttribute('position', new THREE.Float32BufferAttribute([-0.5, -0.5, 0, 0.5, -0.5, 0, 0.5, 0.5, 0, -0.5, 0.5, 0], 3));
    g.setAttribute('uv', new THREE.Float32BufferAttribute([0, 0, 1, 0, 1, 1, 0, 1], 2));
    g.setIndex([0, 1, 2, 0, 2, 3]);
    g.setAttribute('instancePosition', position);
    g.setAttribute('instanceColor', color);
    g.instanceCount = count;
    g.boundingSphere = sphere;
    obj = new THREE.Mesh(g, material);
  } else {
    const g = new THREE.BufferGeometry();
    g.setAttribute('position', position);
    g.setAttribute('color', color);
    g.boundingSphere = sphere;
    obj = new THREE.Points(g, material);
  }
  return obj;
}

function disposeGroup(g) {
  scene.remove(g);
  for (const o of g.children) o.geometry.dispose();
}

// B1 piece from one PointBlitz chunk (decisions 0022, 0051: 80 B header, then 12 B/point: position
// u16×3 quantised to the chunk's box, a zero u16, colour u8×4). three.js reads the u16 as normalized
// 0 … 1 and the object's position / scale (box minimum / box size) undo the quantisation — no CPU
// parsing, as on the GPU of PointBlitz.
function chunkPiece(chunk) {
  const v = new DataView(chunk.buffer, chunk.byteOffset, 80);
  const n = v.getUint32(16, true);
  const origin = [v.getFloat64(24, true), v.getFloat64(32, true), v.getFloat64(40, true)];
  const f = (o) => [v.getFloat32(o, true), v.getFloat32(o + 4, true), v.getFloat32(o + 8, true)];
  const min = f(48);
  const max = f(60);
  const stride = v.getUint16(72, true);
  if (stride !== 12) throw new Error(`B1 reads stride 12 chunks, got ${stride}`);
  // In the object's own (normalized) space the box is the unit cube.
  const sphere = new THREE.Sphere(new THREE.Vector3(0.5, 0.5, 0.5), Math.sqrt(3) / 2);
  const Attr = webgpu ? THREE.InstancedBufferAttribute : THREE.BufferAttribute;
  // WebGPU has no unorm16x3 vertex format: read four components there and use xyz.
  const posItems = webgpu ? 4 : 3;
  let position, color;
  if (upload === 'a') {
    // (a) the chunk bytes as they are: one interleaved view per type (three.js takes one typed
    // array per InterleavedBuffer), so the same bytes go to the GPU twice — 24 B/point.
    const IB = webgpu ? THREE.InstancedInterleavedBuffer : THREE.InterleavedBuffer;
    position = new THREE.InterleavedBufferAttribute(new IB(new Uint16Array(chunk.buffer, chunk.byteOffset + 80, n * 6), 6), posItems, 0, true);
    color = new THREE.InterleavedBufferAttribute(new IB(new Uint8Array(chunk.buffer, chunk.byteOffset + 80, n * 12), 12), webgpu ? 4 : 3, 8, true);
  } else {
    // (b) split on the CPU into tightly packed position / colour arrays — 10 B/point on the GPU
    // (12 on WebGPU, where the position needs four components).
    const src = new Uint16Array(chunk.buffer, chunk.byteOffset + 80, n * 6);
    const bytes = new Uint8Array(chunk.buffer, chunk.byteOffset + 80, n * 12);
    const pos = new Uint16Array(n * posItems);
    const col = new Uint8Array(n * 4);
    for (let i = 0; i < n; i++) {
      pos[i * posItems] = src[i * 6];
      pos[i * posItems + 1] = src[i * 6 + 1];
      pos[i * posItems + 2] = src[i * 6 + 2];
      col[i * 4] = bytes[i * 12 + 8];
      col[i * 4 + 1] = bytes[i * 12 + 9];
      col[i * 4 + 2] = bytes[i * 12 + 10];
      col[i * 4 + 3] = 255;
    }
    position = new Attr(pos, posItems, true);
    color = new Attr(col, 4, true);
  }
  const obj = piece(position, color, n, sphere);
  obj.position.set(origin[0] + min[0], origin[1] + min[1], origin[2] + min[2]);
  // A flat box side would make a singular matrix; any tiny scale keeps those points in place.
  obj.scale.set(...[0, 1, 2].map((k) => Math.max(max[k] - min[k], 1e-6)));
  obj.updateMatrixWorld();
  return { obj, n };
}

const BYTES_PER_POINT = { b1a: 24, b1b: webgpu ? 12 : 10, b2: 16 };

async function loadSnapshotB1(ev) {
  const q = have ? `?have=${have[0]}.${have[1]}` : '';
  mark('fetch_start', { seq: ev.seq });
  const res = await fetch(`/chunks/${ev.seq}${q}`, { cache: 'no-store' });
  if (!res.ok) throw new Error(`chunks ${ev.seq}: ${res.status}`);
  const delivery = res.headers.get('X-PB-Delivery');
  const generation = Number(res.headers.get('X-PB-Generation'));
  // A delta appends to the shown generation. A full delivery builds a new one: the very first is
  // shown progressively, a later one is swapped in once its coarse first pass is in (decision 0051,
  // as PointBlitz) and filled in as the rest arrives.
  const into = delivery === 'delta' ? shown : new THREE.Group();
  const progressive = delivery !== 'delta' && !shown;
  if (progressive) {
    shown = into;
    scene.add(into);
  }
  let bytes = 0;
  let added = 0;
  let lastSeen = false;
  const splitter = new ChunkSplitter((chunk, last, firstPass) => {
    const { obj, n } = chunkPiece(chunk);
    into.add(obj);
    added += n;
    if (firstPass && into !== shown) {
      disposeGroup(shown);
      shown = into;
      scene.add(into);
    }
    if (firstPass && !last) {
      mark('first_pass_uploaded', { seq: ev.seq });
      pendingFirst.push(ev.seq);
    }
    if (into === shown) dirty = true;
    if (last) lastSeen = true;
  });
  const reader = res.body.getReader();
  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    bytes += value.length;
    splitter.push(value);
  }
  if (!lastSeen) throw new Error(`chunks ${ev.seq}: delivery ended without its last chunk`);
  mark('fetch_end', { seq: ev.seq, bytes, delivery, generation });
  shownPoints = delivery === 'delta' ? shownPoints + added : added;
  mark('parse_end', { seq: ev.seq, points: shownPoints, bytesPerPoint: BYTES_PER_POINT[`b1${upload}`] });
  if (into !== shown) {
    // A full delivery without a first-pass flag: swap at its end.
    disposeGroup(shown);
    shown = into;
    scene.add(into);
  }
  have = [generation, ev.points];
  dirty = true;
  mark('scene_swap', { seq: ev.seq });
  pendingPresent.push(ev.seq);
}

// B2: PLY parsed off the main thread.
const worker = mode === 'b2' ? new Worker('/static/baseline/three/ply-worker.js', { type: 'module' }) : null;
let workerJob = 0;
const workerWaiting = new Map();
if (worker) {
  worker.onmessage = (m) => {
    const w = workerWaiting.get(m.data.id);
    workerWaiting.delete(m.data.id);
    if (m.data.error) w.reject(new Error(m.data.error));
    else w.resolve(m.data);
  };
}
function parseInWorker(buffer, skip) {
  const id = ++workerJob;
  return new Promise((resolve, reject) => {
    workerWaiting.set(id, { resolve, reject });
    worker.postMessage({ id, buffer, skip }, [buffer]); // transferred, not copied
  });
}

async function loadSnapshotB2(ev) {
  mark('fetch_start', { seq: ev.seq });
  const res = await fetch(`${ev.url}?have=${have ? `${have[0]}.${have[1]}` : ''}`, { cache: 'no-store' });
  if (!res.ok) throw new Error(`data ${ev.seq}: ${res.status}`);
  const delivery = res.headers.get('X-PB-Delivery');
  const generation = Number(res.headers.get('X-PB-Generation'));
  const skip = Number(res.headers.get('X-PB-Skip'));
  const buf = await res.arrayBuffer();
  mark('fetch_end', { seq: ev.seq, bytes: buf.byteLength, delivery, generation });
  const r = await parseInWorker(buf, skip);
  const obj = piece(new THREE.BufferAttribute(r.position, 3), new THREE.BufferAttribute(r.color, 4, true), r.count, sphereFromBox(r.min, r.max));
  shownPoints = delivery === 'delta' ? shownPoints + r.count : r.count;
  mark('parse_end', { seq: ev.seq, points: shownPoints, bytesPerPoint: BYTES_PER_POINT.b2, worker_ms: r.ms });
  if (delivery === 'delta') {
    shown.add(obj);
  } else {
    if (shown) disposeGroup(shown);
    shown = new THREE.Group();
    shown.add(obj);
    scene.add(shown);
  }
  have = [generation, ev.points];
  dirty = true;
  mark('scene_swap', { seq: ev.seq });
  pendingPresent.push(ev.seq);
}

const loadSnapshot = { full: loadSnapshotFull, b1: loadSnapshotB1, b2: loadSnapshotB2 }[mode];

// Waits until the GPU finished the submitted work (decision 0020's sync). WebGL: a 1-pixel
// readPixels, synchronous, returns nothing (B0 keeps its exact timing). WebGPU: a promise.
const syncPixel = new Uint8Array(4);
function gpuDone() {
  if (webgpu) return renderer.backend.device.queue.onSubmittedWorkDone();
  const gl = renderer.getContext();
  gl.readPixels(0, 0, 1, 1, gl.RGBA, gl.UNSIGNED_BYTE, syncPixel);
  return null;
}

const pendingPresent = [];
const pendingFirst = []; // B1: snapshots whose coarse first pass is in the scene (decision 0051)
let stopLoop = false;
async function frame(t) {
  if (stopLoop) return;
  pb.frames.push(t);
  // WebGPU (B1-webgpu): ask for the next frame before waiting for the GPU below, so a slow
  // completion signal cannot make the loop skip a display cycle (P4.18: the page ran at 30 Hz).
  // WebGL2 syncs in place and keeps its order.
  if (webgpu) requestAnimationFrame(frame);
  // B0 draws every frame (decision 0005); B1/B2 only when something changed.
  if (mode === 'full' || dirty || pendingPresent.length || pendingFirst.length) {
    dirty = false;
    renderer.render(scene, camera);
    if (pendingFirst.length && !pendingPresent.length) {
      // First reflection: the first frame showing the new generation, synchronised the same way.
      const seqs = pendingFirst.splice(0);
      const wait = gpuDone();
      if (wait) await wait;
      for (const seq of seqs) mark('first_presented', { seq });
    }
    if (pendingPresent.length) {
      const firsts = pendingFirst.splice(0);
      // The first frame that contains a new snapshot is synchronised once (decision 0020): 'submitted'
      // is when render() returned, 'presented' is when the GPU finished drawing it. One sync per
      // snapshot (14 per flight) — a measurement artefact, applied identically to every implementation.
      const seqs = pendingPresent.splice(0);
      for (const seq of seqs) mark('submitted', { seq });
      const wait = gpuDone();
      if (wait) await wait;
      for (const seq of firsts) mark('first_presented', { seq });
      for (const seq of seqs) mark('presented', { seq, points: shownPoints || undefined });
    }
  }
  if (!webgpu) requestAnimationFrame(frame);
}

async function main() {
  viewpoints = (await (await fetch('/static/bench/viewpoints/flight-01.json')).json()).viewpoints;
  setView(params.get('view') ?? 'overview_sw');
  requestAnimationFrame(frame);
  const manifest = (await (await fetch('/manifest.json')).json()).events;
  mark('start', { scenario, mode, renderer: webgpu ? (renderer.backend.isWebGLBackend ? 'webgpu-renderer on webgl2 (forceWebGL)' : 'webgpu') : 'webgl', upload: mode === 'b1' ? upload : undefined });
  pb.ready = true;

  if (scenario === 'memtest') {
    // Memory metric self-check (decision 0020): hold a 200 MB Float32Array; mem_cpu must rise by
    // about that much while JSHeapUsedSize does not.
    await new Promise((r) => setTimeout(r, 3000)); // let the OS sampler (PowerShell) start
    mark('alloc_start');
    window.__hold = new Float32Array(50 * 1024 * 1024).fill(1);
    mark('alloc_end', { bytes: window.__hold.byteLength });
    await new Promise((r) => setTimeout(r, 2000));
    pb.done = true;
    return;
  }

  if (scenario === 'memspike') {
    // Short-peak self-check (decision 0032): 200 MB filled, then freed ~30 ms later. Transferring
    // the buffer to length 0 releases its backing store right away, without waiting for GC.
    await new Promise((r) => setTimeout(r, 3000)); // let the OS sampler (PowerShell) start
    mark('alloc_start');
    let buf = new ArrayBuffer(200 * 1024 * 1024);
    new Uint8Array(buf).fill(1);
    mark('alloc_end', { bytes: buf.byteLength });
    await new Promise((r) => setTimeout(r, 30));
    buf = buf.transfer(0);
    mark('freed');
    await new Promise((r) => setTimeout(r, 2000));
    pb.done = true;
    return;
  }

  if (scenario === 'replay') {
    // Snapshots are processed strictly in order; a slow parse delays the next one, as in the app.
    let chain = Promise.resolve();
    const es = new EventSource(`/events?speed=${speed}`);
    es.addEventListener('snapshot', (m) => {
      const ev = JSON.parse(m.data);
      mark('snapshot_received', { seq: ev.seq, kind: ev.kind });
      chain = chain.then(() => loadSnapshot(ev));
      chain.catch((e) => mark('error', { message: String(e) }));
    });
    es.addEventListener('end', () => {
      es.close();
      chain.then(() => requestAnimationFrame(() => requestAnimationFrame(() => (pb.done = true))));
    });
  } else {
    const last = manifest[manifest.length - 1];
    mark('snapshot_received', { seq: last.seq, kind: last.kind });
    await loadSnapshot(last);
    if (scenario === 'orbit') {
      // Synchronised frames (decision 0020): each frame is followed by a sync that waits for the
      // GPU to finish. rAF intervals do not include GPU work, so they cannot measure render cost.
      // cpu = render() call alone (command recording + submit); total = cpu + GPU execution + sync;
      // gpu_estimate = total − cpu.
      // Let the normal loop present the swapped snapshot first (its presented mark must not wait for
      // the sync loop — PR #6 review), then stop it for the synchronised frames.
      await waitFrames(3);
      stopLoop = true;
      await waitFrames(1);
      // WebGPU (B1-webgpu): the same sync as PointBlitz web (decision 0049) — frames go to an
      // offscreen target and its first pixel is copied and mapped (readRenderTargetPixelsAsync).
      // WebGL2: the canvas readPixels above. GPU timestamps per batch: three.js trackTimestamp on
      // WebGPU, EXT_disjoint_timer_query_webgl2 around the batch on WebGL2.
      const target = webgpu ? new THREE.RenderTarget(WIDTH, HEIGHT, { depthBuffer: true }) : null;
      const glt = webgpu ? null : glTimer(renderer.getContext());
      pb.gpuTimer = webgpu ? (renderer.backend.isWebGLBackend ? 'three.js trackTimestamp on WebGL2 (EXT_disjoint_timer_query_webgl2), render passes of the batch' : 'three.js trackTimestamp (WebGPU timestamp-query), render passes of the batch') : glt ? 'EXT_disjoint_timer_query_webgl2 TIME_ELAPSED around the batch' : 'none: EXT_disjoint_timer_query_webgl2 not exposed';
      if (target) renderer.setRenderTarget(target);
      if (webgpu) await renderer.resolveTimestampsAsync('render'); // drop what earlier frames recorded
      mark('sync_start');
      for (const v of viewpoints) {
        setView(v.name);
        // batch=N (P4.1 diagnosis): N frames back to back, one sync, per-frame averages — the
        // sync's own cost is spread over N frames, so implementations compare without it.
        for (let k = 0; k < framesPerView; k += batch) {
          const frame = { view: v.name, batch };
          const query = glt?.begin();
          const t0 = performance.now();
          for (let j = 0; j < batch; j++) renderer.render(scene, camera);
          const t1 = performance.now();
          if (query) glt.end(query, batch, frame);
          if (target) {
            await renderer.readRenderTargetPixelsAsync(target, 0, 0, 1, 1);
          } else {
            const wait = gpuDone();
            if (wait) await wait;
          }
          const t2 = performance.now();
          frame.cpu = (t1 - t0) / batch;
          frame.ms = (t2 - t0) / batch;
          if (webgpu) frame.gpu = (await renderer.resolveTimestampsAsync('render')) / batch; // after t2
          pb.syncFrames.push(frame);
        }
      }
      mark('sync_end');
      if (target) renderer.setRenderTarget(null);
      if (glt && !(await glt.finish())) pb.gpuTimer += ' (some results never arrived)';
      stopLoop = false;
      dirty = true;
      requestAnimationFrame(frame);
    }
    await waitFrames(2);
    pb.done = true;
  }
}

function waitFrames(n) {
  return new Promise((resolve) => {
    const step = () => (n-- <= 0 ? resolve() : requestAnimationFrame(step));
    requestAnimationFrame(step);
  });
}

main().catch((e) => {
  mark('error', { message: String(e) });
  console.error(e);
});
