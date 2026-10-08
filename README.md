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

> **Status: measuring.** The native, browser (WebGPU/WebGL2) and server-video targets are built and
> being measured against the three.js baseline on one machine — working reports are in
> [docs/bench/](docs/bench/). A public comparison table is in preparation; until then, no performance
> claims are made here. Every number will come with the dataset, viewpoints, hardware and commit it
> was measured on.

## Why

The first user is [SkyLens](https://github.com/NET-Challenge-S13/skylens), a drone mapping system whose
reconstruction ([skyrecon](https://github.com/AH100-1/skyrecon)) streams a growing point cloud during flight —
from 150 k to 2.5 M points (67.5 MB) over 14 snapshots, 530 MB in total.
Preview snapshots only add points; refined snapshots recompute the whole cloud.
PointBlitz appends what is new, swaps whole generations without a blank frame, and never parses on the main thread.

## Try it

PointBlitz replays a directory of skyrecon snapshot PLYs (not included in this repository) as if a flight were
streaming them. You need Rust (see `rust-toolchain.toml`), and for the browser target
`cargo install wasm-bindgen-cli --version 0.2.129 --locked`.

```sh
cargo build --release -p pointblitz-bench -p pointblitz-native -p pointblitz-server
bash web/build.sh                # browser module (WebGPU + WebGL2 fallback); `bash web/build.sh webgpu` for WebGPU only

# 1. replay server: serves the snapshots, the chunk stream and the demo pages (127.0.0.1 only)
target/release/pointblitz-bench replay --data <ply dir> --web . --port 8700
```

| Target | Start | Notes |
|---|---|---|
| browser | open `http://127.0.0.1:8700/static/web/index.html?scenario=replay&speed=60` | `&backend=webgl` forces WebGL2, `&pkg=webgpu` loads the WebGPU-only module, `&scenario=still` shows only the last snapshot |
| native | `target/release/pointblitz-native --server http://127.0.0.1:8700 --scenario replay` | Vulkan / DX12 / Metal, whatever wgpu picks |
| server video | `target/release/pointblitz-server serve --replay http://127.0.0.1:8700 --port 8720 --wait-for-client`, then open `http://127.0.0.1:8700/static/web/video.html` | Windows with an NVIDIA GPU (NVENC) and a browser with WebCodecs |

Everywhere: drag to orbit, wheel to zoom, keys 1–8 for the fixed viewpoints. `speed` is the replay speed factor
(60 = the 34-minute flight in about 34 s).

Server video options and limits (decision 0042):

- Frames are sent the moment they are encoded; on a 60 Hz display a few runs in five still show an occasional
  doubled frame, because the server's 60 Hz clock and the display drift apart.
- `serve --phase-lock on` moves the server's tick to the browser's display so that drift goes away. It follows
  **one** client (the first that reports), and it cannot match a display that is not 60 Hz. It is off by default:
  it did not meet its acceptance bound (see [docs/bench/server-video.md](docs/bench/server-video.md) §2.5).
- On a display that is not 60 Hz (for example 144 Hz), add `?pacing=adaptive` (or `buffer`) to the video page:
  the browser holds one frame back and shows one per display cycle — smoother, at the cost of up to one cycle of
  latency (decisions 0039, 0040).

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
| P4 | comparison table across all four, with a hardware cost model |
| P5 | meshes (terrain, buildings, textured surfaces), Python bindings |

## 한국어

PointBlitz 는 Rust + wgpu 로 만드는 점군 렌더러입니다. 같은 렌더 코어를 브라우저(wasm), 네이티브, 서버(영상 전송)
세 곳에서 돌리고, 기존 three.js 방식과 같은 데이터·같은 시점으로 성능을 비교합니다.
문서는 [INTENT](INTENT.md) → [SPEC](SPEC.md) → [PROJECT](PROJECT.md) → [결정 기록](docs/decisions/) 순서로 보세요.

직접 돌려 보려면 위 [Try it](#try-it) 를 보세요 — 재생 서버를 띄운 뒤 브라우저·native·서버 영상 중 하나로 봅니다(드래그 회전, 휠 확대, 1–8 고정 시점). 서버 영상은 Windows + NVIDIA(NVENC)가 필요하고, `--phase-lock on` 은 클라이언트 하나·60 Hz 화면에서만 맞으며(기본 끔), 60 Hz 가 아닌 화면에서는 영상 페이지에 `?pacing=adaptive` 를 붙입니다.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT) at your option.
