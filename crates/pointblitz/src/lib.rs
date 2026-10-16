//! PointBlitz: point clouds at GPU speed — one Rust/wgpu core for the browser (wasm), native
//! windows and server-side video (decision 0044: the crate people find by name).
//!
//! This crate re-exports the two library crates:
//! - [`core`] (`pointblitz-core`): scene, chunk store, GPU buffers, point pipeline, headless capture.
//! - [`io`] (`pointblitz-io`): PLY stream parser, the GPU-ready chunk format and slot diff messages
//!   (decision 0052; feature `stream` reads a cumulus3d v4 stream directory).
//!
//! The targets are their own crates: `pointblitz-native` (window viewer), `pointblitz-server`
//! (headless render + NVENC video), `pointblitz-web` (wasm). Measurements against the three.js
//! baseline: <https://github.com/i3months/pointblitz/blob/main/docs/bench/report.md>.

pub use pointblitz_core as core;
pub use pointblitz_io as io;

pub use pointblitz_core::{
    Applied, ApplyError, Camera, GpuTimer, Headless, Inserted, Renderer, Scene, SlotScene,
};
