//! Fixed viewpoints for a point cloud (decision 0017).
//!
//! All coordinates are ENU (x east, y north, z up), metres. Azimuth is measured clockwise
//! from north, elevation up from the horizon.

use std::fmt::Write as _;

pub const FOV_Y_DEG: f64 = 50.0;
pub const WIDTH: u32 = 1920;
pub const HEIGHT: u32 = 1080;
/// Robust bounds use these percentiles per axis so a few stray points do not move the views.
const LO_PCT: f64 = 0.01;
const HI_PCT: f64 = 0.99;
/// Voxel size for finding the densest area (close-up view target).
const DENSE_VOXEL_M: f64 = 2.0;
/// Coarse screen grid used to check that a view is not nearly empty.
const GRID: (usize, usize) = (64, 36);
/// A view passes when at least this share of points is on screen ...
pub const MIN_VISIBLE: f64 = 0.01;
/// ... and at least this share of coarse screen cells is covered.
pub const MIN_COVERAGE: f64 = 0.05;

#[derive(Debug, Clone, PartialEq)]
pub struct Viewpoint {
    pub name: &'static str,
    pub eye: [f64; 3],
    pub target: [f64; 3],
    pub up: [f64; 3],
}

#[derive(Debug, Clone, PartialEq)]
pub struct Bounds {
    pub min: [f64; 3],
    pub max: [f64; 3],
}

impl Bounds {
    fn center(&self) -> [f64; 3] {
        [0, 1, 2].map(|i| 0.5 * (self.min[i] + self.max[i]))
    }
    fn radius(&self) -> f64 {
        0.5 * norm(sub(self.max, self.min))
    }
}

/// Per-axis percentile bounds.
pub fn robust_bounds(points: &[[f32; 3]]) -> Bounds {
    let mut min = [0.0; 3];
    let mut max = [0.0; 3];
    for axis in 0..3 {
        let mut v: Vec<f32> = points.iter().map(|p| p[axis]).collect();
        v.sort_by(f32::total_cmp);
        let at = |q: f64| v[((v.len() - 1) as f64 * q).round() as usize] as f64;
        min[axis] = at(LO_PCT);
        max[axis] = at(HI_PCT);
    }
    Bounds { min, max }
}

/// Centre of the voxel holding the most points inside `b`.
pub fn densest_point(points: &[[f32; 3]], b: &Bounds) -> [f64; 3] {
    use std::collections::HashMap;
    let mut counts: HashMap<[i64; 3], u32> = HashMap::new();
    for p in points {
        let p = p.map(f64::from);
        if (0..3).any(|i| p[i] < b.min[i] || p[i] > b.max[i]) {
            continue;
        }
        let key = p.map(|c| (c / DENSE_VOXEL_M).floor() as i64);
        *counts.entry(key).or_default() += 1;
    }
    let (key, _) = counts
        .into_iter()
        .max_by_key(|(k, c)| (*c, std::cmp::Reverse(*k)))
        .expect("no points inside bounds");
    key.map(|k| (k as f64 + 0.5) * DENSE_VOXEL_M)
}

/// The 8 viewpoints. `d` is the distance at which the bounding sphere fills the vertical field of view.
pub fn derive(b: &Bounds, dense: [f64; 3]) -> Vec<Viewpoint> {
    let c = b.center();
    let d = b.radius() / (FOV_Y_DEG.to_radians() / 2.0).sin();
    let orbit = |name, target: [f64; 3], az: f64, el: f64, dist: f64| {
        let (az, el) = (az.to_radians(), el.to_radians());
        let dir = [az.sin() * el.cos(), az.cos() * el.cos(), el.sin()];
        Viewpoint {
            name,
            eye: add(target, scale(dir, dist)),
            target,
            up: [0.0, 0.0, 1.0],
        }
    };
    vec![
        orbit("overview_sw", c, 225.0, 45.0, d),
        orbit("north", c, 0.0, 30.0, d),
        orbit("east", c, 90.0, 30.0, d),
        orbit("south", c, 180.0, 30.0, d),
        orbit("west", c, 270.0, 30.0, d),
        Viewpoint {
            name: "top_down",
            eye: add(c, [0.0, 0.0, d]),
            target: c,
            up: [0.0, 1.0, 0.0],
        },
        orbit("low_south", c, 180.0, 10.0, d),
        orbit("close_dense", dense, 135.0, 35.0, 0.25 * d),
    ]
}

/// Share of points on screen and share of coarse screen cells covered.
pub fn coverage(v: &Viewpoint, points: &[[f32; 3]]) -> (f64, f64) {
    let f = normalize(sub(v.target, v.eye));
    let r = normalize(cross(f, v.up));
    let u = cross(r, f);
    let t = (FOV_Y_DEG.to_radians() / 2.0).tan();
    let aspect = f64::from(WIDTH) / f64::from(HEIGHT);
    let mut cells = vec![false; GRID.0 * GRID.1];
    let mut visible = 0usize;
    for p in points {
        let q = sub(p.map(f64::from), v.eye);
        let z = dot(q, f);
        if z <= 0.1 {
            continue;
        }
        let x = dot(q, r) / (z * t * aspect);
        let y = dot(q, u) / (z * t);
        if x.abs() > 1.0 || y.abs() > 1.0 {
            continue;
        }
        visible += 1;
        let cx = (((x + 1.0) / 2.0) * GRID.0 as f64).min(GRID.0 as f64 - 1.0) as usize;
        let cy = (((1.0 - y) / 2.0) * GRID.1 as f64).min(GRID.1 as f64 - 1.0) as usize;
        cells[cy * GRID.0 + cx] = true;
    }
    let covered = cells.iter().filter(|c| **c).count();
    (
        visible as f64 / points.len() as f64,
        covered as f64 / cells.len() as f64,
    )
}

