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

fn ssim_channel(a: &[f64], b: &[f64], w: usize, h: usize) -> f64 {
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
    let n = ma.len();
    let mut sum = 0.0;
    for i in 0..n {
        let (mx, my) = (ma[i], mb[i]);
        let vx = saa[i] - mx * mx;
        let vy = sbb[i] - my * my;
        let cxy = sab[i] - mx * my;
        sum +=
            ((2.0 * mx * my + C1) * (2.0 * cxy + C2)) / ((mx * mx + my * my + C1) * (vx + vy + C2));
    }
    sum / n as f64
}

/// SSIM of two RGBA8 images of the same size (alpha ignored).
pub fn ssim_rgba(a: &[u8], b: &[u8], w: u32, h: u32) -> f64 {
    let (w, h) = (w as usize, h as usize);
    assert_eq!(a.len(), w * h * 4);
    assert_eq!(b.len(), w * h * 4);
    assert!(
        w > 2 * RADIUS && h > 2 * RADIUS,
        "image smaller than the SSIM window"
    );
    let channel = |img: &[u8], c: usize| -> Vec<f64> {
        img.chunks_exact(4).map(|p| f64::from(p[c])).collect()
    };
    (0..3)
        .map(|c| ssim_channel(&channel(a, c), &channel(b, c), w, h))
        .sum::<f64>()
        / 3.0
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
    fn kernel_is_normalised() {
        assert!((kernel().iter().sum::<f64>() - 1.0).abs() < 1e-12);
    }
}
