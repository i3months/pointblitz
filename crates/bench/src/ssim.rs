//! SSIM (Wang et al. 2004): 11×11 Gaussian window, σ = 1.5, K1 = 0.01, K2 = 0.03, L = 255,
//! mean over the valid region (window fully inside the image), RGB channels averaged.
//! Same definition as the previous research repo and SPEC §6.2.

const RADIUS: usize = 5;
const SIGMA: f64 = 1.5;
const C1: f64 = (0.01 * 255.0) * (0.01 * 255.0);
const C2: f64 = (0.03 * 255.0) * (0.03 * 255.0);

fn kernel() -> [f64; 2 * RADIUS + 1] {
    let mut k = [0.0; 2 * RADIUS + 1];
    for (i, v) in k.iter_mut().enumerate() {
        let d = i as f64 - RADIUS as f64;
        *v = (-d * d / (2.0 * SIGMA * SIGMA)).exp();
    }
    let s: f64 = k.iter().sum();
    k.map(|v| v / s)
}

/// Separable Gaussian blur over the valid region only; output is (w − 10) × (h − 10).
fn blur(img: &[f64], w: usize, h: usize, k: &[f64]) -> Vec<f64> {
    let ow = w - 2 * RADIUS;
    let oh = h - 2 * RADIUS;
    let mut rows = vec![0.0; ow * h];
    for y in 0..h {
        for x in 0..ow {
            rows[y * ow + x] = (0..k.len()).map(|i| k[i] * img[y * w + x + i]).sum();
        }
    }
    let mut out = vec![0.0; ow * oh];
    for y in 0..oh {
        for x in 0..ow {
            out[y * ow + x] = (0..k.len()).map(|i| k[i] * rows[(y + i) * ow + x]).sum();
        }
    }
    out
}

/// Per-window SSIM over the valid region ((w − 10) × (h − 10), window centred at (x + 5, y + 5)).
fn ssim_channel_map(a: &[f64], b: &[f64], w: usize, h: usize) -> Vec<f64> {
    let k = kernel();
    let ab: Vec<f64> = a.iter().zip(b).map(|(x, y)| x * y).collect();
    let aa: Vec<f64> = a.iter().map(|x| x * x).collect();
    let bb: Vec<f64> = b.iter().map(|y| y * y).collect();
    let (ma, mb) = (blur(a, w, h, &k), blur(b, w, h, &k));
    let (saa, sbb, sab) = (
        blur(&aa, w, h, &k),
        blur(&bb, w, h, &k),
        blur(&ab, w, h, &k),
    );
    (0..ma.len())
        .map(|i| {
            let (mx, my) = (ma[i], mb[i]);
            let vx = saa[i] - mx * mx;
            let vy = sbb[i] - my * my;
            let cxy = sab[i] - mx * my;
            ((2.0 * mx * my + C1) * (2.0 * cxy + C2)) / ((mx * mx + my * my + C1) * (vx + vy + C2))
        })
        .collect()
}

/// SSIM map averaged over the RGB channels.
fn ssim_map_rgba(a: &[u8], b: &[u8], w: usize, h: usize) -> Vec<f64> {
    assert_eq!(a.len(), w * h * 4);
    assert_eq!(b.len(), w * h * 4);
    assert!(
        w > 2 * RADIUS && h > 2 * RADIUS,
        "image smaller than the SSIM window"
    );
    let channel = |img: &[u8], c: usize| -> Vec<f64> {
        img.as_chunks::<4>()
            .0
            .iter()
            .map(|p| f64::from(p[c]))
            .collect()
    };
    let mut map = vec![0.0; (w - 2 * RADIUS) * (h - 2 * RADIUS)];
    for c in 0..3 {
        for (m, v) in map
            .iter_mut()
            .zip(ssim_channel_map(&channel(a, c), &channel(b, c), w, h))
        {
            *m += v / 3.0;
        }
    }
    map
}

