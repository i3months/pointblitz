//! PNG read/write as tightly packed RGBA8.

use std::fs::File;
use std::io::{BufReader, BufWriter};

pub struct Rgba {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

pub fn read_png(path: &str) -> Result<Rgba, String> {
    let file = File::open(path).map_err(|e| format!("{path}: {e}"))?;
    let mut decoder = png::Decoder::new(BufReader::new(file));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(|e| format!("{path}: {e}"))?;
    let mut buf = vec![0; reader.output_buffer_size().ok_or("png too large")?];
    let info = reader
        .next_frame(&mut buf)
        .map_err(|e| format!("{path}: {e}"))?;
    let px = (info.width * info.height) as usize;
    let data = match info.color_type {
        png::ColorType::Rgba => buf[..px * 4].to_vec(),
        png::ColorType::Rgb => buf[..px * 3]
            .chunks_exact(3)
            .flat_map(|c| [c[0], c[1], c[2], 255])
            .collect(),
        png::ColorType::Grayscale => buf[..px].iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::GrayscaleAlpha => buf[..px * 2]
            .chunks_exact(2)
            .flat_map(|c| [c[0], c[0], c[0], c[1]])
            .collect(),
        png::ColorType::Indexed => return Err(format!("{path}: indexed PNG not expanded")),
    };
    Ok(Rgba {
        width: info.width,
        height: info.height,
        data,
    })
}

pub fn write_png(path: &str, img: &Rgba) -> Result<(), String> {
    let file = File::create(path).map_err(|e| format!("{path}: {e}"))?;
    let mut enc = png::Encoder::new(BufWriter::new(file), img.width, img.height);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut w = enc.write_header().map_err(|e| format!("{path}: {e}"))?;
    w.write_image_data(&img.data)
        .map_err(|e| format!("{path}: {e}"))?;
    w.finish().map_err(|e| format!("{path}: {e}"))
}

/// Share of pixels that are not black (RGB all zero), and the 64×36 coarse grid coverage
/// (same measure as decision 0017 and baseline/three/capture.mjs).
pub fn coverage(img: &Rgba) -> (f64, f64) {
    const GX: u32 = 64;
    const GY: u32 = 36;
    let mut cells = vec![false; (GX * GY) as usize];
    let mut lit = 0usize;
    for y in 0..img.height {
        for x in 0..img.width {
            let i = ((y * img.width + x) * 4) as usize;
            if img.data[i] | img.data[i + 1] | img.data[i + 2] != 0 {
                lit += 1;
                let cx = (x * GX / img.width).min(GX - 1);
                let cy = (y * GY / img.height).min(GY - 1);
                cells[(cy * GX + cx) as usize] = true;
            }
        }
    }
    let total = f64::from(img.width) * f64::from(img.height);
    (
        lit as f64 / total,
        cells.iter().filter(|c| **c).count() as f64 / cells.len() as f64,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_round_trip() {
        let img = Rgba {
            width: 3,
            height: 2,
            data: (0..24).map(|v| (v * 10) as u8).collect(),
        };
        let path = std::env::temp_dir().join(format!("pb-png-{}.png", std::process::id()));
        let p = path.to_str().unwrap();
        write_png(p, &img).unwrap();
        let back = read_png(p).unwrap();
        std::fs::remove_file(p).ok();
        assert_eq!((back.width, back.height), (3, 2));
        assert_eq!(back.data, img.data);
    }

    #[test]
    fn coverage_counts_non_black() {
        let mut data = vec![0u8; 64 * 36 * 4];
        data[0] = 1;
        let (lit, grid) = coverage(&Rgba {
            width: 64,
            height: 36,
            data,
        });
        assert_eq!(lit, 1.0 / (64.0 * 36.0));
        assert_eq!(grid, 1.0 / (64.0 * 36.0));
    }
}
