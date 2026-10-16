//! `slot-apply --stream <dir>` (decision 0052 §2): applies every event of a v4 slot diff stream with
//! upload method (a) mirror and (b) scatter on this machine's GPU and reports, per event, the time
//! from `apply` until the GPU finished it.
//!
//! ```text
//! pointblitz-bench slot-apply --stream <live/stream dir> [--runs 5] [--upload mirror,scatter]
//!                             [--size 1920x1080] [--out samples.jsonl]
//! ```
//!
//! Each run starts both scenes empty and applies the events in seq order; the methods alternate
//! which goes first from run to run, and one untimed run per method comes first (pipelines,
//! allocations). Per event, wall clock from just before `apply`:
//! - `apply`: until `apply` returns (CPU: validation, the mirror or the shown bits, the writes and
//!   submits);
//! - `gpu`: until the GPU finished everything submitted, after an empty submit that flushes the
//!   queued writes (the mirror's `write_buffer` runs at the next submit);
//! - `frame`: until the GPU also finished one frame of the whole scene at `--size` (camera fitted
//!   to the last state), i.e. the first frame that shows the event. It includes the `gpu` wait, so
//!   it is an upper bound of apply → frame.
//!
//! The GPU time of the scatter kernels alone is not separated: `GpuTimer` times render passes and
//! the two methods do their GPU work in different places (a copy at the next submit, compute
//! passes), so the wall clock to idle is the common measure. Medians over the runs are printed;
//! `--out` writes every sample as JSON lines. Numbers are not claims on their own: record the GPU
//! state with them (decision 0049).

use pointblitz_core::{Applied, Camera, Headless, SlotScene, UploadMethod};
use pointblitz_io::slots::Fold;
use pointblitz_io::slots::dir::StreamDir;
use std::fmt::Write as _;
use std::time::Instant;

fn arg<'a>(args: &'a [String], key: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == key)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}

/// One event applied once: ms from before `apply`.
#[derive(Debug, Clone, Copy)]
struct Sample {
    apply: f64,
    gpu: f64,
    frame: f64,
}

