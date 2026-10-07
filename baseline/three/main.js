// Reference implementation of the current three.js approach (decision 0005, 0019).
// Benchmark only — deliberately NOT optimised: every snapshot is downloaded whole, parsed on the
// main thread with PLYLoader, and the previous THREE.Points is thrown away.
//
// URL parameters:
//   scenario = replay | cold | orbit   (SPEC §6.1)
//   speed    = replay speed factor (default 60)
//   view     = viewpoint name to hold (default overview_sw)
//   frames   = frames per viewpoint in orbit (default 120)
//
// Measurement hooks live on window.__pb (read by the harness):
//   marks   [{name, t, ...}]  t = performance.now() in ms
//   frames  [t, ...]          requestAnimationFrame timestamps
//   setView(name), ready, done

import * as THREE from 'three';
import { PLYLoader } from 'three/addons/loaders/PLYLoader.js';

const WIDTH = 1920;
const HEIGHT = 1080;
const POINT_SIZE_PX = 2;

const params = new URLSearchParams(location.search);
const scenario = params.get('scenario') ?? 'replay';
const speed = Number(params.get('speed') ?? 60);
const framesPerView = Number(params.get('frames') ?? 120);

const pb = (window.__pb = { marks: [], frames: [], longtasks: [], syncFrames: [], ready: false, done: false, setView });
// Main-thread blocks over 50 ms (SPEC §6.2 main_thread_block).
new PerformanceObserver((list) => {
  for (const e of list.getEntries()) pb.longtasks.push({ t: e.startTime, ms: e.duration });
}).observe({ type: "longtask", buffered: true });
const mark = (name, extra = {}) => pb.marks.push({ name, t: performance.now(), ...extra });

const renderer = new THREE.WebGLRenderer({ antialias: false, preserveDrawingBuffer: true });
renderer.setPixelRatio(1);
renderer.setSize(WIDTH, HEIGHT);
document.body.appendChild(renderer.domElement);

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
  return new THREE.CanvasTexture(c);
}
const material = new THREE.PointsMaterial({
  size: POINT_SIZE_PX,
  sizeAttenuation: false,
  vertexColors: true,
  map: diskTexture(),
  alphaTest: 0.5,
});

let points = null;
let viewpoints = [];

function setView(name) {
  const v = viewpoints.find((x) => x.name === name);
  if (!v) throw new Error(`unknown viewpoint ${name}`);
  camera.position.set(...v.eye);
  camera.up.set(...v.up);
  camera.lookAt(...v.target);
  camera.updateMatrixWorld();
  mark('view', { view: name });
}

const loader = new PLYLoader();

// One snapshot, the way the current app would do it: whole file, main-thread parse, full replace.
async function loadSnapshot(ev) {
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

const pendingPresent = [];
const syncPixel = new Uint8Array(4);
let stopLoop = false;
function frame(t) {
  if (stopLoop) return;
  pb.frames.push(t);
  renderer.render(scene, camera);
  if (pendingPresent.length) {
    // The first frame that contains a new snapshot is synchronised once (decision 0020): 'submitted'
    // is when render() returned, 'presented' is when the GPU finished drawing it. One sync per
    // snapshot (14 per flight) — a measurement artefact, applied identically to every implementation.
    for (const seq of pendingPresent) mark('submitted', { seq });
    renderer.getContext().readPixels(0, 0, 1, 1, WebGL2RenderingContext.RGBA, WebGL2RenderingContext.UNSIGNED_BYTE, syncPixel);
    while (pendingPresent.length) mark('presented', { seq: pendingPresent.shift() });
  }
  requestAnimationFrame(frame);
}

async function main() {
  viewpoints = (await (await fetch('/static/bench/viewpoints/flight-01.json')).json()).viewpoints;
  setView(params.get('view') ?? 'overview_sw');
  requestAnimationFrame(frame);
  const manifest = (await (await fetch('/manifest.json')).json()).events;
  mark('start', { scenario });
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

  if (scenario === 'replay') {
    // Snapshots are processed strictly in order; a slow parse delays the next one, as in the app.
    let chain = Promise.resolve();
    const es = new EventSource(`/events?speed=${speed}`);
    es.addEventListener('snapshot', (m) => {
      const ev = JSON.parse(m.data);
      mark('snapshot_received', { seq: ev.seq, kind: ev.kind });
      chain = chain.then(() => loadSnapshot(ev));
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
      // Synchronised frames (decision 0020): each frame is followed by a 1-pixel readPixels, which
      // waits for the GPU to finish. rAF intervals do not include GPU work, so they cannot measure
      // render cost. cpu = render() call alone (command recording + submit); total = cpu + GPU
      // execution + sync; gpu_estimate = total − cpu.
      stopLoop = true;
      await waitFrames(2);
      const gl = renderer.getContext();
      mark('sync_start');
      for (const v of viewpoints) {
        setView(v.name);
        for (let k = 0; k < framesPerView; k++) {
          const t0 = performance.now();
          renderer.render(scene, camera);
          const t1 = performance.now();
          gl.readPixels(0, 0, 1, 1, gl.RGBA, gl.UNSIGNED_BYTE, syncPixel);
          const t2 = performance.now();
          pb.syncFrames.push({ view: v.name, cpu: t1 - t0, ms: t2 - t0 });
        }
      }
      mark('sync_end');
      stopLoop = false;
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
