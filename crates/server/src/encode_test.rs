//! `encode-test` (P3.1): encode fixed-viewpoint and orbit frames, record cost per frame.

use crate::nvenc::{Config, Encoder};
use glam::DVec3;
use pointblitz_core::{Camera, Headless, Scene};
use pointblitz_io::chunk::ply_to_chunks;
use std::fmt::Write as _;
use std::time::Instant;

const HOLD: usize = 30;
const ORBIT: usize = 240;
const ORBIT_STEP_DEG: f64 = 1.5;
const SAMPLE_EVERY: usize = 30;

fn arg<'a>(args: &'a [String], key: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == key)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}

type Views = Vec<(String, Camera)>;

/// What every frame of a stream is rendered with.
struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    scene: Scene,
    hl: Headless,
}

fn load_views(path: &str) -> Result<(Views, u32, u32), String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let json: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("{path}: {e}"))?;
    let cam = &json["camera"];
    let fov = cam["fov_y_deg"].as_f64().ok_or("fov_y_deg")?;
    let w = cam["width"].as_u64().ok_or("width")? as u32;
    let h = cam["height"].as_u64().ok_or("height")? as u32;
    let v3 = |v: &serde_json::Value| -> Option<[f64; 3]> {
        let a = v.as_array()?;
        Some([
            a.first()?.as_f64()?,
            a.get(1)?.as_f64()?,
            a.get(2)?.as_f64()?,
        ])
    };
    let views = json["viewpoints"]
        .as_array()
        .ok_or("viewpoints")?
        .iter()
        .map(|v| {
            Ok((
                v["name"].as_str().ok_or("name")?.to_string(),
                Camera::new(
                    v3(&v["eye"]).ok_or("eye")?,
                    v3(&v["target"]).ok_or("target")?,
                    v3(&v["up"]).ok_or("up")?,
                    fov,
                ),
            ))
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok((views, w, h))
}

fn write_png(path: &str, w: u32, h: u32, rgba: &[u8]) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| format!("{path}: {e}"))?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut wr = enc.write_header().map_err(|e| e.to_string())?;
    wr.write_image_data(rgba).map_err(|e| e.to_string())?;
    wr.finish().map_err(|e| e.to_string())
}

/// Rotates `v` about unit axis `k` by `angle` (Rodrigues).
fn rotate(v: DVec3, k: DVec3, angle: f64) -> DVec3 {
    let (s, c) = angle.sin_cos();
    v * c + k.cross(v) * s + k * k.dot(v) * (1.0 - c)
}

struct Stream {
    h264: Vec<u8>,
    json: String,
}

impl Stream {
    fn new() -> Self {
        Self {
            h264: Vec::new(),
            json: String::from("{\"frames\":[\n"),
        }
    }
}

