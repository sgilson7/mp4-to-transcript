//! What is said. The rules around the speech-recognition model.
//!
//! The model reads thirty seconds at a time and returns lines of text, each
//! with a start and an end. The core moves through the recording in such
//! blocks, the way the model's own authors do: when a block's last line runs
//! into the block's end, that line is likely cut off, so it is dropped and the
//! next block starts where it began. Stretches where the segmentation model
//! heard no one are skipped, because the model tends to invent words in
//! silence.

use serde::Deserialize;

use crate::diarize::RATE;

/// Samples in one block: the model's thirty-second window.
pub const BLOCK: u64 = 30 * RATE;
/// A block starts this much before the first speech after the last block.
pub const LEAD: u64 = RATE / 2;
/// A line ending within this many samples of a block's end is taken to be
/// cut off by it.
pub const CUT: u64 = RATE / 2;
/// Samples per millisecond.
pub const PER_MS: u64 = RATE / 1000;

/// The next block after `seek`: from just before the next speech, for up to
/// thirty seconds. None when no speech is left.
pub fn next_block(seek: u64, total: u64, speech: &[(u64, u64)]) -> Option<(u64, u64)> {
    let heard = speech.iter().find(|(_, end)| *end > seek)?;
    let start = seek.max(heard.0.saturating_sub(LEAD));
    if start >= total {
        return None;
    }
    Some((start, (start + BLOCK).min(total)))
}

/// One line of text and the samples it covers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub start: u64,
    pub end: u64,
    pub text: String,
}

/// Why the model's answer cannot be used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadError(pub String);

impl ReadError {
    pub fn message(&self) -> String {
        format!("The speech model's answer could not be read: {}", self.0)
    }
}

#[derive(Deserialize)]
struct Answer {
    #[serde(default)]
    chunks: Vec<Chunk>,
}

#[derive(Deserialize)]
struct Chunk {
    timestamp: (Option<serde_json::Number>, Option<serde_json::Number>),
    text: String,
}

/// A decimal number of seconds as whole milliseconds, rounded half up.
/// Reads the number's text, so no float is involved: `5.76` is 5760,
/// `1e-7` is 0, `30` is 30000. Negative numbers read as 0.
pub fn millis(n: &serde_json::Number) -> u64 {
    let text = n.to_string();
    if text.starts_with('-') {
        return 0;
    }
    let (mantissa, exponent) = match text.find(['e', 'E']) {
        Some(at) => (
            &text[..at],
            text[at + 1..].trim_start_matches('+').parse::<i64>().unwrap_or(0),
        ),
        None => (&text[..], 0),
    };
    let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    let digits: String = whole.chars().chain(fraction.chars()).take(30).collect();
    let dropped = (whole.len() + fraction.len()).saturating_sub(30) as i64;
    let Ok(value) = digits.parse::<u128>() else {
        return 0;
    };
    // value * 10^(3 + exponent - fraction digits + dropped digits)
    let power = 3 + exponent - fraction.len() as i64 + dropped;
    let result = if power >= 0 {
        if power > 20 {
            u128::from(u64::MAX)
        } else {
            value.saturating_mul(10u128.pow(power as u32))
        }
    } else if power < -38 {
        0
    } else {
        let scale = 10u128.pow((-power) as u32);
        (value + scale / 2) / scale
    };
    result.min(u128::from(u64::MAX)) as u64
}

/// What one block gave: the lines kept, and where the next block may start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settled {
    pub lines: Vec<Line>,
    pub seek: u64,
}

/// Reads the model's answer for the block `[start, end)` of a recording
/// `total` samples long, and decides which lines to keep.
pub fn settle(start: u64, end: u64, total: u64, answer: &str) -> Result<Settled, ReadError> {
    let answer: Answer = serde_json::from_str(answer).map_err(|e| ReadError(e.to_string()))?;
    let mut lines: Vec<(Line, bool)> = answer
        .chunks
        .into_iter()
        .map(|c| {
            let a = start + c.timestamp.0.as_ref().map(millis).unwrap_or(0) * PER_MS;
            let b = c.timestamp.1.as_ref().map(|n| start + millis(n) * PER_MS);
            let cut = b.map(|b| b + CUT >= end).unwrap_or(true);
            let b = b.unwrap_or(end).min(end);
            (
                Line {
                    start: a.min(end),
                    end: b,
                    text: c.text,
                },
                cut,
            )
        })
        .collect();
    let mut seek = end;
    let last_is_cut = lines.last().map(|(_, cut)| *cut).unwrap_or(false);
    if end < total && last_is_cut && lines.len() > 1 {
        let (dropped, _) = lines.pop().unwrap_or_else(|| unreachable!());
        if dropped.start > start {
            seek = dropped.start;
        }
    }
    let lines = lines
        .into_iter()
        .map(|(l, _)| l)
        .filter_map(|l| {
            let text = tidy(&l.text);
            (!text.is_empty() && l.end > l.start).then_some(Line { text, ..l })
        })
        .collect();
    Ok(Settled { lines, seek })
}

