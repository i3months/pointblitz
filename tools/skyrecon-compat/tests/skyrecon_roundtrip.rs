//! Compatibility with skyrecon's own PLY writer (decision 0014, decision 0023).
//!
//! Lives outside the workspace (tools/skyrecon-compat, decision 0046): skyrecon-core is a git
//! dependency pinned to a revision that is not publicly reachable. This test writes clouds with
//! `skyrecon_core::io::write_ply` in every layout and reads them back with PointBlitz's parsers.

use pointblitz_io::{PlyStream, parse_header, points};
use skyrecon_core::io::{PlyLayout, PointCloud, write_ply};

fn cloud(n: usize, normals: bool, colors: bool) -> PointCloud {
    let mut c = PointCloud::default();
    for i in 0..n {
        let f = i as f32;
        c.positions
            .push([f * 0.37 - 50.0, -f * 1.25, (f * 0.11).sin() * 7.0]);
        if normals {
            c.normals.push([0.0, (f * 0.3).cos(), (f * 0.3).sin()]);
        }
        if colors {
            c.colors.push([
                (i * 7 % 256) as u8,
                (i * 13 % 256) as u8,
                (i * 29 % 256) as u8,
            ]);
        }
    }
    c
}

fn roundtrip(c: &PointCloud, layout: PlyLayout, expected_stride: usize) {
    let dir = std::env::temp_dir().join(format!(
        "pointblitz-skyrecon-{}-{layout:?}-{}",
        std::process::id(),
        c.len()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("cloud.ply");
    write_ply(&path, c, layout).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    std::fs::remove_dir_all(&dir).ok();

    let h = parse_header(&bytes).unwrap();
    assert_eq!(h.vertex_count, c.len());
    assert_eq!(h.stride, expected_stride, "{layout:?}");

    let read: Vec<_> = points(&bytes).unwrap().collect();
    for (i, p) in read.iter().enumerate() {
        assert_eq!(p.position, c.positions[i], "{layout:?} point {i}");
        let want = if c.has_colors() {
            c.colors[i]
        } else {
            [255, 255, 255]
        };
        assert_eq!(p.color, want, "{layout:?} color {i}");
    }

    // The incremental parser gives the same points in odd-sized pieces.
    let mut s = PlyStream::new();
    let mut streamed = Vec::new();
    for piece in bytes.chunks(13) {
        s.push(piece, |p| streamed.push(p)).unwrap();
    }
    assert!(s.is_complete());
    assert!(streamed == read);
}

#[test]
fn reads_skyrecon_xyz_rgb_normal() {
    roundtrip(&cloud(1000, true, true), PlyLayout::XyzRgbNormal, 27);
}

#[test]
fn reads_skyrecon_xyz_normal_rgb() {
    roundtrip(&cloud(1000, true, true), PlyLayout::XyzNormalRgb, 27);
}

#[test]
fn reads_skyrecon_without_normals() {
    roundtrip(&cloud(500, false, true), PlyLayout::XyzRgbNormal, 15);
}
