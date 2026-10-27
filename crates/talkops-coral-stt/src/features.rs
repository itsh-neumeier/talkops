//! Log-mel front end on the host CPU, the same as `tools/coral/coraltools/
//! features.py` (NeMo defaults: Hann window, Slaney mel scale, log with a
//! guard of 2^-24, normalisation per feature). The Edge TPU runs only the
//! acoustic model.

use serde::Deserialize;

use crate::fft::power_spectrum;

/// Front end parameters from the model metadata.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default)]
pub struct Frontend {
    pub sample_rate: u32,
    pub window_size: f32,
    pub window_stride: f32,
    pub n_fft: usize,
    pub features: usize,
    pub preemph: f32,
    pub normalize: String,
    pub log_guard: f32,
}

impl Default for Frontend {
    fn default() -> Self {
        Self {
            sample_rate: 16_000,
            window_size: 0.02,
            window_stride: 0.01,
            n_fft: 512,
            features: 64,
            preemph: 0.97,
            normalize: "per_feature".into(),
            log_guard: 2f32.powi(-24),
        }
    }
}

impl Frontend {
    pub fn hop(&self) -> usize {
        (self.sample_rate as f32 * self.window_stride).round() as usize
    }

    pub fn win(&self) -> usize {
        (self.sample_rate as f32 * self.window_size).round() as usize
    }

    /// Output frames for a number of samples.
    pub fn frames(&self, samples: usize) -> usize {
        samples / self.hop() + 1
    }
}

/// Precomputed window and mel filter bank.
pub struct LogMel {
    pub fe: Frontend,
    window: Vec<f32>,
    /// `features` rows of `n_fft / 2 + 1` weights.
    bank: Vec<Vec<f32>>,
}

fn hz_to_mel(f: f64) -> f64 {
    if f >= 1000.0 {
        15.0 + (f / 1000.0).ln() / (6.4f64.ln() / 27.0)
    } else {
        f / (200.0 / 3.0)
    }
}

fn mel_to_hz(m: f64) -> f64 {
    if m >= 15.0 {
        1000.0 * ((6.4f64.ln() / 27.0) * (m - 15.0)).exp()
    } else {
        m * (200.0 / 3.0)
    }
}

/// Slaney-style mel filter bank with area normalisation (as librosa).
pub fn mel_filterbank(rate: u32, n_fft: usize, n_mels: usize) -> Vec<Vec<f32>> {
    let bins = n_fft / 2 + 1;
    let fft_freqs: Vec<f64> = (0..bins)
        .map(|k| k as f64 * f64::from(rate) / 2.0 / (bins - 1) as f64)
        .collect();
    let (lo, hi) = (hz_to_mel(0.0), hz_to_mel(f64::from(rate) / 2.0));
    let hz: Vec<f64> = (0..n_mels + 2)
        .map(|i| mel_to_hz(lo + (hi - lo) * i as f64 / (n_mels + 1) as f64))
        .collect();
    (0..n_mels)
        .map(|m| {
            let norm = 2.0 / (hz[m + 2] - hz[m]);
            fft_freqs
                .iter()
                .map(|f| {
                    let lower = (f - hz[m]) / (hz[m + 1] - hz[m]);
                    let upper = (hz[m + 2] - f) / (hz[m + 2] - hz[m + 1]);
                    (lower.min(upper).max(0.0) * norm) as f32
                })
                .collect()
        })
        .collect()
}

impl LogMel {
    pub fn new(fe: Frontend) -> Self {
        let mut window = vec![0.0; fe.n_fft];
        let win = fe.win().min(fe.n_fft);
        let off = (fe.n_fft - win) / 2;
        for i in 0..win {
            // Periodic Hann window, as numpy's hanning(win + 1)[:-1].
            window[off + i] =
                0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / win as f32).cos();
        }
        let bank = mel_filterbank(fe.sample_rate, fe.n_fft, fe.features);
        Self { fe, window, bank }
    }

    /// Features of `samples` (at the model rate) as `features` rows of
    /// `frames` values, row-major, padded with zeros or cut to `frames`.
    pub fn compute(&self, samples: &[f32], frames: usize) -> Vec<f32> {
        let fe = &self.fe;
        let mut x = Vec::with_capacity(samples.len() + fe.n_fft);
        let pad = fe.n_fft / 2;
        x.resize(pad, 0.0);
        for (i, s) in samples.iter().enumerate() {
            let prev = if i == 0 { 0.0 } else { samples[i - 1] };
            x.push(if fe.preemph != 0.0 && i > 0 {
                s - fe.preemph * prev
            } else {
                *s
            });
        }
        x.resize(x.len() + pad, 0.0);
        let hop = fe.hop();
        let n = if x.len() >= fe.n_fft {
            1 + (x.len() - fe.n_fft) / hop
        } else {
            0
        };
        let used = n.min(frames);
        let mut out = vec![0.0f32; fe.features * frames];
        let (mut re, mut im, mut frame) = (Vec::new(), Vec::new(), vec![0.0; fe.n_fft]);
        for t in 0..used {
            for (k, v) in frame.iter_mut().enumerate() {
                *v = x[t * hop + k] * self.window[k];
            }
            let spec = power_spectrum(&frame, &mut re, &mut im);
            for (m, row) in self.bank.iter().enumerate() {
                let e: f32 = row.iter().zip(&spec).map(|(w, p)| w * p).sum();
                out[m * frames + t] = (e + fe.log_guard).ln();
            }
        }
        if fe.normalize == "per_feature" && used > 0 {
            for m in 0..fe.features {
                let row = &mut out[m * frames..m * frames + used];
                let mean = row.iter().sum::<f32>() / used as f32;
                let var = row.iter().map(|v| (v - mean) * (v - mean)).sum::<f32>() / used as f32;
                let std = var.sqrt() + 1e-5;
                for v in row.iter_mut() {
                    *v = (*v - mean) / std;
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shape_and_normalisation() {
        let mel = LogMel::new(Frontend::default());
        let samples: Vec<f32> = (0..25_500).map(|i| (i as f32 * 0.21).sin() * 0.2).collect();
        let frames = Frontend::default().frames(samples.len());
        let f = mel.compute(&samples, frames);
        assert_eq!(f.len(), 64 * frames);
        for m in 0..64 {
            let row = &f[m * frames..(m + 1) * frames];
            let mean = row.iter().sum::<f32>() / frames as f32;
            assert!(mean.abs() < 1e-3, "row {m}: {mean}");
        }
        // Padding: a longer target is zero after the audio.
        let padded = mel.compute(&samples[..1600], 256);
        assert_eq!(padded.len(), 64 * 256);
        assert!(padded[200..256].iter().all(|v| *v == 0.0));
    }

    #[test]
    fn filterbank_rows_are_triangles() {
        let fb = mel_filterbank(16_000, 512, 64);
        assert_eq!(fb.len(), 64);
        assert!(
            fb.iter()
                .all(|r| r.len() == 257 && r.iter().all(|w| *w >= 0.0))
        );
        assert!(fb.iter().all(|r| r.iter().sum::<f32>() > 0.0));
    }
}
