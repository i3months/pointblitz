//! Fixed-viewpoint captures with PointBlitz and comparison against another set of captures
//! (P1.3, decisions 0017, 0020).
//!
//! - `capture --ply <file> --viewpoints <json> --out <dir> [--width 1920 --height 1080]`
//! - `compare <dir A> <dir B> [--viewpoints <json>] [--md <file>]`

use crate::image::{Rgba, coverage, read_png, write_png};
use crate::ssim::ssim_rgba;
use pointblitz_core::{Camera, Headless, Scene};
use pointblitz_io::chunk::ply_to_chunks;
use std::fmt::Write as _;
use std::time::Instant;

struct View {
    name: String,
    camera: Camera,
}

fn load_views(path: &str) -> Result<(Vec<View>, u32, u32), String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let json: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("{path}: {e}"))?;
    let cam = &json["camera"];
    let fov = cam["fov_y_deg"].as_f64().ok_or("camera.fov_y_deg")?;
    let w = cam["width"].as_u64().ok_or("camera.width")? as u32;
    let h = cam["height"].as_u64().ok_or("camera.height")? as u32;
    let v3 = |v: &serde_json::Value| -> Result<[f64; 3], String> {
        let a = v.as_array().ok_or("expected [x, y, z]")?;
        Ok([0, 1, 2].map(|i| {
            a.get(i)
                .and_then(serde_json::Value::as_f64)
                .unwrap_or(f64::NAN)
        }))
    };
    let mut views = Vec::new();
    for v in json["viewpoints"].as_array().ok_or("viewpoints")? {
        let name = v["name"].as_str().ok_or("viewpoint name")?.to_string();
        let camera = Camera::new(v3(&v["eye"])?, v3(&v["target"])?, v3(&v["up"])?, fov);
        views.push(View { name, camera });
    }
    Ok((views, w, h))
}

fn arg<'a>(args: &'a [String], key: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == key)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}

pub fn run_capture(args: &[String]) -> Result<(), String> {
    let ply = arg(args, "--ply").ok_or("--ply <file> is required")?;
    let vp = arg(args, "--viewpoints").ok_or("--viewpoints <json> is required")?;
    let out = arg(args, "--out").ok_or("--out <dir> is required")?;
    let (views, mut w, mut h) = load_views(vp)?;
    if let Some(v) = arg(args, "--width") {
        w = v.parse().map_err(|_| "--width")?;
    }
    if let Some(v) = arg(args, "--height") {
        h = v.parse().map_err(|_| "--height")?;
    }
    std::fs::create_dir_all(out).map_err(|e| format!("{out}: {e}"))?;

    let (device, queue, info) =
        pointblitz_core::headless::device().ok_or("no GPU adapter available")?;
    eprintln!(
        "adapter: {} ({:?}, driver {})",
        info.name, info.backend, info.driver_info
    );

    let bytes = std::fs::read(ply).map_err(|e| format!("{ply}: {e}"))?;
    let chunks = ply_to_chunks(&bytes, 0, 256 * 1024).map_err(|e| format!("{ply}: {e}"))?;
    let mut scene = Scene::new();
    for c in &chunks {
        scene.insert(&device, c).map_err(|e| e.to_string())?;
    }
    eprintln!("{} points in {} chunks", scene.points(), chunks.len());

    let mut hl = Headless::new(&device, [w, h]);
    for v in &views {
        let t = Instant::now();
        let data = hl.capture(&device, &queue, &v.camera, &scene);
        let img = Rgba {
            width: w,
            height: h,
            data,
        };
        let path = format!("{out}/{}.png", v.name);
        write_png(&path, &img)?;
        let (lit, grid) = coverage(&img);
        println!(
            "{:<12} lit {:>6.2} %  grid {:>6.2} %  ({:.1} ms incl. readback)",
            v.name,
            lit * 100.0,
            grid * 100.0,
            t.elapsed().as_secs_f64() * 1e3
        );
    }
    Ok(())
}

pub fn run_compare(args: &[String]) -> Result<(), String> {
    let a = args.first().ok_or("compare <dir A> <dir B>")?;
    let b = args.get(1).ok_or("compare <dir A> <dir B>")?;
    let names: Vec<String> = match arg(args, "--viewpoints") {
        Some(vp) => load_views(vp)?.0.into_iter().map(|v| v.name).collect(),
        None => {
            let mut n: Vec<String> = std::fs::read_dir(a)
                .map_err(|e| format!("{a}: {e}"))?
                .filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter_map(|f| f.strip_suffix(".png").map(str::to_string))
                .collect();
            n.sort();
            n
        }
    };
    let mut md = String::from(
        "| 시점 | SSIM | 밝은 픽셀 A | 밝은 픽셀 B | 격자 A | 격자 B | 픽셀이 다른 비율 |\n|---|---:|---:|---:|---:|---:|---:|\n",
    );
    let mut sum = 0.0;
    for n in &names {
        let ia = read_png(&format!("{a}/{n}.png"))?;
        let ib = read_png(&format!("{b}/{n}.png"))?;
        if (ia.width, ia.height) != (ib.width, ib.height) {
            return Err(format!("{n}: sizes differ"));
        }
        let s = ssim_rgba(&ia.data, &ib.data, ia.width, ia.height);
        sum += s;
        let diff = ia
            .data
            .chunks_exact(4)
            .zip(ib.data.chunks_exact(4))
            .filter(|(p, q)| p[..3] != q[..3])
            .count() as f64
            / f64::from(ia.width * ia.height);
        let (la, ga) = coverage(&ia);
        let (lb, gb) = coverage(&ib);
        writeln!(
            md,
            "| {n} | {s:.4} | {:.2} % | {:.2} % | {:.2} % | {:.2} % | {:.2} % |",
            la * 100.0,
            lb * 100.0,
            ga * 100.0,
            gb * 100.0,
            diff * 100.0
        )
        .unwrap();
    }
    writeln!(md, "| 평균 | {:.4} | | | | | |", sum / names.len() as f64).unwrap();
    print!("{md}");
    if let Some(path) = arg(args, "--md") {
        std::fs::write(path, &md).map_err(|e| format!("{path}: {e}"))?;
    }
    Ok(())
}