/// Encodes `frames` (camera, optional sample name) and writes `<name>.h264/.json/-src`.
fn encode_stream(
    out: &str,
    name: &str,
    gpu: &mut Gpu,
    config: Config,
    frames: &[(Camera, Option<String>)],
) -> Result<String, String> {
    let src = format!("{out}/{name}-src");
    std::fs::create_dir_all(&src).map_err(|e| format!("{src}: {e}"))?;
    let mut enc = Encoder::new(config)?;
    let mut s = Stream::new();
    let (mut enc_ms, mut up_ms, mut cap_ms) = (Vec::new(), Vec::new(), Vec::new());
    for (i, (camera, sample)) in frames.iter().enumerate() {
        let t = Instant::now();
        let rgba = gpu.hl.capture(&gpu.device, &gpu.queue, camera, &gpu.scene);
        cap_ms.push(t.elapsed().as_secs_f64() * 1e3);
        let f = enc.encode(&rgba, false)?;
        if let Some(n) = sample {
            write_png(
                &format!("{src}/{n}.png"),
                config.width,
                config.height,
                &rgba,
            )?;
        }
        enc_ms.push(f.encode_ms);
        up_ms.push(f.upload_ms);
        writeln!(
            s.json,
            "{}{{\"i\":{i},\"bytes\":{},\"idr\":{},\"upload_ms\":{:.3},\"encode_ms\":{:.3},\"sample\":{}}}",
            if i == 0 { "" } else { "," },
            f.bytes.len(),
            f.idr,
            f.upload_ms,
            f.encode_ms,
            sample.as_ref().map_or("null".into(), |n| format!("\"{n}\""))
        )
        .unwrap();
        s.h264.extend_from_slice(&f.bytes);
    }
    let pct = |v: &mut Vec<f64>, q: f64| {
        v.sort_by(f64::total_cmp);
        v[((v.len() - 1) as f64 * q).round() as usize]
    };
    let bytes = s.h264.len();
    let secs = frames.len() as f64 / f64::from(config.fps);
    let summary = format!(
        "\"qp\":{},\"frame_count\":{},\"bytes\":{bytes},\"mbps_at_fps\":{:.2},\"encode_ms_p50\":{:.3},\"encode_ms_p95\":{:.3},\"encode_ms_p99\":{:.3},\"upload_ms_p50\":{:.3},\"capture_ms_p50\":{:.3}",
        config.qp,
        frames.len(),
        bytes as f64 * 8.0 / secs / 1e6,
        pct(&mut enc_ms, 0.5),
        pct(&mut enc_ms, 0.95),
        pct(&mut enc_ms, 0.99),
        pct(&mut up_ms, 0.5),
        pct(&mut cap_ms, 0.5),
    );
    s.json.push_str(&format!("],\n{summary}}}\n"));
    std::fs::write(format!("{out}/{name}.h264"), &s.h264).map_err(|e| e.to_string())?;
    std::fs::write(format!("{out}/{name}.json"), &s.json).map_err(|e| e.to_string())?;
    Ok(format!("{{\"stream\":\"{name}\",{summary}}}"))
}

pub fn run(args: &[String]) -> Result<(), String> {
    let ply = arg(args, "--ply").ok_or("--ply <file> is required")?;
    let vp = arg(args, "--viewpoints").ok_or("--viewpoints <json> is required")?;
    let out = arg(args, "--out").ok_or("--out <dir> is required")?;
    let qps: Vec<u32> = arg(args, "--qp")
        .unwrap_or("18,23,28")
        .split(',')
        .map(|q| q.trim().parse().map_err(|_| format!("bad QP {q}")))
        .collect::<Result<_, _>>()?;
    let (views, w, h) = load_views(vp)?;

    let (device, queue, info) =
        pointblitz_core::headless::device().ok_or("no GPU adapter available")?;
    eprintln!("adapter: {} ({:?})", info.name, info.backend);
    let bytes = std::fs::read(ply).map_err(|e| format!("{ply}: {e}"))?;
    let mut scene = Scene::new();
    for c in ply_to_chunks(&bytes, 0, 256 * 1024).map_err(|e| e.to_string())? {
        scene.insert(&device, &c).map_err(|e| e.to_string())?;
    }
    let hl = Headless::new(&device, [w, h]);
    let mut gpu = Gpu {
        device,
        queue,
        scene,
        hl,
    };

    let mut held = Vec::new();
    for (name, cam) in &views {
        for k in 0..HOLD {
            held.push((cam.clone(), (k + 1 == HOLD).then(|| name.clone())));
        }
    }
    let base = views.first().ok_or("no viewpoints")?.1.clone();
    let up = base.up.normalize();
    let orbit: Vec<_> = (0..ORBIT)
        .map(|i| {
            let mut c = base.clone();
            c.eye = base.target
                + rotate(
                    base.eye - base.target,
                    up,
                    (i as f64 * ORBIT_STEP_DEG).to_radians(),
                );
            (
                c,
                (i % SAMPLE_EVERY == SAMPLE_EVERY - 1).then(|| format!("orbit_{i:03}")),
            )
        })
        .collect();

    let mut lines = Vec::new();
    for qp in qps {
        let dir = format!("{out}/qp{qp}");
        std::fs::create_dir_all(&dir).map_err(|e| format!("{dir}: {e}"))?;
        let config = Config {
            width: w,
            height: h,
            fps: 60,
            qp,
        };
        for (name, frames) in [("views", &held), ("orbit", &orbit)] {
            let line = encode_stream(&dir, name, &mut gpu, config, frames)?;
            println!("{line}");
            lines.push(line);
        }
    }
    std::fs::write(format!("{out}/summary.jsonl"), lines.join("\n") + "\n")
        .map_err(|e| e.to_string())
}
