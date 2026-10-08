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

> **Status: first results, one machine.** Same-session comparison against the three.js baseline on one PC
> (Windows 11, RTX 4070, i7-13700F, 60 Hz display, loopback network), measured 2026-10-08 under the
> validity rules C1–C4. Full table, conditions and footnotes: [docs/bench/comparison.md](docs/bench/comparison.md).
> Other hardware is estimated with a cost model, not measured ([docs/bench/cost-model.md](docs/bench/cost-model.md)).

## Results (reference dataset flight-01, 2.5 M points at the end, median of 3–5 runs)

| | three.js baseline | native | browser WebGPU | browser WebGL2 | server video |
|---|---:|---:|---:|---:|---:|
| new preview on screen (×60 replay, p50) | 471 ms | **21 ms** | **27 ms** | **36 ms** | **23 ms** |
| new refined snapshot on screen (×60, p50) | 632 ms | **131 ms** | **121 ms** | **123 ms** | **118 ms** |
| cold start, last snapshot on screen | 904 ms | **186 ms** | **133 ms** | **131 ms** | **156 ms** |
| main-thread blocks > 50 ms (×60 replay) | 14 | — | **0** | **0** | **0** |
| bytes received (×60 replay) | 530.8 MB | 180.4 MB | 180.4 MB | 180.4 MB | 10.3 MB |
| frame time p50 / p95 / p99 (orbit, synced every frame) | 1.70 / 1.90 / 2.0 ms | 1.55 / 1.81 / 2.00 ms | 3.1 / 4.3 / 5.0 ms | 3.5 / 6.0 / 13.5 ms | — |

What these numbers do and do not say:

- **New data reaches the screen about 5–22× sooner** (refined and cold about 5–7×, preview 13–22×), because PointBlitz streams GPU-ready chunks, sends only new points for preview snapshots, and never parses on the main thread.
- **Drawing speed is on par, not faster.** Native frame time is within the target (the baseline's own values) with no margin. **Browser frame time misses the target**: the judged value synchronises with the GPU after every frame, and that round trip dominates; in an earlier diagnostic session that synchronised every 30 frames, WebGPU drew a frame in 1.43 ms, like native (1.37 ms) — see the footnotes.
- **WebGL2** also blocks the main thread once during a cold start (target: never).
- **Server video** meets every target, but 2 of 5 rotation runs dropped more than 1 % of display cycles (worst 6.32 %, median 0.40 %): the server ticks at 60 Hz and the display ran at about 59.94 Hz, so the phase drifts through the refresh boundary. No frame is lost; one cycle shows two.
- Memory and bandwidth are recorded, not targets.

## Why

The first user is [SkyLens](https://github.com/NET-Challenge-S13/skylens), a drone mapping system whose
reconstruction ([skyrecon](https://github.com/AH100-1/skyrecon)) streams a growing point cloud during flight —
from 150 k to 2.5 M points (67.5 MB) over 14 snapshots, 530 MB in total.
Preview snapshots only add points; refined snapshots recompute the whole cloud.
PointBlitz appends what is new, swaps whole generations without a blank frame, and never parses on the main thread.

## Documents

| | |
|---|---|
| [INTENT.md](INTENT.md) | why this exists |
| [SPEC.md](SPEC.md) | what it must do: inputs, architecture, metrics, comparison table |
| [PROJECT.md](PROJECT.md) | how it is built and reviewed |
| [docs/decisions/](docs/decisions/) | every technical choice and the reason for it |
| [docs/data/flight-01.md](docs/data/flight-01.md) | analysis of the reference dataset |

## Roadmap

| Phase | |
|---|---|
| P0 | measure the current three.js approach on the reference dataset |
| P1 | core + native renderer |
| P2 | browser (wasm, WebGPU / WebGL2) |
| P3 | server-side rendering with hardware video encoding |
| P4 | comparison table across all four (first results above), with a hardware cost model (second machine pending) |
| P5 | meshes (terrain, buildings, textured surfaces), Python bindings |

## 한국어

PointBlitz 는 Rust + wgpu 로 만드는 점군 렌더러입니다. 같은 렌더 코어를 브라우저(wasm), 네이티브, 서버(영상 전송)
세 곳에서 돌리고, 기존 three.js 방식과 같은 데이터·같은 시점으로 성능을 비교합니다.
문서는 [INTENT](INTENT.md) → [SPEC](SPEC.md) → [PROJECT](PROJECT.md) → [결정 기록](docs/decisions/) 순서로 보세요.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT) at your option.
