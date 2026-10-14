//! Who speaks when. The rules after the two speaker models have run.
//!
//! The segmentation model reads ten seconds of sound at a time and says, for
//! each frame of about seventeen milliseconds, which of up to three voices
//! are speaking. Its voices are numbered afresh in every window, so the
//! "first voice" of one window need not be the first voice of the next.
//!
//! To join windows up, the core takes each unbroken run of frames in which
//! one voice speaks alone, and asks the embedding model for a voiceprint of
//! it. A pause or another voice ends a run, so two people the segmentation
//! model mixed up within a window still give separate voiceprints.
//! Voiceprints that point the same way belong to the same speaker: the core
//! groups the runs of two seconds or more by average-linkage clustering on
//! the cosine of the angle between them, then puts each shorter run with the
//! group it is most like. A run shorter than a second gives an unreliable
//! voiceprint and is not used.
//!
//! All positions are sample numbers at 16 kHz.

use crate::bits;

/// Samples per second of the sound the models read.
pub const RATE: u64 = 16_000;
/// Samples in one segmentation window: ten seconds.
pub const WINDOW: u64 = 160_000;
/// A window shorter than this at the end of the recording is joined to the
/// one before it rather than read alone.
pub const SHORT_TAIL: u64 = RATE;
/// The model needs this many samples to produce its first frame.
pub const MIN_WINDOW: u64 = 990;
/// Samples between frames (pyannote segmentation 3.0: step 270).
pub const FRAME_STEP: u64 = 270;
/// Where frame 0 begins, so that each frame is centred on its receptive
/// field (offset 990 / 2 - 270 / 2).
pub const FRAME_OFFSET: u64 = 360;
/// Classes per frame: no voice, voice 1, 2, 3, voices 1+2, 1+3, 2+3.
pub const CLASSES: usize = 7;
/// Each class as a set of local voices, one bit each.
pub const POWERSET: [u8; CLASSES] = [0b000, 0b001, 0b010, 0b100, 0b011, 0b101, 0b110];
/// Local voices per window.
pub const LOCAL_VOICES: u8 = 3;
/// The shortest solo stretch worth a voiceprint: one second.
pub const MIN_EMBED: u64 = RATE;
/// A run at least this long (two seconds) has a voiceprint steady enough to
/// decide how many speakers there are.
pub const ANCHOR: u64 = 2 * RATE;
/// Fraction bits of the fixed-point voiceprint values.
pub const VALUE_FRAC: u32 = 20;
/// Fixed-point one for unit vectors and cosines.
pub const ONE: i64 = 1 << 15;
/// Two voiceprint groups whose average cosine is below this are different
/// speakers. Chosen from a recorded meeting (`SECOND-ORDER.md` R1.7).
pub const SAME_SPEAKER: i64 = 35 * ONE / 100;
/// A group under this share of all anchor time, in percent...
pub const SMALL_SHARE_PERCENT: u64 = 5;
/// ...and under this many samples (thirty seconds) is not a speaker of its own.
pub const SMALL_SAMPLES: u64 = 30 * RATE;

/// The windows to read, as `[start, end)`.
pub fn windows(total: u64) -> Vec<(u64, u64)> {
    let mut out: Vec<(u64, u64)> = Vec::new();
    let mut start = 0;
    while start < total {
        let end = (start + WINDOW).min(total);
        match out.last_mut() {
            Some(last) if end - start < SHORT_TAIL => last.1 = end,
            _ => out.push((start, end)),
        }
        start = end;
    }
    if out.len() == 1 && out[0].1 - out[0].0 < MIN_WINDOW {
        out.clear();
    }
    out
}

/// Why a model result cannot be used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResultError {
    Size { expected: usize, got: usize },
}

impl ResultError {
    pub fn message(&self) -> String {
        match self {
            ResultError::Size { expected, got } => {
                format!("A model returned {got} bytes where {expected} were expected.")
            }
        }
    }
}

/// One window's frames: the set of local voices speaking in each.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowFrames {
    pub start: u64,
    pub end: u64,
    pub masks: Vec<u8>,
}

impl WindowFrames {
    /// The samples frame `i` stands for, clipped to the window.
    pub fn span(&self, i: usize) -> (u64, u64) {
        let a = (self.start + FRAME_OFFSET + FRAME_STEP * i as u64).min(self.end);
        let b = (a + FRAME_STEP).min(self.end);
        (a, b)
    }
}

