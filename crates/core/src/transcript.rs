//! The transcript: each line of text given a speaker, lines joined where one
//! speaker's sentence was split, speakers numbered, and the result written in
//! Zoom's two layouts.
//!
//! A line goes to the speaker heard for most of it. A line under which no
//! speaker was named goes to the speaker whose turn is nearest in time. A
//! line under which no one at all was heard is dropped: the speech model wrote
//! it over silence.
//!
//! Speakers are numbered in the order they first speak, so "Speaker 1" is
//! always the first voice in the transcript.

use crate::diarize::{overlap, Turn};
use crate::transcribe::{Line, PER_MS};

/// Two lines from one speaker this close together are one remark split by
/// the model, when the first does not end a sentence.
pub const JOIN_GAP: u64 = 1_000 * PER_MS;
/// A speaker must be heard under at least a fifth of a line to be its speaker.
pub const HEARD_SHARE_DIVISOR: u64 = 5;
/// A joined line is never longer than this.
pub const JOIN_LIMIT: u64 = 30_000 * PER_MS;

/// One line of the transcript. `speaker` counts from 0 in order of first
/// speech; the reader sees it counted from 1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cue {
    pub start: u64,
    pub end: u64,
    pub speaker: usize,
    pub text: String,
}

/// Whether anyone is heard during `[start, end)`.
fn heard(start: u64, end: u64, speech: &[(u64, u64)]) -> bool {
    speech.iter().any(|(a, b)| overlap(start, end, *a, *b) > 0)
}

/// The speaker heard most during `[start, end)`, if a named speaker was heard
/// for at least a fifth (`HEARD_SHARE_DIVISOR`) of it. The speech model's line ends are often a
/// few tenths of a second out, so a line can graze the next speaker's turn.
/// Ties go to the lower speaker number.
fn heard_most(start: u64, end: u64, turns: &[Turn]) -> Option<usize> {
    let speakers = turns.iter().map(|t| t.speaker + 1).max().unwrap_or(0);
    let mut heard = vec![0u64; speakers];
    for t in turns {
        heard[t.speaker] += overlap(start, end, t.start, t.end);
    }
    let mut best: Option<usize> = None;
    for (s, h) in heard.iter().enumerate() {
        if *h > 0 && best.map(|b| *h > heard[b]).unwrap_or(true) {
            best = Some(s);
        }
    }
    best.filter(|b| heard[*b] * HEARD_SHARE_DIVISOR >= end - start)
}

/// The speaker of the turn nearest `[start, end)`, or speaker 0 with no turns.
fn nearest(start: u64, end: u64, turns: &[Turn]) -> usize {
    let distance = |t: &Turn| {
        if t.end <= start {
            start - t.end
        } else {
            t.start.saturating_sub(end)
        }
    };
    turns
        .iter()
        .min_by_key(|t| (distance(t), t.speaker))
        .map(|t| t.speaker)
        .unwrap_or(0)
}

fn ends_sentence(text: &str) -> bool {
    text.trim_end_matches(['"', '\'', ')', ']'])
        .ends_with(['.', '?', '!', '…'])
}

/// Builds the transcript from the model's lines, the speakers' turns, and
/// where anyone was heard.
pub fn build(lines: &[Line], turns: &[Turn], speech: &[(u64, u64)]) -> Vec<Cue> {
    let mut cues: Vec<Cue> = Vec::new();
    for line in lines {
        if !heard(line.start, line.end, speech) {
            continue;
        }
        // With no named speaker under it, a line that finishes the last
        // line's sentence is the same speaker's; otherwise the nearest turn's.
        let continues = |last: &Cue| !ends_sentence(&last.text) && line.start <= last.end + JOIN_GAP;
        let speaker = match (heard_most(line.start, line.end, turns), cues.last()) {
            (Some(s), _) => s,
            (None, Some(last)) if continues(last) => last.speaker,
            (None, _) => nearest(line.start, line.end, turns),
        };
        if let Some(last) = cues.last_mut() {
            let joins = last.speaker == speaker
                && line.start <= last.end + JOIN_GAP
                && !ends_sentence(&last.text)
                && line.end.saturating_sub(last.start) <= JOIN_LIMIT;
            if joins {
                last.end = last.end.max(line.end);
                last.text = format!("{} {}", last.text, line.text);
                continue;
            }
        }
        cues.push(Cue {
            start: line.start,
            end: line.end,
            speaker,
            text: line.text.clone(),
        });
    }
    // Number speakers by first speech.
    let mut order: Vec<usize> = Vec::new();
    for c in &mut cues {
        c.speaker = match order.iter().position(|s| *s == c.speaker) {
            Some(p) => p,
            None => {
                order.push(c.speaker);
                order.len() - 1
            }
        };
    }
    cues
}