/// The text with its spaces collapsed. A line that is only a bracketed tag,
/// such as `[BLANK_AUDIO]` or `(music)`, is the model describing a sound, not
/// words, and becomes empty.
pub fn tidy(text: &str) -> String {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let tag =
        (text.starts_with('[') && text.ends_with(']')) || (text.starts_with('(') && text.ends_with(')'));
    let inner = text.get(1..text.len().saturating_sub(1)).unwrap_or("");
    if tag && !inner.contains(['[', ']', '(', ')']) {
        return String::new();
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn number(text: &str) -> serde_json::Number {
        serde_json::from_str(text).unwrap()
    }

    #[test]
    fn decimal_seconds_become_milliseconds() {
        assert_eq!(millis(&number("5.76")), 5_760);
        assert_eq!(millis(&number("30")), 30_000);
        assert_eq!(millis(&number("0")), 0);
        assert_eq!(millis(&number("12.3456")), 12_346);
        assert_eq!(millis(&number("12.3454")), 12_345);
        assert_eq!(millis(&number("1e-7")), 0);
        assert_eq!(millis(&number("2.5e1")), 25_000);
        assert_eq!(millis(&number("-3")), 0);
        assert_eq!(millis(&number("5.760000000000001")), 5_760);
    }

    #[test]
    fn the_next_block_starts_half_a_second_before_speech() {
        let speech = vec![(100_000, 200_000), (900_000, 950_000)];
        assert_eq!(next_block(0, 1_000_000, &speech), Some((92_000, 572_000)));
        // Inside speech, the block starts at the seek point.
        assert_eq!(next_block(150_000, 1_000_000, &speech), Some((150_000, 630_000)));
        // After the first span, it skips to the second; the end is the total.
        assert_eq!(
            next_block(572_000, 1_000_000, &speech),
            Some((892_000, 1_000_000))
        );
        assert_eq!(next_block(950_000, 1_000_000, &speech), None);
        assert_eq!(next_block(0, 1_000_000, &[]), None);
    }

    #[test]
    fn a_line_cut_off_by_the_block_end_is_left_for_the_next_block() {
        // Block from 1 s to 31 s. The last line ends at 30 s of the block,
        // the block's end: dropped, and the next block starts at its start.
        let answer = r#"{"text":"","chunks":[
            {"timestamp":[0,5.76],"text":" Good morning."},
            {"timestamp":[27.7,30],"text":" Okay, Karen, can you draft?"}]}"#;
        let s = settle(16_000, 496_000, 2_000_000, answer).unwrap();
        assert_eq!(
            s.lines,
            vec![Line {
                start: 16_000,
                end: 16_000 + 5_760 * 16,
                text: "Good morning.".into()
            }]
        );
        assert_eq!(s.seek, 16_000 + 27_700 * 16);
    }

    #[test]
    fn a_line_with_no_end_counts_as_cut_off() {
        let answer = r#"{"chunks":[{"timestamp":[0,4],"text":"A."},{"timestamp":[20,null],"text":"B"}]}"#;
        let s = settle(0, 480_000, 2_000_000, answer).unwrap();
        assert_eq!(s.lines.len(), 1);
        assert_eq!(s.seek, 320_000);
    }

    #[test]
    fn the_last_block_keeps_its_last_line_and_closes_it_at_the_end() {
        let answer = r#"{"chunks":[{"timestamp":[0,4],"text":"A."},{"timestamp":[20,null],"text":"B"}]}"#;
        let s = settle(0, 400_000, 400_000, answer).unwrap();
        assert_eq!(
            s.lines[1],
            Line {
                start: 320_000,
                end: 400_000,
                text: "B".into()
            }
        );
        assert_eq!(s.seek, 400_000);
    }

    #[test]
    fn a_single_line_filling_the_block_is_kept_so_the_reading_moves_on() {
        let answer = r#"{"chunks":[{"timestamp":[0,30],"text":"One long sentence"}]}"#;
        let s = settle(0, 480_000, 2_000_000, answer).unwrap();
        assert_eq!(s.lines.len(), 1);
        assert_eq!(s.seek, 480_000);
    }

    #[test]
    fn a_complete_last_line_lets_the_next_block_start_at_the_block_end() {
        let answer = r#"{"chunks":[{"timestamp":[0,4],"text":"A."},{"timestamp":[20,25],"text":"B."}]}"#;
        let s = settle(0, 480_000, 2_000_000, answer).unwrap();
        assert_eq!(s.lines.len(), 2);
        assert_eq!(s.seek, 480_000);
    }

    #[test]
    fn empty_lines_and_sound_tags_are_dropped() {
        let answer = r#"{"chunks":[{"timestamp":[0,1],"text":"  "},{"timestamp":[1,2],"text":" [BLANK_AUDIO]"},
            {"timestamp":[2,3],"text":"(music)"},{"timestamp":[3,4],"text":" Hello   there. "}]}"#;
        let s = settle(0, 480_000, 480_000, answer).unwrap();
        assert_eq!(s.lines.len(), 1);
        assert_eq!(s.lines[0].text, "Hello there.");
    }

    #[test]
    fn tidy_keeps_text_that_only_contains_brackets() {
        assert_eq!(tidy("(laughs) Yes, fine (mostly)"), "(laughs) Yes, fine (mostly)");
        assert_eq!(tidy("[a] and [b]"), "[a] and [b]");
        assert_eq!(tidy("[Music]"), "");
    }

    #[test]
    fn an_answer_that_is_not_json_is_refused() {
        let e = settle(0, 1, 1, "nope").unwrap_err();
        assert!(e
            .message()
            .starts_with("The speech model's answer could not be read: "));
    }
}
