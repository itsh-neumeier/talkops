//! Audio input: WAV decoding, PCM16 conversion and resampling.

use std::io::Cursor;

/// Sample rate of the models.
pub const MODEL_RATE: u32 = 16_000;

#[derive(Debug, thiserror::Error)]
pub enum AudioError {
    #[error("unsupported audio: {0} (send WAV)")]
    Unsupported(String),
}

/// Decodes a WAV file to mono `f32` samples (channels mixed) and its rate.
pub fn decode_wav(bytes: &[u8]) -> Result<(Vec<f32>, u32), AudioError> {
    let reader = hound::WavReader::new(Cursor::new(bytes))
        .map_err(|e| AudioError::Unsupported(e.to_string()))?;
    let spec = reader.spec();
    let channels = usize::from(spec.channels.max(1));
    let interleaved: Vec<f32> = match (spec.sample_format, spec.bits_per_sample) {
        (hound::SampleFormat::Float, 32) => reader
            .into_samples::<f32>()
            .collect::<Result<_, _>>()
            .map_err(|e| AudioError::Unsupported(e.to_string()))?,
        (hound::SampleFormat::Int, bits @ (8 | 16 | 24 | 32)) => {
            let scale = (1i64 << (bits - 1)) as f32;
            reader
                .into_samples::<i32>()
                .map(|s| s.map(|v| v as f32 / scale))
                .collect::<Result<_, _>>()
                .map_err(|e| AudioError::Unsupported(e.to_string()))?
        }
        (format, bits) => {
            return Err(AudioError::Unsupported(format!(
                "{format:?} with {bits} bits"
            )));
        }
    };
    let mono = interleaved
        .chunks(channels)
        .map(|f| f.iter().sum::<f32>() / f.len() as f32)
        .collect();
    Ok((mono, spec.sample_rate))
}

/// Little-endian PCM16 to `f32`.
pub fn pcm16(bytes: &[u8]) -> Vec<f32> {
    bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| f32::from(i16::from_le_bytes(*b)) / 32768.0)
        .collect()
}

/// Resampling with a windowed-sinc low-pass filter (for any rate pair;
/// telephone audio is 8 kHz, the Realtime protocol uses 24 kHz).
pub fn resample(samples: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to || samples.is_empty() {
        return samples.to_vec();
    }
    let ratio = f64::from(to) / f64::from(from);
    let cutoff = ratio.min(1.0) * 0.95;
    const HALF: isize = 16;
    let len = (samples.len() as f64 * ratio).round() as usize;
    (0..len)
        .map(|i| {
            let pos = i as f64 / ratio;
            let centre = pos.floor() as isize;
            let (mut sum, mut norm) = (0.0f64, 0.0f64);
            for k in centre - HALF + 1..=centre + HALF {
                let Some(s) = usize::try_from(k).ok().and_then(|k| samples.get(k)) else {
                    continue;
                };
                let x = pos - k as f64;
                let sinc = if x.abs() < 1e-9 {
                    1.0
                } else {
                    (std::f64::consts::PI * x * cutoff).sin() / (std::f64::consts::PI * x * cutoff)
                };
                let window = 0.5 + 0.5 * (std::f64::consts::PI * x / HALF as f64).cos();
                let w = sinc * window;
                sum += f64::from(*s) * w;
                norm += w;
            }
            if norm.abs() > 1e-9 {
                (sum / norm) as f32
            } else {
                0.0
            }
        })
        .collect()
}

fn gcd(a: u32, b: u32) -> u32 {
    if b == 0 { a } else { gcd(b, a % b) }
}

/// Incremental resampler for streams: keeps a little history so chunk
/// borders do not click.
pub struct StreamResampler {
    from: u32,
    to: u32,
    pending: Vec<f32>,
    /// Output samples produced so far.
    produced: usize,
    /// Input samples consumed (dropped from `pending`) so far.
    consumed: usize,
}

impl StreamResampler {
    pub fn new(from: u32, to: u32) -> Self {
        Self {
            from,
            to,
            pending: Vec::new(),
            produced: 0,
            consumed: 0,
        }
    }

