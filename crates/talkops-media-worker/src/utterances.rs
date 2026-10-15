//! Builds transcript lines from timed words.
//!
//! Whisper returns segments of up to 30 s with many sentences. One such
//! block of one party, sorted only by its start, comes before all answers of
//! the other party in that time (the transcript shows "packets" of text).
//! Thus both engines (whisper.cpp and the API) give words with their times,
//! and this module makes short lines from them: at the end of a sentence, at
//! pauses, where the other party speaks, and at the latest after
//! [MAX_LINE_SECS]. Then the lines of both parties are merged by time.

use talkops_core::recordings::Segment;

/// One recognised word with its time in the recording, in seconds. The
/// text includes punctuation (`"Ja,"`).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct Word {
    pub start: f32,
    pub end: f32,
    pub text: String,
}

/// A pause of this length always starts a new line.
const PAUSE_SECS: f32 = 0.8;
/// After the end of a sentence, a pause of this length starts a new line.
const SENTENCE_PAUSE_SECS: f32 = 0.4;
/// After the end of a sentence, a line of this length ends without a pause.
const SENTENCE_LINE_SECS: f32 = 4.0;
/// From this length, a line ends at the next comma or short pause.
const SOFT_LINE_SECS: f32 = 6.0;
/// A short pause that ends a line after [SOFT_LINE_SECS].
const SHORT_PAUSE_SECS: f32 = 0.25;
/// A pause in which the other party speaks ends a line also in a sentence.
const TURN_PAUSE_SECS: f32 = 0.5;
/// No line is longer (words without punctuation and pauses).
pub(crate) const MAX_LINE_SECS: f32 = 10.0;
/// At most this many words of a new sentence at the end of a line go to
/// the next line ("… gefunden? Da" + "war kein …").
const CARRIED_WORDS: usize = 2;

fn ends_sentence(word: &str) -> bool {
    word.trim_end_matches(['"', '\'', '»', '«', ')'])
        .ends_with(['.', '?', '!', '…'])
}

fn ends_clause(word: &str) -> bool {
    word.ends_with([',', ';', ':', '-', '–'])
}

/// Makes lines of one party from its words. `other` holds the words of the
/// other party (empty for mono recordings): when the other party starts to
/// speak in a pause, a new line starts there, so the merged transcript
/// shows the turns in their real order.
pub(crate) fn lines(words: &[Word], other: &[Word], speaker: &str) -> Vec<Segment> {
    let mut other_starts: Vec<f32> = other.iter().map(|w| w.start).collect();
    other_starts.sort_by(f32::total_cmp);
    // The other party starts a word in [from, to).
    let other_speaks = |from: f32, to: f32| {
        let i = other_starts.partition_point(|s| *s < from);
        other_starts.get(i).is_some_and(|s| *s < to)
    };

    let mut out = Vec::new();
    let mut line: Vec<&Word> = Vec::new();
    // Start of the line when its first words came from the previous line.
    let mut start = None;
    for word in words.iter().filter(|w| !w.text.trim().is_empty()) {
        if let (Some(first), Some(prev)) = (line.first(), line.last()) {
            let first = start.unwrap_or(first.start);
            let gap = word.start - prev.end;
            let len = prev.end - first;
            let prev_text = prev.text.trim();
            let sentence = ends_sentence(prev_text);
            let clause = sentence || ends_clause(prev_text);
            let pause = gap >= PAUSE_SECS
                || (other_speaks(prev.end, word.start) && (clause || gap >= TURN_PAUSE_SECS));
            let split = pause
                || (sentence && (gap >= SENTENCE_PAUSE_SECS || len >= SENTENCE_LINE_SECS))
                || (clause && other_speaks(prev.start, word.start))
                || (len >= SOFT_LINE_SECS && (clause || gap >= SHORT_PAUSE_SECS))
                || word.end - first > MAX_LINE_SECS;
            if split {
                // A new sentence that starts just before a pause continues
                // after it: keep its first words together with the rest.
                let carried = if pause { carried_words(&line) } else { 0 };
                let rest = line.split_off(line.len() - carried);
                out.extend(finish(&line, start, speaker));
                line = rest;
                start = (!line.is_empty()).then_some(word.start);
            }
        }
        line.push(word);
    }
    out.extend(finish(&line, start, speaker));
    out
}