/// Reads the segmentation model's scores for one window: `frames` rows of
/// seven 32-bit floats. Each frame takes the class with the highest score.
pub fn read_frames(start: u64, end: u64, frames: usize, bytes: &[u8]) -> Result<WindowFrames, ResultError> {
    let expected = frames * CLASSES * 4;
    if bytes.len() != expected {
        return Err(ResultError::Size {
            expected,
            got: bytes.len(),
        });
    }
    let scores = bits::words(bytes);
    let masks = scores
        .as_chunks::<CLASSES>()
        .0
        .iter()
        .map(|row| {
            let mut best = 0;
            for k in 1..CLASSES {
                if bits::order_key(row[k]) > bits::order_key(row[best]) {
                    best = k;
                }
            }
            POWERSET[best]
        })
        .collect();
    Ok(WindowFrames { start, end, masks })
}

/// A stretch to take a voiceprint of: frames `[first, after)` of window
/// `window`, in which local voice `voice` speaks alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stretch {
    pub window: usize,
    pub voice: u8,
    pub first: usize,
    pub after: usize,
    pub start: u64,
    pub end: u64,
}

/// Every unbroken run of frames in which one voice speaks alone, lasting at
/// least `MIN_EMBED`, in the order the runs begin.
pub fn stretches(window: usize, w: &WindowFrames) -> Vec<Stretch> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < w.masks.len() {
        let mask = w.masks[i];
        let first = i;
        while i < w.masks.len() && w.masks[i] == mask {
            i += 1;
        }
        if mask.count_ones() != 1 {
            continue;
        }
        let start = w.span(first).0;
        let end = w.span(i - 1).1;
        if end - start >= MIN_EMBED {
            out.push(Stretch {
                window,
                voice: mask.trailing_zeros() as u8,
                first,
                after: i,
                start,
                end,
            });
        }
    }
    out
}

/// Reads a voiceprint: 32-bit floats, scaled to unit length in fixed point.
pub fn read_voiceprint(bytes: &[u8], dims: usize) -> Result<Vec<i64>, ResultError> {
    if bytes.len() != dims * 4 || dims == 0 {
        return Err(ResultError::Size {
            expected: dims * 4,
            got: bytes.len(),
        });
    }
    let values: Vec<i64> = bits::words(bytes)
        .into_iter()
        .map(|b| bits::fixed(b, VALUE_FRAC))
        .collect();
    Ok(unit(&values))
}

/// The vector scaled to length `ONE`. A zero vector stays zero.
pub fn unit(v: &[i64]) -> Vec<i64> {
    let squares: u128 = v
        .iter()
        .map(|x| (x.unsigned_abs() as u128) * (x.unsigned_abs() as u128))
        .sum();
    let norm = bits::isqrt(squares) as i128;
    if norm == 0 {
        return vec![0; v.len()];
    }
    v.iter()
        .map(|x| (*x as i128 * ONE as i128 / norm) as i64)
        .collect()
}

/// The cosine of the angle between two unit vectors, in units of `ONE`.
pub fn cosine(a: &[i64], b: &[i64]) -> i64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum::<i64>() / ONE
}

