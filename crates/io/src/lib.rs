//! PLY stream parser and GPU-ready chunk format for PointBlitz.
//!
//! The input is a byte slice, never a file path, so the same code runs natively and in wasm
//! (decision 0014). Properties are matched by name, so both skyrecon layouts
//! (`XyzRgbNormal`, `XyzNormalRgb`) and the 15-byte layout without normals are read.

pub mod ply;

pub use ply::{Header, PlyError, Point, ScalarType, parse_header, points};