/// SSIM of two RGBA8 images of the same size (alpha ignored).
pub fn ssim_rgba(a: &[u8], b: &[u8], w: u32, h: u32) -> f64 {
    let map = ssim_map_rgba(a, b, w as usize, h as usize);
    map.iter().sum::<f64>() / map.len() as f64
}

/// A pixel counts as content when a colour channel exceeds this; video compression turns pure
/// black into small non-zero values, which must not count as points.
pub const LIT_THRESHOLD: u8 = 24;

/// SSIM restricted to the point area (P3.4, PR #10 review): the mean over windows whose centre
/// pixel is lit in either image. The black background, 80–90 % of most views, otherwise lifts
/// SSIM toward 1. Returns (masked SSIM, share of windows in the mask); None if nothing is lit.
pub fn ssim_rgba_masked(a: &[u8], b: &[u8], w: u32, h: u32) -> Option<(f64, f64)> {
    let (w, h) = (w as usize, h as usize);
    let map = ssim_map_rgba(a, b, w, h);
    let ow = w - 2 * RADIUS;
    let lit = |img: &[u8], x: usize, y: usize| {
        let i = (y * w + x) * 4;
        img[i].max(img[i + 1]).max(img[i + 2]) > LIT_THRESHOLD
    };
    let (mut sum, mut n) = (0.0, 0usize);
    for (i, v) in map.iter().enumerate() {
        let (x, y) = (i % ow + RADIUS, i / ow + RADIUS);
        if lit(a, x, y) || lit(b, x, y) {
            sum += v;
            n += 1;
        }
    }
    (n > 0).then(|| (sum / n as f64, n as f64 / map.len() as f64))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn noise(w: u32, h: u32, seed: u32) -> Vec<u8> {
        let mut s = seed;
        (0..w * h * 4)
            .map(|_| {
                s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                (s >> 24) as u8
            })
            .collect()
    }

    #[test]
    fn identical_is_one() {
        let a = noise(40, 30, 1);
        assert!((ssim_rgba(&a, &a, 40, 30) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn symmetric_and_lower_for_different_images() {
        let a = noise(40, 30, 1);
        let b = noise(40, 30, 2);
        let s = ssim_rgba(&a, &b, 40, 30);
        assert!(s < 0.1, "{s}");
        assert!((s - ssim_rgba(&b, &a, 40, 30)).abs() < 1e-12);
    }

    #[test]
    fn small_change_stays_high() {
        let a = noise(40, 30, 3);
        let mut b = a.clone();
        b[4 * (15 * 40 + 20)] ^= 0x10; // one channel of one pixel
        let s = ssim_rgba(&a, &b, 40, 30);
        assert!(s > 0.999 && s < 1.0, "{s}");
    }

    #[test]
    fn masked_ssim_ignores_the_black_background() {
        // 40×30 black; a 10×10 lit square differs between the images.
        let (w, h) = (40u32, 30u32);
        let mut a = vec![0u8; (w * h * 4) as usize];
        let mut b = a.clone();
        for y in 10..20 {
            for x in 15..25 {
                let i = ((y * w + x) * 4) as usize;
                a[i..i + 3].copy_from_slice(&[200, 100, 50]);
                b[i..i + 3].copy_from_slice(&[100, 200, 150]);
            }
        }
        let full = ssim_rgba(&a, &b, w, h);
        let (masked, share) = ssim_rgba_masked(&a, &b, w, h).unwrap();
        assert!(masked < full, "{masked} < {full}");
        assert!(share > 0.0 && share < 0.5);
        assert!(
            ssim_rgba_masked(
                &vec![0; (w * h * 4) as usize],
                &vec![0; (w * h * 4) as usize],
                w,
                h
            )
            .is_none()
        );
    }

    #[test]
    fn kernel_is_normalised() {
        assert!((kernel().iter().sum::<f64>() - 1.0).abs() < 1e-12);
    }
}
