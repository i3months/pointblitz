//! PointBlitz scenario replay and metrics.
//!
//! Subcommands:
//! - `viewpoints <final.ply | stream dir> <out.json>`: derives the 8 fixed viewpoints (decision 0017) and
//!   checks that each one sees the cloud.
//! - `replay --data <dir> [--web <dir>] [--port N]`: serves a snapshot dataset and announces
//!   snapshots at their original times (decision 0018). `--stream <dir>` instead of `--data`: a
//!   cumulus3d v4 slot diff stream, announced the same way (decision 0052).
//! - `capture --ply <file> --viewpoints <json> --out <dir>`: PointBlitz captures at the fixed viewpoints.
//! - `compare <dir A> <dir B> [--viewpoints <json>] [--md <file>]`: SSIM and coverage per viewpoint.

mod capture;
mod chunks;
mod image;
mod replay;
mod slots;
mod ssim;
mod viewpoints;

use std::process::ExitCode;

const USAGE: &str = "usage:
  pointblitz-bench viewpoints <final.ply | live/stream dir> <out.json>
  pointblitz-bench replay --data <dir> | --stream <live/stream dir> [--web <dir>] [--port 8700]
  pointblitz-bench capture --ply <file> --viewpoints <json> --out <dir>
  pointblitz-bench compare <dir A> <dir B> [--viewpoints <json>] [--md <file>]";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("viewpoints") if args.len() == 3 => viewpoints::run(&args[1], &args[2]),
        Some("replay") => replay::run(&args[1..]),
        Some("capture") => capture::run_capture(&args[1..]),
        Some("compare") => capture::run_compare(&args[1..]),
        _ => Err(USAGE.to_string()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
