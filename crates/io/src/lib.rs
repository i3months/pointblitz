//! PLY stream parser and GPU-ready chunk format for PointBlitz.
//!
//! The input is a byte slice, never a file path, so the same code runs natively and in wasm
//! (decision 0014). Properties are matched by name, so both skyrecon layouts
//! (`XyzRgbNormal`, `XyzNormalRgb`) and the 15-byte layout without normals are read.
//!
//! - [`ply`]: whole-file reading.
//! - [`stream`]: incremental reading as bytes arrive.
//! - [`chunk`]: the 16 B/point chunk format clients copy straight to the GPU (decisions 0008, 0022).
//! - `client` (not wasm): the replay-server client used by the native viewer and the video server.

pub mod chunk;
#[cfg(not(target_arch = "wasm32"))]
pub mod client;
pub mod ply;
pub mod stream;

pub use ply::{Header, PlyError, Point, ScalarType, is_prefix, parse_header, points};
pub use stream::PlyStream;
