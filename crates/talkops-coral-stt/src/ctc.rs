//! CTC decoding for fixed windows: window ranges, stitching of the window
//! outputs, greedy decoding with word times, and a confidence value.

/// Output frames of one window: `frames x classes`, row-major.
#[derive(Debug, Clone, PartialEq)]
pub struct Matrix {
    pub rows: usize,
    pub cols: usize,
    pub data: Vec<f32>,
}

impl Matrix {
    pub fn new(rows: usize, cols: usize, data: Vec<f32>) -> Self {
        debug_assert_eq!(rows * cols, data.len());
        Self { rows, cols, data }
    }

    pub fn row(&self, r: usize) -> &[f32] {
        &self.data[r * self.cols..(r + 1) * self.cols]
    }

    pub fn transposed(&self) -> Self {
        let mut data = vec![0.0; self.data.len()];
        for r in 0..self.rows {
            for c in 0..self.cols {
                data[c * self.rows + r] = self.data[r * self.cols + c];
            }
        }
        Self::new(self.cols, self.rows, data)
    }
}

/// Input frame ranges `[start, end)` that cover `total` frames. The last
/// window can end after `total`; the caller pads it with silence.
pub fn windows(total: usize, size: usize, overlap: usize) -> Vec<(usize, usize)> {
    assert!(
        size > 0 && overlap < size,
        "need size > 0 and overlap < size"
    );
    let step = size - overlap;
    let mut out = vec![(0, size)];
    while out.last().is_some_and(|w| w.1 < total) {
        let start = out.last().map_or(0, |w| w.0) + step;
        out.push((start, start + size));
    }
    out
}

/// Joins the window outputs to one row sequence for `total` input frames.
/// Each window gives the frames from half the overlap after its start (not
/// for the first window) to half the overlap before its end (not for the
/// last window).
pub fn stitch(
    outputs: &[Matrix],
    size: usize,
    overlap: usize,
    total: usize,
    subsampling: usize,
) -> Vec<&[f32]> {
    let half = overlap / 2;
    let mut rows = Vec::new();
    for (i, (start, end)) in windows(total, size, overlap).into_iter().enumerate() {
        let Some(out) = outputs.get(i) else { break };
        let keep_from = if i == 0 { start } else { start + half };
        let keep_to = if i == outputs.len() - 1 {
            end
        } else {
            end - (overlap - half)
        }
        .min(total);
        let first = (keep_from - start) / subsampling;
        let last = (keep_to.saturating_sub(start))
            .div_ceil(subsampling)
            .min(out.rows);
        rows.extend((first..last).map(|r| out.row(r)));
    }
    rows
}

pub fn argmax(row: &[f32]) -> usize {
    let mut best = 0;
    for (k, v) in row.iter().enumerate() {
        if *v > row[best] {
            best = k;
        }
    }
    best
}

pub use talkops_stt_service::Word;

/// Greedy CTC on the best label of each frame: collapse repeats, drop
/// blanks. A space label separates words; sentencepiece labels mark a word
/// start with "▁".
pub fn collapse(ids: &[usize], labels: &[String], blank: usize, frame_secs: f32) -> Vec<Word> {
    let mut words = Vec::new();
    let mut cur = String::new();
    let (mut start, mut end) = (None::<usize>, 0usize);
    let mut prev = blank;
    let mut flush = |cur: &mut String, start: &mut Option<usize>, end: usize| {
        if let Some(s) = start.take()
            && !cur.is_empty()
        {
            words.push(Word {
                word: std::mem::take(cur),
                start: s as f32 * frame_secs,
                end: (end + 1) as f32 * frame_secs,
            });
        }
        cur.clear();
    };
    for (t, &k) in ids.iter().enumerate() {
        // A repeated label extends the word to the end of its run.
        if k != blank && k == prev && start.is_some() {
            end = t;
        }
        if k != blank && k != prev {
            let label = labels.get(k).map(String::as_str).unwrap_or_default();
            if label == " " || label.starts_with('▁') {
                flush(&mut cur, &mut start, end);
            }
            let piece = label.replace(['▁', ' '], "");
            if !piece.is_empty() {
                start.get_or_insert(t);
                cur.push_str(&piece);
                end = t;
            }
        }
        prev = k;
    }
    flush(&mut cur, &mut start, end);
    words
}

/// Mean log probability of the best non-blank label over non-blank frames.
/// The rows hold log probabilities (`log_probs`) or logits. Returns
/// `-inf` when no frame is non-blank.
pub fn confidence(rows: &[&[f32]], blank: usize, log_probs: bool) -> f32 {
    let (mut total, mut count) = (0.0f32, 0usize);
    for row in rows {
        let k = argmax(row);
        if k == blank {
            continue;
        }
        let value = if log_probs {
            row[k]
        } else {
            let max = row[k];
            let z = row.iter().map(|v| (v - max).exp()).sum::<f32>().ln() + max;
            row[k] - z
        };
        total += value;
        count += 1;
    }
    if count == 0 {
        f32::NEG_INFINITY
    } else {
        total / count as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_cover_the_input() {
        assert_eq!(windows(100, 40, 10), vec![(0, 40), (30, 70), (60, 100)]);
        assert_eq!(windows(10, 40, 10), vec![(0, 40)]);
    }

    #[test]
    fn stitch_keeps_each_frame_once() {
        let (size, overlap, sub, total) = (40, 8, 2, 100);
        let outs: Vec<Matrix> = windows(total, size, overlap)
            .into_iter()
            .map(|(start, _)| {
                Matrix::new(
                    size / sub,
                    1,
                    (0..size / sub).map(|k| (start + 2 * k) as f32).collect(),
                )
            })
            .collect();
        let joined: Vec<f32> = stitch(&outs, size, overlap, total, sub)
            .iter()
            .map(|r| r[0])
            .collect();
        let expected: Vec<f32> = (0..total).step_by(sub).map(|v| v as f32).collect();
        assert_eq!(joined, expected);
    }

    #[test]
    fn greedy_words_and_times() {
        let labels: Vec<String> = [" ", "a", "b"].iter().map(|s| s.to_string()).collect();
        let words = collapse(&[1, 1, 3, 1, 0, 2, 2, 3], &labels, 3, 0.02);
        let text: Vec<&str> = words.iter().map(|w| w.word.as_str()).collect();
        assert_eq!(text, ["aa", "b"]);
        assert!((words[1].start - 0.10).abs() < 1e-6);
        assert!((words[1].end - 0.14).abs() < 1e-6);
        let sp: Vec<String> = ["▁gu", "ten", "▁tag"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let words = collapse(&[0, 1, 3, 2], &sp, 3, 0.08);
        let text: Vec<&str> = words.iter().map(|w| w.word.as_str()).collect();
        assert_eq!(text, ["guten", "tag"]);
    }

    #[test]
    fn confidence_prefers_sharper_output() {
        let sharp: &[f32] = &[5.0, 0.0, 0.0];
        let flat: &[f32] = &[1.0, 0.8, 0.0];
        assert!(confidence(&[sharp; 4], 2, false) > confidence(&[flat; 4], 2, false));
        let blank: &[f32] = &[0.0, 0.0, 9.0];
        assert_eq!(confidence(&[blank], 2, false), f32::NEG_INFINITY);
    }
}