/// Number of words after the last sentence end of a line, if these are
/// few, end without punctuation and do not make up the whole line.
fn carried_words(line: &[&Word]) -> usize {
    let Some(last_end) = line.iter().rposition(|w| ends_sentence(w.text.trim())) else {
        return 0;
    };
    let n = line.len() - 1 - last_end;
    let open = line.last().is_some_and(|w| {
        let t = w.text.trim();
        !ends_sentence(t) && !ends_clause(t)
    });
    if open && (1..=CARRIED_WORDS).contains(&n) {
        n
    } else {
        0
    }
}

/// Joins the words of a line; drops markers such as `[Musik]` or `(lacht)`.
/// `start` replaces the time of the first word.
fn finish(line: &[&Word], start: Option<f32>, speaker: &str) -> Option<Segment> {
    let (first, last) = (line.first()?, line.last()?);
    let text = strip_markers(
        &line
            .iter()
            .map(|w| w.text.trim())
            .collect::<Vec<_>>()
            .join(" "),
    );
    let start = start.unwrap_or(first.start);
    (!text.is_empty()).then(|| Segment {
        start,
        end: last.end.max(start),
        speaker: speaker.to_owned(),
        text,
    })
}

/// Removes text in `[…]` and `(…)`: Whisper's notes on sounds, not speech.
fn strip_markers(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut depth = 0usize;
    for c in text.chars() {
        match c {
            '[' | '(' => depth += 1,
            ']' | ')' if depth > 0 => depth -= 1,
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Merges the lines of both parties by time. A line that repeats the
/// previous line of the same party at an overlapping time is a decoding
/// error and is dropped.
pub(crate) fn merge(mut segments: Vec<Segment>) -> Vec<Segment> {
    segments.sort_by(|a, b| a.start.total_cmp(&b.start));
    let mut out: Vec<Segment> = Vec::with_capacity(segments.len());
    for seg in segments {
        let repeated = out
            .iter()
            .rev()
            .find(|s| s.speaker == seg.speaker)
            .is_some_and(|s| s.end > seg.start && normalized(&s.text) == normalized(&seg.text));
        if !repeated {
            out.push(seg);
        }
    }
    out
}

fn normalized(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Words of one sentence, `secs` apart, starting at `start`.
    fn said(start: f32, text: &str) -> Vec<Word> {
        text.split_whitespace()
            .enumerate()
            .map(|(i, w)| Word {
                start: start + i as f32 * 0.4,
                end: start + i as f32 * 0.4 + 0.3,
                text: w.to_owned(),
            })
            .collect()
    }

    fn texts(segs: &[Segment]) -> Vec<(&str, &str)> {
        segs.iter()
            .map(|s| (s.speaker.as_str(), s.text.as_str()))
            .collect()
    }

    #[test]
    fn a_long_block_is_put_in_order_with_the_answers() {
        // Whisper returns one 20 s block for the caller; the called party
        // answers in the pauses.
        let mut caller = said(0.0, "Wir haben bestellt im Mai.");
        caller.extend(said(3.0, "So ein richtiger roher Stein."));
        caller.extend(said(8.0, "Wie ein Regenbogen drin."));
        let called = [said(2.0, "Ja."), said(6.5, "Schön.")].concat();
        let mut all = lines(&caller, &called, "caller");
        all.extend(lines(&called, &caller, "called"));
        assert_eq!(
            texts(&merge(all)),
            [
                ("caller", "Wir haben bestellt im Mai."),
                ("called", "Ja."),
                ("caller", "So ein richtiger roher Stein."),
                ("called", "Schön."),
                ("caller", "Wie ein Regenbogen drin."),
            ]
        );
    }

    #[test]
    fn sentences_without_pause_stay_together_while_short() {
        let words = said(0.0, "Ja. Ja, Schorsch, habt ihr die Karten gefunden?");
        let out = lines(&words, &[], "called");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].start, 0.0);
        assert!((out[0].end - 3.1).abs() < 1e-4, "{}", out[0].end);
    }

    #[test]
    fn a_pause_starts_a_new_line() {
        let mut words = said(0.0, "und dann");
        words.extend(said(2.0, "haben wir bestellt"));
        let out = lines(&words, &[], "");
        assert_eq!(texts(&out), [("", "und dann"), ("", "haben wir bestellt")]);
        assert_eq!(out[1].start, 2.0);
    }

    #[test]
    fn long_speech_is_split_at_commas_and_at_the_limit() {
        // 20 words, 8 s, a comma after the 16th word (6.0 s).
        let text = (1..=20)
            .map(|i| {
                if i == 16 {
                    "wort,".to_owned()
                } else {
                    "wort".to_owned()
                }
            })
            .collect::<Vec<_>>()
            .join(" ");
        let out = lines(&said(0.0, &text), &[], "");
        assert_eq!(out.len(), 2, "{out:?}");
        assert!(out[0].text.ends_with("wort,"));
        // 40 words without punctuation: at most MAX_LINE_SECS each.
        let text = vec!["wort"; 40].join(" ");
        let out = lines(&said(0.0, &text), &[], "");
        assert!(out.len() >= 2);
        assert!(out.iter().all(|s| s.end - s.start <= MAX_LINE_SECS));
    }

    #[test]
    fn the_other_party_ends_a_line_at_a_comma() {
        // No pause, but the called party speaks during "gefunden,".
        let caller = said(0.0, "habt ihr sie gefunden, oben haben wir geklingelt");
        let called = said(1.3, "Nein.");
        let out = lines(&caller, &called, "caller");
        assert_eq!(
            texts(&out),
            [
                ("caller", "habt ihr sie gefunden,"),
                ("caller", "oben haben wir geklingelt")
            ]
        );
    }

    #[test]
    fn a_sentence_start_before_a_pause_moves_to_the_next_line() {
        let mut words = said(0.0, "habt ihr die Karten gefunden? Da war");
        words.extend(said(5.0, "kein Zuhause."));
        let out = lines(&words, &[], "called");
        assert_eq!(
            texts(&out),
            [
                ("called", "habt ihr die Karten gefunden?"),
                ("called", "Da war kein Zuhause.")
            ]
        );
        // The line starts where the speech continues.
        assert_eq!(out[1].start, 5.0);
        // An answer in a short pause does not cut a sentence; in a longer
        // pause it does.
        let mut caller = said(0.0, "und dann");
        caller.extend(said(1.0, "haben wir bestellt"));
        let called = said(0.8, "Ja.");
        assert_eq!(lines(&caller, &called, "caller").len(), 1);
        let mut caller = said(0.0, "und dann");
        caller.extend(said(1.3, "haben wir bestellt"));
        assert_eq!(lines(&caller, &called, "caller").len(), 2);
    }

    #[test]
    fn markers_are_dropped() {
        let words = said(0.0, "[Musik] Hallo (lacht laut) Anna");
        assert_eq!(texts(&lines(&words, &[], "")), [("", "Hallo Anna")]);
        assert!(lines(&said(0.0, "(Musik läuft)"), &[], "").is_empty());
    }

    #[test]
    fn overlapping_repetitions_are_dropped() {
        let seg = |start: f32, end: f32, speaker: &str, text: &str| Segment {
            start,
            end,
            speaker: speaker.into(),
            text: text.into(),
        };
        let out = merge(vec![
            seg(7.0, 9.0, "caller", "Das ist rein gegangen."),
            seg(8.5, 9.5, "caller", "das ist rein gegangen"),
            seg(8.6, 9.0, "called", "Das ist rein gegangen."),
            // Said again later: real speech, kept.
            seg(12.0, 13.0, "caller", "Das ist rein gegangen."),
        ]);
        assert_eq!(out.len(), 3);
        assert_eq!(out[1].speaker, "called");
        assert_eq!(out[2].start, 12.0);
    }
}