pub fn run(args: &[String]) -> Result<(), String> {
    let dir = arg(args, "--stream").ok_or("--stream <live/stream dir> is required")?;
    let runs: usize = arg(args, "--runs")
        .unwrap_or("5")
        .parse()
        .map_err(|_| "--runs")?;
    let methods: Vec<UploadMethod> = arg(args, "--upload")
        .unwrap_or("mirror,scatter")
        .split(',')
        .map(str::parse)
        .collect::<Result<_, _>>()?;
    let size = arg(args, "--size").unwrap_or("1920x1080");
    let (w, h) = size.split_once('x').ok_or("--size WxH")?;
    let size = [
        w.parse().map_err(|_| "--size")?,
        h.parse().map_err(|_| "--size")?,
    ];
    let out = arg(args, "--out");

    let mut stream = StreamDir::open(dir).map_err(|e| format!("{dir}: {e}"))?;
    let messages = stream.messages().map_err(|e| format!("{dir}: {e}"))?;
    if messages.is_empty() {
        return Err(format!("{dir}: no events"));
    }
    let (device, queue, info) =
        pointblitz_core::headless::device().ok_or("no GPU adapter available")?;
    eprintln!(
        "adapter: {} ({:?}, driver {})",
        info.name, info.backend, info.driver_info
    );
    for m in &methods {
        if !m.supported(&device) {
            return Err(format!("this device cannot use {m}"));
        }
    }

    // Camera: the whole last state from the south-west and above.
    let mut fold = Fold::new();
    for (_, m) in &messages {
        fold.apply(m).map_err(|e| e.to_string())?;
    }
    let mut last = SlotScene::new([0.0; 3]);
    last.apply_state(&device, &queue, &fold.message())
        .map_err(|e| e.to_string())?;
    let camera = fit(last.bounds().ok_or("the stream shows no points")?);
    drop(last);
    let mut hl = Headless::new(&device, size);

    let mut one_run = |method: UploadMethod| -> Result<Vec<Sample>, String> {
        let mut s = SlotScene::new([0.0; 3]).with_upload(method);
        let mut samples = Vec::with_capacity(messages.len());
        for (e, m) in &messages {
            let t0 = Instant::now();
            let r = s.apply(&device, &queue, m).map_err(|e| e.to_string())?;
            let t1 = Instant::now();
            let index = queue.submit([]);
            device
                .poll(wgpu::PollType::Wait {
                    submission_index: Some(index),
                    timeout: None,
                })
                .map_err(|e| e.to_string())?;
            let t2 = Instant::now();
            hl.render_with_slots(&device, &queue, &camera, &[], &[&s]);
            let t3 = Instant::now();
            if r != Applied::Shown(vec![e.seq]) || s.points() != e.points {
                return Err(format!("event {}: {r:?}, {} points", e.seq, s.points()));
            }
            let ms = |t: Instant| (t - t0).as_secs_f64() * 1e3;
            samples.push(Sample {
                apply: ms(t1),
                gpu: ms(t2),
                frame: ms(t3),
            });
        }
        Ok(samples)
    };

    for &m in &methods {
        one_run(m)?;
    }
    // samples[method][run][event]
    let mut samples: Vec<Vec<Vec<Sample>>> = vec![Vec::new(); methods.len()];
    let mut lines = String::new();
    for run in 0..runs {
        let mut order: Vec<usize> = (0..methods.len()).collect();
        if run % 2 == 1 {
            order.reverse();
        }
        for k in order {
            let r = one_run(methods[k])?;
            for ((e, _), x) in messages.iter().zip(&r) {
                writeln!(
                    lines,
                    r#"{{"run":{run},"upload":"{}","seq":{},"kind":"{}","n_add":{},"n_move":{},"n_remove":{},"apply_ms":{:.3},"gpu_ms":{:.3},"frame_ms":{:.3}}}"#,
                    methods[k], e.seq, e.kind, e.n_add, e.n_move, e.n_remove, x.apply, x.gpu, x.frame
                )
                .unwrap();
            }
            samples[k].push(r);
        }
    }
    if let Some(path) = out {
        std::fs::write(path, &lines).map_err(|e| format!("{path}: {e}"))?;
    }

    println!(
        "slot-apply: {} events, {runs} runs, frame {}x{}, medians in ms (apply / gpu done / frame done)",
        messages.len(),
        size[0],
        size[1]
    );
    let mut head = format!(
        "{:>3} {:8} {:>7} {:>7} {:>7}",
        "seq", "kind", "add", "move", "remove"
    );
    for m in &methods {
        write!(head, " | {:^23}", m.as_str()).unwrap();
    }
    println!("{head}");
    for (i, (e, _)) in messages.iter().enumerate() {
        let mut row = format!(
            "{:>3} {:8} {:>7} {:>7} {:>7}",
            e.seq, e.kind, e.n_add, e.n_move, e.n_remove
        );
        for per_run in &samples {
            let pick = |f: fn(&Sample) -> f64| median(per_run.iter().map(|r| f(&r[i])).collect());
            write!(
                row,
                " | {:>7.2} {:>7.2} {:>7.2}",
                pick(|s| s.apply),
                pick(|s| s.gpu),
                pick(|s| s.frame)
            )
            .unwrap();
        }
        println!("{row}");
    }
    Ok(())
}

fn median(mut v: Vec<f64>) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    v.sort_by(f64::total_cmp);
    let n = v.len();
    if n % 2 == 1 {
        v[n / 2]
    } else {
        0.5 * (v[n / 2 - 1] + v[n / 2])
    }
}

/// A camera that sees the box from the south-west and above (like overview_sw, decision 0017).
fn fit((lo, hi): ([f64; 3], [f64; 3])) -> Camera {
    let c = [0, 1, 2].map(|a| 0.5 * (lo[a] + hi[a]));
    let r = 0.5 * (0..3).map(|a| (hi[a] - lo[a]).powi(2)).sum::<f64>().sqrt();
    let d = r.max(1.0) / 25f64.to_radians().sin();
    let dir = [-0.5, -0.7, 0.5f64];
    let n = dir.iter().map(|v| v * v).sum::<f64>().sqrt();
    let eye = [0, 1, 2].map(|a| c[a] + dir[a] / n * d);
    Camera::new(eye, c, [0.0, 0.0, 1.0], 50.0)
}

#[cfg(test)]
mod tests {
    use super::median;

    #[test]
    fn median_of_odd_and_even_counts() {
        assert_eq!(median(vec![3.0, 1.0, 2.0]), 2.0);
        assert_eq!(median(vec![4.0, 1.0, 2.0, 3.0]), 2.5);
        assert!(median(Vec::new()).is_nan());
    }
}
