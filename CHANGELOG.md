# Changelog

All six crates (`pointblitz`, `pointblitz-core`, `pointblitz-io`, `pointblitz-native`, `pointblitz-server`,
`pointblitz-web`) share one version.

## Unreleased

Additions only: 0.2.1 code builds unchanged, and the chunk format (v2) and the snapshot path are as before.
Slot diff streams (decision 0052): cumulus3d 0.4.0's `--stable-display` live stream (format v4), where every point
owns a fixed slot id and each event carries only the points added, moved and removed.

- `pointblitz-io`: `slots` — the slot diff message (32-byte header `PBSD` v1 + the v4 records unchanged), encode,
  decode and validation (lengths, every id below the capacity), `Diff` (the three sections as records), `Fold`
  (all messages so far as one, for a client that joins late). `slots::dir` (new cargo feature `stream`) reads a
  v4 `live/stream` directory in `seq` order, with the per-event capacity. `client`: `spawn_slots`,
  `spawn_slots_cold`, `is_slot_stream`.
- `pointblitz-core`: `SlotScene` — one GPU buffer, 16 bytes per slot (f32 position relative to the stream origin,
  RGB, flags); `apply` puts a whole message on screen from the next frame (records into a CPU mirror, then one
  `write_buffer` of the changed range — every backend, WebGL2 included), starts at seq 1 (or at a folded state,
  `apply_state`, or an explicit `expect`), holds a message whose predecessor is missing, drops old ones, doubles
  the buffer with a GPU copy when the capacity grows. Clients retry an event they cannot fetch and then fall
  back to the folded state through it. Drawn by the existing point
  pipeline with the same size and colour rules, hidden slots dropped in the vertex shader:
  `Renderer::prepare_with_slots` / `draw_slots` / `render_with_slots`, `Headless::capture_with_slots` (the 0.2.1
  calls are unchanged).
- Replay server: `pointblitz-bench replay --stream <dir>` announces the events at their original times
  (`event: slots`), serves `/slots/<seq>` and `/slots/state?seq=N` (folded); `pointblitz-bench viewpoints` takes a
  stream directory. `bench/viewpoints/theater30-stable.json`.
- Native viewer, browser (`apply_slots`, `slot_bounds`, …) and server video follow such a server and apply each
  event whole; the server video reads the stream directory itself with `--data` (decision 0050).
- `pointblitz`: re-exports `SlotScene`, `Applied`, `ApplyError`, `UploadMethod`; new feature `stream`.
- Upload method (b) (decision 0052 §2): `SlotScene::with_upload(UploadMethod::Scatter)` writes each message's
  pack as it is to a storage buffer and a compute shader scatters the records into the slots — the same slot
  bytes as the mirror, no CPU mirror. WebGPU and native; on WebGL2 the scene keeps `Mirror`, the default.
  `SlotScene::read_slots` (not wasm32) reads the slot buffer back. Selectable for the A/B: native
  `--upload mirror|scatter`, server video `--upload`, browser `?upload=scatter` (`Viewer::set_slot_upload`), the
  bench scripts pass it through, and `pointblitz-bench slot-apply --stream <dir>` times apply → GPU done per event
  for both. `PB_NO_GPU=1` makes `headless::device()` return none, so the GPU tests skip.
- Not measured yet (decision 0052 §5).

## 0.2.1 — 2026-10-16

Additions only; 0.2.0 code builds unchanged and the chunk format is still v2.

- `pointblitz-core`: layers for drawing points, meshes and lines in one pass with one depth buffer, upstreamed from
  SkyLens (which carried them as a vendored patch of 0.2.0): `begin_pass` (clear colour, depth kept), `FrameStyle`,
  `Fog` (three.js-style linear fog on every shader), `MeshRenderer` (Lambert or unlit, textures with mipmaps,
  generated normals, opaque/alpha/additive blending), `LineRenderer` (1 px lines, per-vertex alpha).
- `Renderer::set_point_size` (`PointSize::Pixels` or `PointSize::Metres` with distance attenuation),
  `Renderer::prepare` / `Renderer::draw` (several scenes per frame into a pass the caller opened, optional
  per-scene alpha); `render` uses the same path.
- `Headless::capture_with`: capture a frame the caller records.
- Written for WebGL2 limits (uniforms read in the vertex stage only, no storage buffers, dynamic-offset uniform
  slots); not yet run in a browser, since the web crate does not use the layers.

## 0.2.0 — 2026-10-09

**Breaking: chunk format v2.** Clients and servers of 0.1.0 cannot read what 0.2.0 sends, and the other way round
(decision 0051).

- Chunk format v2: positions are 16-bit, quantised to each chunk's bounding box (12 bytes per point instead of 16);
  the GPU undoes the quantisation, so clients still copy chunks without parsing. The header carries the point stride.
- Refined snapshots are delivered coarse first: every 8th point, then the rest, pass by pass. A new flag marks the
  chunk that completes the first pass; clients show the new generation there and fill in the rest
  (`event_first_latency`, "first reflection").
- Replay server: converts a snapshot when it is announced and serves `/chunks` from memory (the two most recent
  snapshots); large snapshots are read in parallel (decision 0050).
- `pointblitz-server serve --data <dir>`: the video server reads the snapshots on its own machine instead of
  fetching chunks over HTTP (decision 0050).
- `pointblitz-io`: new `convert` module (delivery plan, background conversion, coarse-first pieces) shared by the
  replay server and the video server; `/manifest.json` has an `appends` field.
- `pointblitz-core`: `GpuTimer` and `Renderer::set_timestamp_writes` for GPU timestamps (decision 0049).
- Measured on one PC: a new refined snapshot is first on screen in about 35–43 ms in the browser (WebGPU) and about
  49 ms as server video, about 3× sooner than before the latency work (builds 0404a79 … ff05817, the same
  delivery path as 0.1.0) ([README](README.md), `docs/bench/`).

## 0.1.0 — 2026-10-08

First release on crates.io: render core, PLY / chunk format, native viewer, server video (NVENC), browser module
source (decision 0044).
