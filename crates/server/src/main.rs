//! PointBlitz server video (P3): headless rendering with the core, H.264 with NVENC.
//!
//! ```text
//! pointblitz-server serve --replay <url> [--port 8720] …   stream video (P3.2, serve.rs)
//! pointblitz-server probe [--url ws://…]                    headless test client (probe.rs)
//! pointblitz-server encode-test --ply <file> --viewpoints <json> --out <dir> [--qp 18,23,28]
//! ```
//! `encode-test` (P3.1) renders two streams per QP and writes them with what each frame cost:
//! - `views`: 30 frames held at each fixed viewpoint (the last one of each is sampled);
//! - `orbit`: 240 frames circling the overview target, 1.5° per frame (every 30th sampled).
//!
//! Output per QP: `<stream>.h264` (Annex B), `<stream>.json` (per-frame bytes, IDR, upload and
//! encode ms, sample name), and the pre-encode frames of the samples as `<stream>-src/<name>.png`.

#[cfg(windows)]
mod nvenc;

#[cfg(windows)]
mod encode_test;

mod probe;
#[cfg(windows)]
mod serve;

const USAGE: &str = "usage:
  pointblitz-server encode-test --ply <file> --viewpoints <json> --out <dir> [--qp 18,23,28]
  pointblitz-server serve --replay <url> [--port 8720] [--speed 60] [--qp 18] [--fps 60] [--exit-after-end <s>] [--wait-for-client]
  pointblitz-server probe [--url ws://127.0.0.1:8720] [--expect-snapshots 14] [--max-seconds 120]";

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result: Result<(), String> = match args.first().map(String::as_str) {
        #[cfg(windows)]
        Some("encode-test") => encode_test::run(&args[1..]),
        #[cfg(not(windows))]
        Some("encode-test" | "serve") => {
            Err("NVENC encoding is implemented for Windows only (decision 0035)".into())
        }
        #[cfg(windows)]
        Some("serve") => serve::run(&args[1..]),
        Some("probe") => probe::run(&args[1..]),
        _ => Err(USAGE.into()),
    };
    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            std::process::ExitCode::FAILURE
        }
    }
}
