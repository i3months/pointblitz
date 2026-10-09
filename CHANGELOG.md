# Changelog

All six crates (`pointblitz`, `pointblitz-core`, `pointblitz-io`, `pointblitz-native`, `pointblitz-server`,
`pointblitz-web`) share one version.

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
  49 ms as server video, about 3× sooner than 0.1.0's build ([README](README.md), `docs/bench/`).

## 0.1.0 — 2026-10-08

First release on crates.io: render core, PLY / chunk format, native viewer, server video (NVENC), browser module
source (decision 0044).