/// How many speakers the transcript names.
pub fn speakers(cues: &[Cue]) -> usize {
    cues.iter().map(|c| c.speaker + 1).max().unwrap_or(0)
}

/// `HH:MM:SS.mmm` for a sample number.
pub fn stamp(sample: u64) -> String {
    let ms = sample / PER_MS;
    format!(
        "{:02}:{:02}:{:02}.{:03}",
        ms / 3_600_000,
        ms / 60_000 % 60,
        ms / 1_000 % 60,
        ms % 1_000
    )
}

/// `HH:MM:SS` for a sample number.
pub fn clock(sample: u64) -> String {
    stamp(sample)[..8].to_string()
}

/// `M:SS`, or `H:MM:SS` from an hour, for a number of milliseconds.
pub fn duration(ms: u64) -> String {
    let s = ms / 1_000;
    if s >= 3_600 {
        format!("{}:{:02}:{:02}", s / 3_600, s / 60 % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

/// Zoom's WebVTT layout: numbered cues, `Name: text`.
pub fn vtt(cues: &[Cue], label: &dyn Fn(usize) -> String) -> String {
    let mut out = String::from("WEBVTT\n");
    for (i, c) in cues.iter().enumerate() {
        out.push_str(&format!(
            "\n{}\n{} --> {}\n{}: {}\n",
            i + 1,
            stamp(c.start),
            stamp(c.end),
            label(c.speaker),
            c.text
        ));
    }
    out
}

/// Zoom's saved-transcript text layout: `[Name] HH:MM:SS`, then the text.
pub fn txt(cues: &[Cue], label: &dyn Fn(usize) -> String) -> String {
    cues.iter()
        .map(|c| format!("[{}] {}\n{}\n", label(c.speaker), clock(c.start), c.text))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    const S: u64 = 16_000; // samples in a second

    fn line(a: u64, b: u64, text: &str) -> Line {
        Line {
            start: a * S,
            end: b * S,
            text: text.into(),
        }
    }

    fn turn(a: u64, b: u64, speaker: usize) -> Turn {
        Turn {
            start: a * S,
            end: b * S,
            speaker,
        }
    }

    fn label(n: usize) -> String {
        format!("Speaker {}", n + 1)
    }

    #[test]
    fn each_line_goes_to_the_speaker_heard_most_and_numbers_follow_first_speech() {
        // Cluster 2 speaks first, so it becomes speaker 0.
        let turns = vec![turn(0, 5, 2), turn(4, 6, 0), turn(10, 20, 0)];
        let speech = vec![(0, 20 * S)];
        let cues = build(&[line(0, 5, "Hello."), line(10, 14, "Hi.")], &turns, &speech);
        assert_eq!(cues[0].speaker, 0);
        assert_eq!(cues[1].speaker, 1);
        assert_eq!(speakers(&cues), 2);
    }

    #[test]
    fn a_line_with_no_named_speaker_goes_to_the_nearest_turn() {
        // Heard (speech) but no turn under it: nearest turn ends 1 s before.
        let turns = vec![turn(0, 4, 1), turn(12, 14, 0)];
        let speech = vec![(0, 14 * S)];
        let cues = build(&[line(0, 4, "One."), line(5, 7, "Two.")], &turns, &speech);
        assert_eq!(cues[1].speaker, cues[0].speaker);
    }

    #[test]
    fn a_line_with_no_named_speaker_that_ends_a_sentence_goes_with_its_start() {
        // "first." is heard (speech) but no named turn is under it, and the
        // next speaker's turn is nearer than the last. It finishes the
        // sentence before it, so it is that speaker's, and the two join.
        // Speaker 0's turn ends at 4 s, a second before; speaker 1's starts
        // a tenth of a second before the line ends (5% of it), so it is the
        // nearer, and it grazes the line.
        let next = Turn {
            start: 7 * S - S / 10,
            end: 9 * S,
            speaker: 1,
        };
        let turns = vec![turn(0, 4, 0), next];
        let speech = vec![(0, 9 * S)];
        let lines = [
            line(0, 5, "so students have time with proofs"),
            line(5, 7, "first."),
            line(7, 9, "I agree."),
        ];
        let cues = build(&lines, &turns, &speech);
        assert_eq!(cues.len(), 2);
        assert_eq!(cues[0].text, "so students have time with proofs first.");
        assert_eq!(cues[1].speaker, 1);
    }

    #[test]
    fn a_line_over_silence_is_dropped() {
        let turns = vec![turn(0, 4, 0)];
        let speech = vec![(0, 4 * S)];
        let cues = build(
            &[line(0, 4, "Real."), line(10, 12, "Thank you.")],
            &turns,
            &speech,
        );
        assert_eq!(cues.len(), 1);
    }

    #[test]
    fn with_no_turns_every_line_is_speaker_one() {
        let speech = vec![(0, 10 * S)];
        let cues = build(&[line(0, 4, "A."), line(5, 6, "B.")], &[], &speech);
        assert_eq!(cues.iter().map(|c| c.speaker).collect::<Vec<_>>(), vec![0, 0]);
    }

    #[test]
    fn a_sentence_split_by_the_model_is_joined() {
        let turns = vec![turn(0, 20, 0)];
        let speech = vec![(0, 20 * S)];
        let lines = [
            line(0, 5, "I think the first two weeks"),
            line(5, 7, "are too crowded."),
            line(7, 9, "Next sentence."),
        ];
        let cues = build(&lines, &turns, &speech);
        assert_eq!(cues.len(), 2);
        assert_eq!(cues[0].text, "I think the first two weeks are too crowded.");
        assert_eq!((cues[0].start, cues[0].end), (0, 7 * S));
    }

    #[test]
    fn lines_are_not_joined_across_speakers_gaps_or_the_length_limit() {
        let speech = vec![(0, 100 * S)];
        let two = vec![turn(0, 5, 0), turn(5, 9, 1)];
        assert_eq!(
            build(&[line(0, 5, "and so"), line(5, 9, "yes")], &two, &speech).len(),
            2
        );
        let one = vec![turn(0, 100, 0)];
        assert_eq!(
            build(&[line(0, 5, "and so"), line(7, 9, "yes")], &one, &speech).len(),
            2
        );
        assert_eq!(
            build(&[line(0, 25, "and so"), line(25, 31, "yes")], &one, &speech).len(),
            2
        );
    }

    #[test]
    fn stamps_and_durations() {
        assert_eq!(stamp(0), "00:00:00.000");
        assert_eq!(stamp(16 * 5_760), "00:00:05.760");
        // 1 h 2 min 3.456 s
        assert_eq!(stamp(16 * 3_723_456), "01:02:03.456");
        assert_eq!(clock(16 * 3_723_456), "01:02:03");
        assert_eq!(duration(44_228), "0:44");
        assert_eq!(duration(605_000), "10:05");
        assert_eq!(duration(3_723_456), "1:02:03");
    }

    #[test]
    fn the_vtt_matches_zooms_layout() {
        let cues = vec![
            Cue {
                start: 0,
                end: 16 * 5_760,
                speaker: 0,
                text: "Good morning.".into(),
            },
            Cue {
                start: 16 * 5_760,
                end: 16 * 12_520,
                speaker: 1,
                text: "Morning.".into(),
            },
        ];
        assert_eq!(
            vtt(&cues, &label),
            "WEBVTT\n\n1\n00:00:00.000 --> 00:00:05.760\nSpeaker 1: Good morning.\n\n2\n00:00:05.760 --> 00:00:12.520\nSpeaker 2: Morning.\n"
        );
        assert_eq!(
            txt(&cues, &label),
            "[Speaker 1] 00:00:00\nGood morning.\n\n[Speaker 2] 00:00:05\nMorning.\n"
        );
        assert_eq!(vtt(&[], &label), "WEBVTT\n");
    }
}
