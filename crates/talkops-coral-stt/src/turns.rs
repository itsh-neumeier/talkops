//! Turns in a file: the same energy rules as the turn detector of the
//! TalkOps realtime client (`talkops-media-worker/src/realtime.rs`): 20 ms
//! frames, 300 ms lead-in, 700 ms of silence end a turn, a turn is cut
//! after 15 s, and a turn with less than 200 ms of speech is dropped.

const FRAME_SECS: f32 = 0.02;
const LEAD_FRAMES: usize = 15;
const END_FRAMES: usize = 35;
const MIN_SPEECH_FRAMES: usize = 10;
const MAX_TURN_SECS: f32 = 15.0;

/// Sample ranges `[start, end)` of the turns.
pub fn split(samples: &[f32], rate: u32) -> Vec<(usize, usize)> {
    let frame = (rate as f32 * FRAME_SECS) as usize;
    if frame == 0 {
        return Vec::new();
    }
    let max_frames = (MAX_TURN_SECS / FRAME_SECS) as usize;
    let mut floor = 0.002f32;
    let mut out = Vec::new();
    // (start frame, speech frames, silent frames)
    let mut turn: Option<(usize, usize, usize)> = None;
    let frames = samples.len() / frame;
    for i in 0..frames {
        let f = &samples[i * frame..(i + 1) * frame];
        let rms = (f.iter().map(|s| s * s).sum::<f32>() / frame as f32).sqrt();
        let speech = rms >= (floor * 4.0).max(0.006);
        if !speech {
            floor = (floor * 0.98 + rms * 0.02).min(0.05);
        }
        match turn.as_mut() {
            None if speech => turn = Some((i.saturating_sub(LEAD_FRAMES), 1, 0)),
            None => {}
            Some((start, speech_frames, silent)) => {
                if speech {
                    *speech_frames += 1;
                    *silent = 0;
                } else {
                    *silent += 1;
                }
                let end = i + 1;
                if *silent >= END_FRAMES || end - *start >= max_frames {
                    if *speech_frames >= MIN_SPEECH_FRAMES {
                        out.push((*start * frame, end * frame));
                    }
                    turn = (*silent < END_FRAMES).then_some((end, 0, 0));
                }
            }
        }
    }
    if let Some((start, speech_frames, _)) = turn
        && speech_frames >= MIN_SPEECH_FRAMES
    {
        out.push((start * frame, (frames * frame).min(samples.len())));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const R: u32 = 8000;

    fn tone(secs: f32) -> Vec<f32> {
        (0..(secs * R as f32) as usize)
            .map(|i| (i as f32 * 0.3).sin() * 0.3)
            .collect()
    }

    fn silence(secs: f32) -> Vec<f32> {
        vec![0.0; (secs * R as f32) as usize]
    }

    #[test]
    fn turns_with_lead_in_like_the_worker() {
        let mut audio = silence(1.0);
        audio.extend(tone(2.0));
        audio.extend(silence(1.0));
        let turns = split(&audio, R);
        assert_eq!(turns.len(), 1);
        let (s, e) = (turns[0].0 as f32 / R as f32, turns[0].1 as f32 / R as f32);
        assert!((s - 0.7).abs() < 0.03 && (e - 3.7).abs() < 0.03, "{s} {e}");
    }

    #[test]
    fn splits_long_speech_and_drops_clicks() {
        assert_eq!(split(&tone(40.0), R).len(), 3);
        let mut audio = tone(0.06);
        audio.extend(silence(1.0));
        assert!(split(&audio, R).is_empty());
    }
}