/// Groups voiceprints into speakers. `lengths[k]` is how many samples
/// voiceprint `k` was made from; those of at least `ANCHOR` are anchors.
///
/// The anchors are grouped by average linkage: with `target`, groups merge
/// until that many remain; without, until no two groups have an average
/// cosine of at least `SAME_SPEAKER`, and then a group too small to be a
/// speaker of its own (`is_small`) joins the group it is most like. Every
/// other voiceprint then joins the group with which its average cosine is
/// highest. With no anchors, every voiceprint is one. Returns a group number
/// for each voiceprint, numbered in the order the groups first appear.
pub fn cluster(prints: &[Vec<i64>], lengths: &[u64], target: Option<usize>) -> Vec<usize> {
    let mut anchors: Vec<usize> = (0..prints.len()).filter(|k| lengths[*k] >= ANCHOR).collect();
    if anchors.is_empty() {
        anchors = (0..prints.len()).collect();
    }
    let mut groups = link(
        &anchors.iter().map(|k| prints[*k].clone()).collect::<Vec<_>>(),
        target,
    );
    let count = groups.iter().copied().max().map(|m| m + 1).unwrap_or(0);
    // Average cosine between voiceprint k and the anchors of group g.
    let likeness = |k: usize, g: usize, groups: &[usize]| {
        let members: Vec<usize> = (0..anchors.len()).filter(|a| groups[*a] == g).collect();
        let total: i64 = members
            .iter()
            .map(|a| cosine(&prints[k], &prints[anchors[*a]]))
            .sum();
        total / members.len().max(1) as i64
    };
    if target.is_none() {
        let size = |g: usize, groups: &[usize]| -> u64 {
            (0..anchors.len())
                .filter(|a| groups[*a] == g)
                .map(|a| lengths[anchors[a]])
                .sum()
        };
        let total: u64 = anchors.iter().map(|k| lengths[*k]).sum();
        let kept: Vec<usize> = (0..count)
            .filter(|g| !is_small(size(*g, &groups), total))
            .collect();
        if !kept.is_empty() {
            let before = groups.clone();
            for a in 0..anchors.len() {
                if !kept.contains(&before[a]) {
                    // Most like, by the voiceprint's own likeness to each kept group.
                    let k = anchors[a];
                    let best = kept
                        .iter()
                        .copied()
                        .max_by_key(|g| (likeness(k, *g, &before), usize::MAX - g));
                    groups[a] = best.unwrap_or(groups[a]);
                }
            }
        }
    }
    let live: Vec<usize> = (0..count).filter(|g| groups.contains(g)).collect();
    let mut group: Vec<usize> = vec![0; prints.len()];
    for (a, k) in anchors.iter().enumerate() {
        group[*k] = groups[a];
    }
    for (k, slot) in group.iter_mut().enumerate() {
        if anchors.contains(&k) {
            continue;
        }
        let best = live
            .iter()
            .copied()
            .max_by_key(|g| (likeness(k, *g, &groups), usize::MAX - g));
        *slot = best.unwrap_or(0);
    }
    first_appearance(&group)
}

/// Whether a group with `size` samples of anchors, out of `total`, is too
/// small to be a speaker of its own: under `SMALL_SHARE_PERCENT` of all the
/// anchors and under `SMALL_SAMPLES`. A real speaker's voice drifts with
/// distance from the microphone, laughter and emphasis, and those moments
/// form small groups of their own.
pub fn is_small(size: u64, total: u64) -> bool {
    size * 100 < total * SMALL_SHARE_PERCENT && size < SMALL_SAMPLES
}

/// Renumbers groups in the order they first appear.
fn first_appearance(group: &[usize]) -> Vec<usize> {
    let mut order: Vec<usize> = Vec::new();
    group
        .iter()
        .map(|g| match order.iter().position(|o| o == g) {
            Some(p) => p,
            None => {
                order.push(*g);
                order.len() - 1
            }
        })
        .collect()
}

/// Average-linkage clustering of every voiceprint given (see `cluster`).
fn link(prints: &[Vec<i64>], target: Option<usize>) -> Vec<usize> {
    let n = prints.len();
    // sim[i][j]: average cosine between groups i and j (i, j live).
    let mut sim: Vec<Vec<i64>> = (0..n)
        .map(|i| (0..n).map(|j| cosine(&prints[i], &prints[j])).collect())
        .collect();
    let mut size: Vec<i64> = vec![1; n];
    let mut live: Vec<bool> = vec![true; n];
    let mut group: Vec<usize> = (0..n).collect();
    let mut count = n;
    loop {
        if let Some(t) = target {
            if count <= t.max(1) {
                break;
            }
        }
        let mut best: Option<(usize, usize, i64)> = None;
        for i in 0..n {
            if !live[i] {
                continue;
            }
            for j in i + 1..n {
                if live[j] && best.map(|(_, _, s)| sim[i][j] > s).unwrap_or(true) {
                    best = Some((i, j, sim[i][j]));
                }
            }
        }
        let Some((i, j, s)) = best else {
            break;
        };
        if target.is_none() && s < SAME_SPEAKER {
            break;
        }
        // Merge j into i (Lance-Williams update for average linkage).
        for k in 0..n {
            if live[k] && k != i && k != j {
                let merged = (size[i] * sim[i][k] + size[j] * sim[j][k]) / (size[i] + size[j]);
                sim[i][k] = merged;
                sim[k][i] = merged;
            }
        }
        size[i] += size[j];
        live[j] = false;
        for g in group.iter_mut() {
            if *g == j {
                *g = i;
            }
        }
        count -= 1;
    }
    first_appearance(&group)
}

