//! NVENC bindings (vendored, MIT — see ../NOTICE.md). Generated code: lints are off here.
#![allow(warnings, clippy::all, unsafe_code, dead_code)]

mod guid;
mod version;

#[rustfmt::skip]
pub mod nvEncodeAPI;

pub use nvEncodeAPI::*;
