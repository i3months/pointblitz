// pointblitz-web/webgpu (P4.7): WebGPU only, without the GL backend (decision 0033) — about a
// tenth of the default module's size; `Viewer.create` fails where the browser has no WebGPU.
export { default as init, Viewer } from './pkg-webgpu/pointblitz_web.js';
export { ChunkSplitter } from './chunks.js';