pub fn run(input: &str, output: &str) -> Result<(), String> {
    let bytes = std::fs::read(input).map_err(|e| format!("{input}: {e}"))?;
    let pts: Vec<[f32; 3]> = pointblitz_io::points(&bytes)
        .map_err(|e| format!("{input}: {e}"))?
        .map(|p| p.position)
        .collect();
    let b = robust_bounds(&pts);
    let views = derive(&b, densest_point(&pts, &b));

    let mut json = String::new();
    let source = std::path::Path::new(input)
        .file_name()
        .map_or(input.into(), |n| n.to_string_lossy());
    writeln!(json, "{{").unwrap();
    writeln!(json, "  \"source\": \"{source}\",").unwrap();
    writeln!(json, "  \"points\": {},", pts.len()).unwrap();
    writeln!(json, "  \"frame\": \"ENU\",").unwrap();
    writeln!(
        json,
        "  \"bounds\": {{ \"min\": {}, \"max\": {} }},",
        arr(b.min),
        arr(b.max)
    )
    .unwrap();
    writeln!(
        json,
        "  \"camera\": {{ \"fov_y_deg\": {FOV_Y_DEG}, \"width\": {WIDTH}, \"height\": {HEIGHT} }},"
    )
    .unwrap();
    writeln!(json, "  \"viewpoints\": [").unwrap();
    let mut ok = true;
    for (i, v) in views.iter().enumerate() {
        let (vis, cov) = coverage(v, &pts);
        let pass = vis >= MIN_VISIBLE && cov >= MIN_COVERAGE;
        ok &= pass;
        println!(
            "{:<12} visible {:>6.2} %  coverage {:>6.2} %  {}",
            v.name,
            vis * 100.0,
            cov * 100.0,
            if pass { "ok" } else { "FAIL" }
        );
        let sep = if i + 1 < views.len() { "," } else { "" };
        writeln!(
            json,
            "    {{ \"name\": \"{}\", \"eye\": {}, \"target\": {}, \"up\": {}, \"check\": {{ \"visible\": {vis:.4}, \"coverage\": {cov:.4} }} }}{sep}",
            v.name,
            arr(v.eye),
            arr(v.target),
            arr(v.up)
        )
        .unwrap();
    }
    writeln!(json, "  ]").unwrap();
    writeln!(json, "}}").unwrap();
    std::fs::write(output, json).map_err(|e| format!("{output}: {e}"))?;
    if ok {
        Ok(())
    } else {
        Err("some viewpoints do not see enough of the cloud".into())
    }
}

fn arr(v: [f64; 3]) -> String {
    format!("[{:.3}, {:.3}, {:.3}]", v[0], v[1], v[2])
}
fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn scale(a: [f64; 3], s: f64) -> [f64; 3] {
    [a[0] * s, a[1] * s, a[2] * s]
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn norm(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}
fn normalize(a: [f64; 3]) -> [f64; 3] {
    scale(a, 1.0 / norm(a))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cube(n: i32, size: f32) -> Vec<[f32; 3]> {
        let mut v = Vec::new();
        for i in 0..n {
            for j in 0..n {
                for k in 0..n {
                    let s = size / (n - 1) as f32;
                    v.push([i as f32 * s, j as f32 * s, k as f32 * s]);
                }
            }
        }
        v
    }

    #[test]
    fn robust_bounds_ignore_outliers() {
        let mut pts = cube(10, 10.0);
        pts.push([1000.0, 1000.0, 1000.0]);
        let b = robust_bounds(&pts);
        assert!(b.max.iter().all(|m| *m <= 10.0), "{b:?}");
    }

    #[test]
    fn eight_views_all_see_a_cube() {
        let pts = cube(20, 50.0);
        let b = robust_bounds(&pts);
        let views = derive(&b, densest_point(&pts, &b));
        assert_eq!(views.len(), 8);
        for v in &views {
            let (vis, cov) = coverage(v, &pts);
            assert!(
                vis >= MIN_VISIBLE && cov >= MIN_COVERAGE,
                "{}: {vis} {cov}",
                v.name
            );
        }
    }

    #[test]
    fn north_view_looks_south() {
        let b = Bounds {
            min: [-1.0, -1.0, -1.0],
            max: [1.0, 1.0, 1.0],
        };
        let n = &derive(&b, [0.0; 3])[1];
        assert_eq!(n.name, "north");
        assert!(n.eye[1] > 0.0 && n.eye[0].abs() < 1e-9 && n.eye[2] > 0.0);
    }

    #[test]
    fn a_view_facing_away_sees_nothing() {
        let pts = cube(5, 1.0);
        let v = Viewpoint {
            name: "away",
            eye: [0.0, -10.0, 0.0],
            target: [0.0, -20.0, 0.0],
            up: [0.0, 0.0, 1.0],
        };
        assert_eq!(coverage(&v, &pts), (0.0, 0.0));
    }
}
