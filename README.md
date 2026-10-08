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

- **New data reaches the screen about 5–23× sooner** (refined and cold about 5–7×, preview 13–23×), because PointBlitz streams GPU-ready chunks, sends only new points for preview snapshots, and never parses on the main thread.
- **Drawing speed is on par, not faster.** Native frame time is within the target (the baseline's own values) with no margin. **Browser frame time misses the target**: the judged value synchronises with the GPU after every frame, and that round trip dominates. Synchronising only every 30 frames (a diagnostic run under the same rules, not the judged value), WebGPU draws a frame in 1.43 ms and native in 1.37 ms.
- **WebGL2** also blocks the main thread once during a cold start (target: never); its cost is on the CPU side, in submitting GL work.
- **Server video** meets every target, but 2 of 5 rotation runs dropped more than 1 % of display cycles (worst 6.32 %, median 0.40 %): the server ticks at exactly 60 Hz and the display runs slightly below 60 Hz, so the phase drifts through the refresh boundary. No frame is lost; one cycle shows two.
- Memory and bandwidth are recorded, not targets.

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
| server video | `target/release/pointblitz-server serve --replay http://127.0.0.1:8700 --port 8720 --wait-for-client`, then open `http://127.0.0.1:8700/static/web/video.html` | Windows or Linux with an NVIDIA GPU (NVENC, from the driver) and a browser with WebCodecs |

Everywhere: drag to orbit, wheel to zoom, keys 1–8 for the fixed viewpoints. `speed` is the replay speed factor
(60 = the 34-minute flight in about 34 s).

Server video options and limits (decision 0042):

- Frames are sent the moment they are encoded; on a 60 Hz display 2 of 5 rotation runs still showed an occasional
  doubled frame, because the server's 60 Hz clock and the display drift apart.
- `serve --phase-lock on` moves the server's tick to the browser's display so that drift goes away. It follows
  **one** client (the first that reports), and it cannot match a display that is not 60 Hz. It is off by default:
  it did not meet its acceptance bound (see [docs/bench/server-video.md](docs/bench/server-video.md) §2.5).
- `?pacing=adaptive` (or `buffer`) on the video page holds one frame back and evens out frame timing, at the cost
  of up to one frame of latency (decisions 0039, 0040). It is meant for displays that are not 60 Hz, but it has
  **not been measured on such displays**.

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

**첫 결과(2026-10-08, 한 PC 같은 세션, 측정 유효성 규칙 C1–C4)** — 자세한 표·조건·각주는 [docs/bench/comparison.md](docs/bench/comparison.md), 무엇이 왜 좋아졌는지는 [성능 보고서](docs/bench/report.md).

- 새 데이터가 화면에 나오기까지: three.js 471 ms(preview) / 632 ms(refined) / 904 ms(cold) → PointBlitz 21–36 / 118–131 / 131–186 ms(**약 5–23 배 빠름**). 받는 데이터도 530.8 MB → 180.4 MB(브라우저·native), 10.3 MB(서버 영상).
- **그리기 속도는 같은 수준이고 더 빠르지 않습니다.** native 의 frame_time 은 목표(기준 방식 값) 안이지만 여유가 없고, **브라우저는 목표 미달**입니다 — 매 프레임 GPU 와 동기화하는 비용 때문이며, 30 프레임마다 동기화하면(진단 값) WebGPU 1.43 ms, native 1.37 ms 입니다.
- WebGL2 는 cold 에서 메인 스레드를 한 번 막습니다(목표: 0 회).
- 서버 영상은 모든 목표를 지키지만, 회전 5 회 중 2 회는 화면 주기의 1 % 넘게 빈 주기가 생겼습니다(최대 6.32 %, 중앙값 0.40 %). 서버는 정확히 60 Hz, 화면은 60 Hz 보다 조금 느려 위상이 화면 갱신 경계를 지나가기 때문이며, 프레임이 사라지지는 않습니다.
- 다른 하드웨어 값은 실측이 아니라 비용 모델로 추정합니다(두 번째 장비 보정 대기).

직접 돌려 보려면 위 [Try it](#try-it) 를 보세요 — 재생 서버를 띄운 뒤 브라우저·native·서버 영상 중 하나로 봅니다(드래그 회전, 휠 확대, 1–8 고정 시점). 서버 영상은 Windows 또는 Linux + NVIDIA GPU(드라이버의 NVENC)가 필요하고, `--phase-lock on` 은 클라이언트 하나·60 Hz 화면에서만 맞으며(기본 끔), 영상 페이지의 `?pacing=adaptive` 는 한 프레임을 쌓아 두어 프레임 간격을 고르게 합니다(지연 최대 한 프레임) — 60 Hz 가 아닌 화면을 위한 것이지만 그런 화면에서는 재 보지 않았습니다.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT) at your option.
