// pointblitz-web (P4.7): the default module — WebGPU, with a WebGL2 fallback (decision 0028).
// `init()` loads the wasm once; `Viewer.create(canvas, backend, memoryHints)` opens the renderer.
export { default as init, Viewer } from './pkg/pointblitz_web.js';
export { ChunkSplitter } from './chunks.js';