/// A run of samples in which one speaker is heard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Turn {
    pub start: u64,
    pub end: u64,
    pub speaker: usize,
}

/// Joins touching or overlapping `[start, end)` spans; sorts them first.
pub fn join(mut spans: Vec<(u64, u64)>) -> Vec<(u64, u64)> {
    spans.sort();
    let mut out: Vec<(u64, u64)> = Vec::new();
    for (a, b) in spans {
        match out.last_mut() {
            Some(last) if a <= last.1 => last.1 = last.1.max(b),
            _ => out.push((a, b)),
        }
    }
    out
}

/// Each speaker's turns. `labels[k]` is the speaker of `stretches[k]`. A
/// frame where a voice is heard outside its stretches (overlapping speech,
/// or a run too short for a voiceprint) takes the speaker of the nearest
/// stretch of the same local voice that it reaches without a pause in that
/// voice. A run of a voice that reaches no stretch counts for nobody: the
/// segmentation model may give one local voice to two people, and a pause
/// is where the second may begin.
pub fn turns(frames: &[WindowFrames], stretches: &[Stretch], labels: &[usize]) -> Vec<Turn> {
    let speakers = labels.iter().copied().max().map(|m| m + 1).unwrap_or(0);
    let mut spans: Vec<Vec<(u64, u64)>> = vec![Vec::new(); speakers];
    for (w, win) in frames.iter().enumerate() {
        for voice in 0..LOCAL_VOICES {
            let heard = |i: usize| win.masks[i] & (1 << voice) != 0;
            let mut i = 0;
            while i < win.masks.len() {
                if !heard(i) {
                    i += 1;
                    continue;
                }
                // One unbroken run of this voice, overlaps included.
                let first = i;
                while i < win.masks.len() && heard(i) {
                    i += 1;
                }
                let own: Vec<usize> = (0..stretches.len())
                    .filter(|k| {
                        let s = &stretches[*k];
                        s.window == w && s.voice == voice && s.first >= first && s.after <= i
                    })
                    .collect();
                for f in first..i {
                    let distance = |k: &usize| {
                        let s = &stretches[*k];
                        if f < s.first {
                            s.first - f
                        } else {
                            (f + 1).saturating_sub(s.after)
                        }
                    };
                    if let Some(k) = own.iter().min_by_key(|k| distance(k)) {
                        spans[labels[*k]].push(win.span(f));
                    }
                }
            }
        }
    }
    let mut out: Vec<Turn> = spans
        .into_iter()
        .enumerate()
        .flat_map(|(speaker, s)| {
            join(s)
                .into_iter()
                .map(move |(start, end)| Turn { start, end, speaker })
        })
        .collect();
    out.sort_by_key(|t| (t.start, t.speaker));
    out
}

/// Where anyone is heard, whether or not a speaker could be named.
pub fn speech(frames: &[WindowFrames]) -> Vec<(u64, u64)> {
    join(
        frames
            .iter()
            .flat_map(|w| (0..w.masks.len()).filter(|i| w.masks[*i] != 0).map(|i| w.span(i)))
            .collect(),
    )
}

