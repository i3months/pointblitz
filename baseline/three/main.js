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

const pb = (window.__pb = { marks: [], frames: [], ready: false, done: false, setView });
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
  mark('parse_end', { seq: ev.seq, points: geometry.getAttribute('position').count });
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
function frame(t) {
  pb.frames.push(t);
  renderer.render(scene, camera);
  while (pendingPresent.length) mark('presented', { seq: pendingPresent.shift() });
  requestAnimationFrame(frame);
}

async function main() {
  viewpoints = (await (await fetch('/static/bench/viewpoints/flight-01.json')).json()).viewpoints;
  setView(params.get('view') ?? 'overview_sw');
  requestAnimationFrame(frame);
  const manifest = (await (await fetch('/manifest.json')).json()).events;
  mark('start', { scenario });
  pb.ready = true;

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
      for (const v of viewpoints) {
        setView(v.name);
        await waitFrames(framesPerView);
      }
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
