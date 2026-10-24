//! A small radix-2 FFT for the log-mel front end (window sizes are powers
//! of two, 512 by default).

/// Power spectrum `|X[k]|^2` for `k = 0..=n/2` of a real frame of length `n`
/// (a power of two).
pub fn power_spectrum(frame: &[f32], re: &mut Vec<f32>, im: &mut Vec<f32>) -> Vec<f32> {
    let n = frame.len();
    debug_assert!(n.is_power_of_two());
    re.clear();
    re.extend_from_slice(frame);
    im.clear();
    im.resize(n, 0.0);
    // Bit-reversal permutation.
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let angle = -2.0 * std::f64::consts::PI / len as f64;
        let (w_re, w_im) = (angle.cos(), angle.sin());
        for start in (0..n).step_by(len) {
            let (mut cur_re, mut cur_im) = (1.0f64, 0.0f64);
            for k in 0..len / 2 {
                let (a, b) = (start + k, start + k + len / 2);
                let t_re = (re[b] as f64) * cur_re - (im[b] as f64) * cur_im;
                let t_im = (re[b] as f64) * cur_im + (im[b] as f64) * cur_re;
                re[b] = (re[a] as f64 - t_re) as f32;
                im[b] = (im[a] as f64 - t_im) as f32;
                re[a] = (re[a] as f64 + t_re) as f32;
                im[a] = (im[a] as f64 + t_im) as f32;
                let next_re = cur_re * w_re - cur_im * w_im;
                cur_im = cur_re * w_im + cur_im * w_re;
                cur_re = next_re;
            }
        }
        len <<= 1;
    }
    (0..=n / 2).map(|k| re[k] * re[k] + im[k] * im[k]).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_a_direct_dft() {
        let n = 64;
        let x: Vec<f32> = (0..n)
            .map(|i| (i as f32 * 0.37).sin() + 0.5 * (i as f32 * 1.3).cos())
            .collect();
        let fast = power_spectrum(&x, &mut Vec::new(), &mut Vec::new());
        for (k, value) in fast.iter().enumerate() {
            let (mut re, mut im) = (0.0f64, 0.0f64);
            for (i, v) in x.iter().enumerate() {
                let a = -2.0 * std::f64::consts::PI * (k * i) as f64 / n as f64;
                re += *v as f64 * a.cos();
                im += *v as f64 * a.sin();
            }
            let direct = (re * re + im * im) as f32;
            assert!((value - direct).abs() < 1e-3 * direct.max(1.0), "bin {k}");
        }
    }
}