/// How many samples `[a0, a1)` and `[b0, b1)` share.
pub fn overlap(a0: u64, a1: u64, b0: u64, b1: u64) -> u64 {
    a1.min(b1).saturating_sub(a0.max(b0))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ONE_F: u32 = 0x3F80_0000; // 1.0
    const MINUS_F: u32 = 0xBF80_0000; // -1.0
    const TWO_F: u32 = 0x4000_0000; // 2.0

    fn le(words: &[u32]) -> Vec<u8> {
        words.iter().flat_map(|w| w.to_le_bytes()).collect()
    }

    /// A score row whose highest class is `class`.
    fn row(class: usize) -> Vec<u32> {
        (0..CLASSES)
            .map(|k| if k == class { TWO_F } else { MINUS_F })
            .collect()
    }

    fn window(start: u64, masks: &[u8]) -> WindowFrames {
        WindowFrames {
            start,
            end: start + WINDOW,
            masks: masks.to_vec(),
        }
    }

    #[test]
    fn windows_are_ten_seconds_and_a_short_tail_joins_the_last() {
        assert_eq!(windows(0), vec![]);
        assert_eq!(windows(500), vec![]);
        assert_eq!(windows(2_000), vec![(0, 2_000)]);
        assert_eq!(windows(160_000), vec![(0, 160_000)]);
        assert_eq!(
            windows(350_000),
            vec![(0, 160_000), (160_000, 320_000), (320_000, 350_000)]
        );
        // 330,000 leaves a 10,000-sample tail, under a second: joined.
        assert_eq!(windows(330_000), vec![(0, 160_000), (160_000, 330_000)]);
    }

    #[test]
    fn frames_take_the_highest_class_as_a_voice_set() {
        let mut scores = Vec::new();
        for class in [0, 1, 3, 4, 6] {
            scores.extend(row(class));
        }
        let w = read_frames(0, WINDOW, 5, &le(&scores)).unwrap();
        assert_eq!(w.masks, vec![0b000, 0b001, 0b100, 0b011, 0b110]);
    }

    #[test]
    fn a_negative_score_loses_to_a_smaller_negative_one() {
        // All negative: -1 for class 2, -2 for the rest. Class 2 wins.
        let minus_two = 0xC000_0000;
        let scores: Vec<u32> = (0..CLASSES)
            .map(|k| if k == 2 { MINUS_F } else { minus_two })
            .collect();
        let w = read_frames(0, WINDOW, 1, &le(&scores)).unwrap();
        assert_eq!(w.masks, vec![0b010]);
    }

    #[test]
    fn frames_of_the_wrong_size_are_refused() {
        let e = read_frames(0, WINDOW, 2, &le(&row(1))).unwrap_err();
        assert_eq!(
            e,
            ResultError::Size {
                expected: 56,
                got: 28
            }
        );
        assert_eq!(e.message(), "A model returned 28 bytes where 56 were expected.");
    }

    #[test]
    fn frame_spans_start_at_the_offset_and_step_by_270() {
        let w = window(160_000, &[0; 3]);
        assert_eq!(w.span(0), (160_360, 160_630));
        assert_eq!(w.span(2), (160_900, 161_170));
        let short = WindowFrames {
            start: 0,
            end: 500,
            masks: vec![0; 2],
        };
        assert_eq!(short.span(1), (500, 500));
    }

    #[test]
    fn every_solo_run_of_a_second_or_more_is_a_stretch() {
        // Voice 1 alone for 60 frames (16,200 samples: over a second), then
        // voice 2 alone for 70, both for 60, voice 2 alone for 120, a pause of
        // 3, and voice 2 again for 59 (15,930 samples: under a second).
        let mut masks = vec![0b001; 60];
        masks.extend(vec![0b010; 70]);
        masks.extend(vec![0b011; 60]);
        masks.extend(vec![0b010; 120]);
        masks.extend(vec![0b000; 3]);
        masks.extend(vec![0b010; 59]);
        let w = window(0, &masks);
        let s = stretches(4, &w);
        assert_eq!(
            s.iter().map(|x| (x.voice, x.first, x.after)).collect::<Vec<_>>(),
            vec![(0, 0, 60), (1, 60, 130), (1, 190, 310)]
        );
        assert_eq!((s[0].window, s[0].start, s[0].end), (4, 360, 360 + 60 * 270));
        assert_eq!((s[2].start, s[2].end), (360 + 190 * 270, 360 + 310 * 270));
        // 120 frames is 32,400 samples: over two seconds (32,000). 60 is not.
        assert!(s[2].end - s[2].start >= ANCHOR);
        assert!(s[0].end - s[0].start < ANCHOR);
    }

    #[test]
    fn a_solo_run_under_a_second_gets_no_voiceprint() {
        // 59 frames is 15,930 samples.
        let w = window(0, &[0b100; 59]);
        assert!(stretches(0, &w).is_empty());
    }

    #[test]
    fn voiceprints_are_scaled_to_unit_length() {
        // (3, 4) has length 5, so the unit vector is (0.6, 0.8).
        let v = read_voiceprint(&le(&[0x4040_0000, 0x4080_0000]), 2).unwrap();
        assert_eq!(v, vec![ONE * 3 / 5, ONE * 4 / 5]);
        assert_eq!(unit(&[0, 0]), vec![0, 0]);
        assert!(read_voiceprint(&le(&[ONE_F]), 2).is_err());
    }

    #[test]
    fn cosines_of_unit_vectors() {
        let x = vec![ONE, 0];
        let y = vec![0, ONE];
        assert_eq!(cosine(&x, &x), ONE);
        assert_eq!(cosine(&x, &y), 0);
        assert_eq!(cosine(&x, &[-ONE, 0]), -ONE);
    }

    /// A unit vector at `degrees` from the x axis, from a table worked out by
    /// hand (cos, sin) * 32768, rounded.
    fn at(degrees: u32) -> Vec<i64> {
        match degrees {
            0 => vec![32768, 0],
            10 => vec![32270, 5690],
            20 => vec![30792, 11207],
            40 => vec![25102, 21063],
            80 => vec![5690, 32270],
            90 => vec![0, 32768],
            _ => unreachable!(),
        }
    }

    #[test]
    fn close_voiceprints_group_and_distant_ones_do_not() {
        // 0, 10 and 20 degrees are within cos 20 = 0.94 of each other;
        // 80 and 90 likewise. Between the groups the six cosines are 0.17,
        // 0, 0.34, 0.17, 0.5 and 0.34: an average of 0.25, under 0.35.
        let prints = vec![at(0), at(80), at(10), at(90), at(20)];
        assert_eq!(
            cluster(&prints, &[ANCHOR; 5][..prints.len()], None),
            vec![0, 1, 0, 1, 0]
        );
    }

    #[test]
    fn a_target_count_forces_the_groups() {
        let prints = vec![at(0), at(80), at(10), at(90), at(20)];
        assert_eq!(cluster(&prints, &[ANCHOR; 5], Some(1)), vec![0, 0, 0, 0, 0]);
        // Three groups: the closest pair merges first (0 and 10, or 80 and
        // 90, both 10 degrees apart: the first found wins), then the next.
        let three = cluster(&prints, &[ANCHOR; 5], Some(3));
        assert_eq!(three.iter().max(), Some(&2));
        assert_eq!(three[0], three[2]);
        assert_eq!(cluster(&prints, &[ANCHOR; 5], Some(9)), vec![0, 1, 2, 3, 4]);
        assert_eq!(cluster(&[], &[], None), Vec::<usize>::new());
    }

    #[test]
    fn average_linkage_uses_the_mean_not_the_closest_member() {
        // 0 and 40 merge (cos 40 = 0.77). 90 is 50 degrees from 40 (cos 0.64,
        // over the threshold) but 90 from 0 (cos 0). The average, 0.32, is
        // under 0.35, so 90 stays apart; single linkage would have joined it.
        let prints = vec![at(0), at(40), at(90)];
        assert_eq!(cluster(&prints, &[ANCHOR; 3], None), vec![0, 0, 1]);
    }

    #[test]
    fn a_tiny_group_joins_the_speaker_it_is_most_like() {
        const S: u64 = RATE;
        // Two minutes at 0 and 10 degrees; four seconds at 90. 4 s is under
        // 5% of 124 s (6.2 s) and under 30 s: not a speaker of its own.
        let prints = vec![at(0), at(10), at(90)];
        assert_eq!(cluster(&prints, &[60 * S, 60 * S, 4 * S], None), vec![0, 0, 0]);
        // A known count of two keeps it.
        assert_eq!(cluster(&prints, &[60 * S, 60 * S, 4 * S], Some(2)), vec![0, 0, 1]);
        // 40 s is under 5% of 1,440 s but over 30 s: a speaker.
        assert_eq!(cluster(&prints, &[700 * S, 700 * S, 40 * S], None), vec![0, 0, 1]);
        // 7 s of 124 s is over 5%: a speaker.
        assert_eq!(cluster(&prints, &[60 * S, 57 * S, 7 * S], None), vec![0, 0, 1]);
        assert!(is_small(6 * S, 124 * S));
        assert!(!is_small(7 * S, 124 * S));
    }

    #[test]
    fn short_runs_join_the_group_they_are_most_like_without_making_one() {
        // Anchors at 0 and 90 degrees are two speakers. A short run at 80
        // degrees joins 90. A short run at 180 degrees is unlike both (cos -1
        // with 0, cos 0 with 90); as an anchor it would be a third speaker,
        // but a short run cannot make a group, so it joins 90, the less
        // unlike.
        let back = vec![-ONE, 0];
        let prints = vec![at(0), at(80), at(90), back];
        // The short runs last 1.9 s: long enough not to be a tiny group if
        // they could make one.
        let anchor = [ANCHOR, ANCHOR - RATE / 10, ANCHOR, ANCHOR - RATE / 10];
        assert_eq!(cluster(&prints, &anchor, None), vec![0, 1, 1, 1]);
        // With no anchors, every voiceprint is one, and 180 stands alone.
        assert_eq!(cluster(&prints, &[0; 4], None), vec![0, 1, 1, 2]);
    }

    #[test]
    fn spans_join_when_they_touch() {
        assert_eq!(join(vec![(5, 9), (0, 3), (3, 4), (8, 12)]), vec![(0, 4), (5, 12)]);
    }

    #[test]
    fn turns_follow_each_stretch_and_fill_in_from_the_nearest() {
        // Window 0: voice 1 (frames 0-1), voice 2 (frame 2), voices 1 and 2
        // together (frame 3), voice 1 again (frames 4-5). The two runs of
        // voice 1 have different labels, as two people the model mixed up.
        // After a pause, voice 1 is heard again for one frame (7) that
        // reaches no stretch: nobody. Window 1: voice 1 has a stretch; voice
        // 3 has none and counts for nobody.
        let w0 = window(0, &[0b001, 0b001, 0b010, 0b011, 0b001, 0b001, 0b000, 0b001]);
        let w1 = window(WINDOW, &[0b001, 0b100]);
        let st = |window, voice, first, after| Stretch {
            window,
            voice,
            first,
            after,
            start: 0,
            end: 0,
        };
        let s = vec![st(0, 0, 0, 2), st(0, 1, 2, 3), st(0, 0, 4, 6), st(1, 0, 0, 1)];
        let labels = vec![0, 1, 2, 1];
        let t = turns(&[w0.clone(), w1.clone()], &s, &labels);
        // Frame 3: voice 1's first stretch ends at frame 1, two frames back;
        // its second starts at frame 4, one frame on, so frame 3 takes label
        // 2. Voice 2's stretch is next to it (label 1).
        let f = |i: u64| 360 + 270 * i;
        assert_eq!(
            t,
            vec![
                Turn {
                    start: f(0),
                    end: f(2),
                    speaker: 0
                },
                Turn {
                    start: f(2),
                    end: f(4),
                    speaker: 1
                },
                Turn {
                    start: f(3),
                    end: f(6),
                    speaker: 2
                },
                Turn {
                    start: WINDOW + f(0),
                    end: WINDOW + f(1),
                    speaker: 1
                },
            ]
        );
        assert_eq!(
            speech(&[w0, w1]),
            vec![(f(0), f(6)), (f(7), f(8)), (WINDOW + f(0), WINDOW + f(2))]
        );
    }

    #[test]
    fn an_overlap_between_two_stretches_of_one_voice_goes_to_the_nearer() {
        // Voice 1 alone (frames 0-1), with voice 2 (frames 2-3), alone again
        // (frames 4-5), all one run of voice 1. Frame 2 is nearer the first
        // stretch, frame 3 the second. Voice 2 reaches no stretch: nobody.
        let w = window(0, &[0b001, 0b001, 0b011, 0b011, 0b001, 0b001]);
        let st = |first, after| Stretch {
            window: 0,
            voice: 0,
            first,
            after,
            start: 0,
            end: 0,
        };
        let f = |i: u64| 360 + 270 * i;
        assert_eq!(
            turns(&[w], &[st(0, 2), st(4, 6)], &[0, 1]),
            vec![
                Turn {
                    start: f(0),
                    end: f(3),
                    speaker: 0
                },
                Turn {
                    start: f(3),
                    end: f(6),
                    speaker: 1
                },
            ]
        );
    }

    #[test]
    fn overlaps() {
        assert_eq!(overlap(0, 10, 5, 20), 5);
        assert_eq!(overlap(0, 10, 10, 20), 0);
        assert_eq!(overlap(5, 6, 0, 20), 1);
    }
}
