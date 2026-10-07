// PointBlitz in the browser (P2, decisions 0028, 0029): the wasm viewer on a WebGPU canvas.
//
// URL parameters:
//   scenario = check | still     check: report the adapter only (P2.1)
//                                still: load the last snapshot as one full delivery, then hold
//                                       (fixed-viewpoint captures, P2.2)
//   view     = viewpoint to start at (default overview_sw)
//
// Hooks on window.__pb (read by bench/web/*.mjs): marks [{name, t, ...}], info, gpu,
// setView(name) → resolves when that view is drawn and the GPU is done, ready, done.

import init, { Viewer } from './pkg/pointblitz_web.js';

const params = new URLSearchParams(location.search);
const scenario = params.get('scenario') ?? 'still';
const pb = (window.__pb = { marks: [], ready: false, done: false, setView });
const mark = (name, extra = {}) => pb.marks.push({ name, t: performance.now(), ...extra });

let viewer = null;
let viewpoints = [];
let fov = 50;
let dirty = false;
let waiters = [];

// Draw only when something changed (decision 0027); the loop itself costs nothing when idle.
function frame() {
  if (dirty && viewer) {
    dirty = false;
    viewer.render();
    const done = viewer.gpu_done();
    const w = waiters;
    waiters = [];
    for (const resolve of w) done.then(resolve);
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

/** Splits a delivery body into chunks (decision 0022: point_count at byte 16, 80 B + 16 B/point). */
export function splitChunks(buf) {
  const view = new DataView(buf);
  const out = [];
  for (let off = 0; off + 80 <= buf.byteLength; ) {
    const len = 80 + 16 * view.getUint32(off + 16, true);
    out.push(new Uint8Array(buf, off, len));
    off += len;
  }
  return out;
}

async function main() {
  mark('wasm_init_start');
  await init();
  mark('wasm_init_end');
  viewer = await Viewer.create(document.getElementById('view'));
  pb.info = JSON.parse(viewer.info());
  // WebGPU does not give wgpu the adapter name; the browser's GPUAdapter.info says which GPU it is.
  const a = await navigator.gpu.requestAdapter({ powerPreference: 'high-performance' });
  pb.gpu = a && { vendor: a.info.vendor, architecture: a.info.architecture, device: a.info.device, description: a.info.description };
  mark('viewer', { info: pb.info, gpu: pb.gpu });
  requestAnimationFrame(frame);
  if (scenario === 'check') return;

  const vp = await (await fetch('/static/bench/viewpoints/flight-01.json')).json();
  viewpoints = vp.viewpoints;
  fov = vp.camera.fov_y_deg;
  const manifest = (await (await fetch('/manifest.json')).json()).events;
  const last = manifest[manifest.length - 1];
  mark('fetch_start', { seq: last.seq });
  const body = await (await fetch(`/chunks/${last.seq}`, { cache: 'no-store' })).arrayBuffer();
  mark('fetch_end', { seq: last.seq, bytes: body.byteLength });
  for (const c of splitChunks(body)) viewer.insert(c);
  mark('uploaded', { seq: last.seq, points: viewer.points(), gpu_bytes: viewer.gpu_bytes() });
  await setView(params.get('view') ?? 'overview_sw');
  mark('presented', { seq: last.seq });
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
