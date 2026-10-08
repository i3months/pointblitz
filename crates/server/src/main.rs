//! PointBlitz server video (P3): headless rendering with the core, H.264 with NVENC.
//!
//! ```text
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

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        #[cfg(windows)]
        Some("encode-test") => encode_test::run(&args[1..]),
        #[cfg(not(windows))]
        Some("encode-test") => Err("NVENC encoding is implemented for Windows only (decision 0035)".into()),
        _ => Err("usage: pointblitz-server encode-test --ply <file> --viewpoints <json> --out <dir> [--qp 18,23,28]".into()),
    };
    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            std::process::ExitCode::FAILURE
        }
    }
}
