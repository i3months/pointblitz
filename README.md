# PointBlitz

**Point clouds at GPU speed — one Rust core, every target.**

PointBlitz is a point-cloud renderer written in Rust on top of [wgpu](https://wgpu.rs).
The same render core runs in three places:

| Target | Backend | What the client gets |
|---|---|---|
| Browser | wasm + WebGPU (WebGL2 fallback) | GPU-ready point chunks |
| Native | Vulkan / DX12 / Metal | GPU-ready point chunks |
| Server | headless wgpu + hardware video encoder | a video stream |

It is built for people who hit the limits of three.js with large point clouds:
full-file downloads before the first frame, main-thread parsing, and re-uploading the whole cloud every time it grows.

> **Status: design phase.** Nothing is benchmarked yet. Every performance claim will come with the
> data, the viewpoints and the hardware it was measured on. See [docs/DESIGN.ko.md](docs/DESIGN.ko.md).

## Why

The first user is [SkyLens](https://github.com/NET-Challenge-S13/skylens), a drone mapping system whose
reconstruction ([skyrecon](https://github.com/AH100-1/skyrecon)) streams a growing point cloud during flight —
from 150 k points to 2.5 M points (67.5 MB) over 14 snapshots. Re-downloading and re-parsing every snapshot
costs 530 MB per flight. PointBlitz streams only what changed and keeps everything on the GPU.

## Plan

1. Baseline: reproduce the current three.js approach on the same data and measure it.
2. Core: point buffers, chunked append/replace, culling, LOD, fixed-viewpoint capture.
3. Targets: native → browser (wasm) → server (video).
4. A comparison table across all four (three.js, native, browser, server video), with a hardware cost model
   so the numbers carry over to GPUs we did not test on.
5. Later: meshes (terrain, buildings, textured surfaces) and Python bindings.

## 한국어

PointBlitz 는 Rust + wgpu 로 만드는 점군 렌더러입니다. 같은 렌더 코어를 브라우저(wasm), 네이티브, 서버(영상 전송)
세 곳에서 돌리고, 기존 three.js 방식과 같은 데이터·같은 시점으로 성능을 비교합니다. 설계는
[docs/DESIGN.ko.md](docs/DESIGN.ko.md) 를 보세요.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT) at your option.
