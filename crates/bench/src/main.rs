//! PointBlitz scenario replay and metrics.
//!
//! Subcommands:
//! - `viewpoints <final.ply> <out.json>`: derives the 8 fixed viewpoints (decision 0017) and
//!   checks that each one sees the cloud.

mod viewpoints;

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("viewpoints") if args.len() == 3 => viewpoints::run(&args[1], &args[2]),
        _ => Err("usage: pointblitz-bench viewpoints <final.ply> <out.json>".to_string()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
