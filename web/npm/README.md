# pointblitz-web

Browser renderer of [PointBlitz](https://github.com/i3months/pointblitz): point clouds drawn with WebGPU
(WebGL2 fallback) from one Rust/wgpu core compiled to wasm.

> Not published yet. This directory is the package structure (P4.7); publishing is a separate
> decision (P4.8). Build it with `bash web/npm/pack.sh` from the repository root.

```js
import { init, Viewer, ChunkSplitter } from 'pointblitz-web';      // WebGPU + WebGL2 fallback
// import { init, Viewer, ChunkSplitter } from 'pointblitz-web/webgpu'; // WebGPU only, smaller

await init();
const viewer = await Viewer.create(canvas, 'auto', 'performance'); // 'auto' | 'webgpu' | 'webgl'
viewer.set_camera(eye, target, up, fovYDeg);

// Point chunks (PointBlitz chunk format v1, 80 B header + 16 B/point) as they arrive:
const split = new ChunkSplitter((chunk) => { viewer.insert(chunk); viewer.render(); });
const res = await fetch(url);
for await (const part of res.body) split.push(part);
```

Two modules: `pointblitz-web` (default, WebGPU with WebGL2 fallback) and `pointblitz-web/webgpu`
(WebGPU only, about a tenth of the size). Sizes are reported by CI.

Licensed under either of Apache License, Version 2.0 or MIT license at your option.