    /// Adds input and returns all output that has enough right context.
    pub fn push(&mut self, input: &[f32]) -> Vec<f32> {
        if self.from == self.to {
            return input.to_vec();
        }
        self.pending.extend_from_slice(input);
        let ratio = f64::from(self.to) / f64::from(self.from);
        let available = self.consumed + self.pending.len();
        // Output position i needs input up to i / ratio + 16.
        let ready = ((available as f64 - 17.0) * ratio).floor().max(0.0) as usize;
        if ready <= self.produced {
            return Vec::new();
        }
        let all = resample(&self.pending, self.from, self.to);
        let offset = (self.consumed as f64 * ratio).round() as usize;
        let from = self.produced.saturating_sub(offset);
        let to = (ready - offset).min(all.len());
        let out = all.get(from..to).map(<[f32]>::to_vec).unwrap_or_default();
        self.produced += out.len();
        // Keep 64 input samples of history before the next output. The cut
        // must keep the output grid: a multiple of `from / gcd` inputs.
        let unit = (self.from / gcd(self.from, self.to)) as usize;
        let keep_from = ((self.produced as f64 / ratio) as usize).saturating_sub(64) / unit * unit;
        if keep_from > self.consumed {
            let drop = keep_from - self.consumed;
            self.pending.drain(..drop.min(self.pending.len()));
            self.consumed = keep_from;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(rate: u32, secs: f32, hz: f32) -> Vec<f32> {
        (0..(rate as f32 * secs) as usize)
            .map(|i| 0.5 * (2.0 * std::f32::consts::PI * hz * i as f32 / rate as f32).sin())
            .collect()
    }

    fn rms(x: &[f32]) -> f32 {
        (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt()
    }

    #[test]
    fn resamples_keeping_the_level() {
        for (from, to) in [(8000, 16_000), (24_000, 16_000), (48_000, 16_000)] {
            let y = resample(&tone(from, 1.0, 440.0), from, to);
            assert_eq!(y.len(), to as usize);
            assert!(
                (rms(&y[200..y.len() - 200]) - 0.3535).abs() < 0.02,
                "{from}->{to}"
            );
        }
        // A tone above the new Nyquist frequency is removed.
        let y = resample(&tone(24_000, 1.0, 10_000.0), 24_000, 16_000);
        assert!(rms(&y[200..y.len() - 200]) < 0.05);
    }

    #[test]
    fn stream_resampler_matches_the_block_resampler() {
        let x = tone(24_000, 1.0, 440.0);
        let mut s = StreamResampler::new(24_000, 16_000);
        let mut out = Vec::new();
        for chunk in x.chunks(1234) {
            out.extend(s.push(chunk));
        }
        let block = resample(&x, 24_000, 16_000);
        assert!(out.len() > 15_900, "{}", out.len());
        let diff = out
            .iter()
            .zip(&block)
            .skip(100)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f32::max);
        assert!(diff < 0.01, "{diff}");
    }

    #[test]
    fn decodes_pcm16_stereo_wav() {
        let mut bytes = Vec::new();
        {
            let mut w = hound::WavWriter::new(
                Cursor::new(&mut bytes),
                hound::WavSpec {
                    channels: 2,
                    sample_rate: 8000,
                    bits_per_sample: 16,
                    sample_format: hound::SampleFormat::Int,
                },
            )
            .unwrap();
            for _ in 0..80 {
                w.write_sample(16384i16).unwrap();
                w.write_sample(0i16).unwrap();
            }
            w.finalize().unwrap();
        }
        let (mono, rate) = decode_wav(&bytes).unwrap();
        assert_eq!((mono.len(), rate), (80, 8000));
        assert!((mono[0] - 0.25).abs() < 1e-4);
        assert!(decode_wav(b"not a wav").is_err());
        assert_eq!(pcm16(&[0, 0x80, 0xff, 0x7f]), vec![-1.0, 32767.0 / 32768.0]);
    }
}
