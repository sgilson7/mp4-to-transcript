//! The door into the core. The shim holds one `Handle` and forwards each call.
//!
//! The core describes the screen as a view model (`Ui`): its name, title,
//! panels with their fields, meters and actions, and the glossary's
//! definitions. The page builds the DOM from that description without knowing
//! what a panel means. Settings, screens and flags all come back through one
//! function, `act(name, value)`; an unknown action is an error. (After
//! Peter's Mandible.)
//!
//! The work itself runs as a series of jobs the core names one at a time:
//! read ten seconds for who is speaking (`segment`), take a voiceprint of one
//! stretch (`embed`), or write down thirty seconds of speech (`transcribe`).
//! The page hands each job to its worker, which runs the model on the samples
//! the job names and nothing else, and the answer comes back through
//! `result`. Every choice about what to read next, and what the answers mean,
//! is made here.
//!
//! The template keeps its state as JSON the page hands back on each call. This
//! tool holds a recording's worth of model answers, so the state lives in
//! `Handle`, which the shim holds (`SECOND-ORDER.md` R1.3).

use serde::{Deserialize, Serialize};

use crate::diarize::{self, Stretch, Turn, WindowFrames, RATE};
use crate::glossary;
use crate::page::{self, fill, Strings};
use crate::transcribe::{self, Line as Said, PER_MS};
use crate::transcript::{self, Cue};

fn ok<T: Serialize>(value: &T) -> String {
    serde_json::json!({ "ok": value }).to_string()
}

fn error(message: String) -> String {
    serde_json::json!({ "error": message }).to_string()
}

/// The most speakers a reader can name in advance.
pub const MOST_SPEAKERS: usize = 6;
/// Estimated cost of each job, in milliseconds of a laptop's time, from the
/// test meeting in Chromium (`MEASUREMENTS.md`). Only their ratios matter:
/// the time left is the time so far scaled by the cost left over the cost done.
pub const SEGMENT_COST: u64 = 70;
pub const EMBED_COST: u64 = 450;
/// Transcription cost per sample read: about 5 s for a 30 s block.
pub const SAMPLES_PER_COST: u64 = 96;
/// Expected voiceprints per window, in tenths, before the window is read.
pub const STRETCHES_PER_WINDOW_TENTHS: u64 = 15;
/// No time left is shown until this much cost has been measured.
pub const SETTLED_COST: u64 = 2_000;
/// The progress meter's full value.
pub const METER_MAX: u64 = 1_000;
const MB: u64 = 1_000_000;

/// One button: the action name and value the page sends back through `act`.
/// `pressed` makes it a toggle the page marks with `aria-pressed`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Action {
    pub name: String,
    pub value: String,
    pub label: String,
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pressed: Option<bool>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub primary: bool,
}

impl Action {
    fn new(name: &str, value: &str, label: &str) -> Action {
        Action {
            name: name.into(),
            value: value.into(),
            label: label.into(),
            enabled: true,
            pressed: None,
            primary: false,
        }
    }
}

/// A file picker. The page sends `act(name, {"name", "size"})` when a file is
/// chosen, and keeps the file itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Field {
    pub name: String,
    pub label: String,
    pub kind: String,
    pub accept: String,
}

/// A bar for how far the work has gone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Meter {
    pub id: String,
    pub label: String,
    pub max: u64,
    pub value: u64,
}

/// One line of text. A line with an id can be found by the browser check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Line {
    #[serde(skip_serializing_if = "String::is_empty")]
    pub id: String,
    pub text: String,
}

impl Line {
    fn plain(text: String) -> Line {
        Line {
            id: String::new(),
            text,
        }
    }
    fn named(id: &str, text: String) -> Line {
        Line { id: id.into(), text }
    }
}

/// One line of the finished transcript as the page shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CueView {
    pub at: String,
    pub speaker: String,
    pub text: String,
}

/// One block of the screen. Empty fields are left out of the JSON.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Panel {
    pub id: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub heading: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub lines: Vec<Line>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<Field>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub meters: Vec<Meter>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<Action>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub cues: Vec<CueView>,
}

/// A term and its definition, for the page's hover hints.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Hint {
    pub term: String,
    pub definition: String,
}

/// A job for the page's worker: run one model on samples `[start, end)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Job {
    pub kind: &'static str,
    pub start: u64,
    pub end: u64,
}

/// Everything the page draws, and what it should do after drawing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Ui {
    pub screen: String,
    pub title: String,
    pub panels: Vec<Panel>,
    pub hints: Vec<Hint>,
    /// A sentence for the page's message line; empty for none.
    pub message: String,
    /// What the page carries out after drawing, in order: `worker.start`,
    /// `audio.decode`, `audio.send`, `job`, `worker.stop`, `file.save`,
    /// `input.clear`.
    pub effects: Vec<String>,
    /// The job `job` sends.
    pub job: Option<Job>,
    /// The sample rate `audio.decode` converts the recording to.
    pub rate: u64,
    /// The name and type `file.save` saves under.
    pub file_name: String,
    pub file_type: String,
}

/// What the page can do, reported once at start.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct Abilities {
    /// The browser offers module workers.
    pub worker: bool,
    /// The browser offers `OfflineAudioContext`.
    pub audio: bool,
}

#[derive(Deserialize)]
struct Chosen {
    name: String,
    size: u64,
}

/// A loading report from transformers.js, passed on as it comes. Reports
/// without a byte count (a file starting or finishing) count for nothing.
#[derive(Deserialize)]
struct Loading {
    #[serde(default)]
    name: String,
    #[serde(default)]
    file: String,
    #[serde(default)]
    loaded: u64,
}

#[derive(Deserialize)]
struct Decoded {
    samples: u64,
    now: u64,
}

/// What the page reports with a job's answer. `bytes` travel beside it.
#[derive(Deserialize)]
struct Answer {
    kind: String,
    now: u64,
    #[serde(default)]
    frames: usize,
    #[serde(default)]
    dims: usize,
    #[serde(default)]
    text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Screen {
    Setup,
    Browser,
    Working,
    Done,
}

/// One run from "Make the transcript" to the finished transcript.
#[derive(Debug, Clone, Default)]
struct Run {
    loaded: Vec<(String, u64)>,
    ready: bool,
    samples: Option<u64>,
    windows: Vec<(u64, u64)>,
    frames: Vec<WindowFrames>,
    stretches: Vec<Stretch>,
    prints: Vec<Vec<i64>>,
    grouped: bool,
    turns: Vec<Turn>,
    speech: Vec<(u64, u64)>,
    seek: u64,
    said: Vec<Said>,
    pending: Option<Job>,
    started: Option<u64>,
    now: u64,
    cost_done: u64,
}

/// The core's state for one page.
pub struct Handle {
    strings: Strings,
    hints: Vec<Hint>,
    /// Each model file the worker loads, and its size in bytes.
    files: Vec<(String, u64)>,
    screen: Screen,
    chosen: Option<(String, u64)>,
    target: Option<usize>,
    run: Option<Run>,
    cues: Vec<Cue>,
    length: u64,
    effects: Vec<String>,
    message: String,
    saved: String,
    save_name: String,
    save_type: String,
    broken: Option<String>,
}

fn hints_from(glossary_md: &str) -> Result<Vec<Hint>, String> {
    let entries = glossary::parse(glossary_md).map_err(|e| e.message())?;
    Ok(entries
        .into_iter()
        .map(|e| Hint {
            term: e.term,
            definition: e.definition,
        })
        .collect())
}

/// `12.3`-style megabytes, to one decimal place, rounded half up.
fn megabytes(bytes: u64) -> String {
    let tenths = (bytes + MB / 20) / (MB / 10);
    format!("{}.{}", tenths / 10, tenths % 10)
}

/// A file name for the transcript: the recording's name without its
/// extension, with characters file systems refuse removed.
pub fn stem(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or("");
    let base = match base.rfind('.') {
        Some(at) if at > 0 => &base[..at],
        _ => base,
    };
    base.chars()
        .filter(|c| !matches!(c, ':' | '*' | '?' | '"' | '<' | '>' | '|') && !c.is_control())
        .collect::<String>()
        .trim()
        .to_string()
}

impl Run {
    fn total(&self) -> u64 {
        self.samples.unwrap_or(0)
    }

    fn segmented(&self) -> bool {
        self.samples.is_some() && self.frames.len() == self.windows.len()
    }

    /// The cost of the work not yet done, as best it can be told.
    fn cost_left(&self) -> u64 {
        let windows_left = (self.windows.len() - self.frames.len()) as u64;
        let prints_left = (self.stretches.len() - self.prints.len()) as u64
            + windows_left * STRETCHES_PER_WINDOW_TENTHS / 10;
        let speech_left: u64 = if self.segmented() {
            self.speech
                .iter()
                .map(|(a, b)| b.saturating_sub((*a).max(self.seek)))
                .sum()
        } else {
            self.total()
        };
        windows_left * SEGMENT_COST + prints_left * EMBED_COST + speech_left / SAMPLES_PER_COST
    }

    fn cost_of(job: &Job) -> u64 {
        match job.kind {
            "segment" => SEGMENT_COST,
            "embed" => EMBED_COST,
            _ => (job.end - job.start) / SAMPLES_PER_COST,
        }
    }

    /// Milliseconds left, once enough work has been timed.
    fn left(&self) -> Option<u64> {
        let started = self.started?;
        let elapsed = self.now.saturating_sub(started);
        if self.cost_done < SETTLED_COST || elapsed == 0 {
            return None;
        }
        Some(elapsed * self.cost_left() / self.cost_done)
    }
}

impl Handle {
    /// Reads the strings, the glossary, the model files and what the browser
    /// can do. A file that cannot be read leaves the handle broken: every call
    /// then returns the reason as an error.
    pub fn new(strings_json: &str, glossary_md: &str, files_json: &str, abilities_json: &str) -> Handle {
        let mut broken = None;
        let strings = page::parse_strings(strings_json).map_err(|e| e.message());
        let hints = hints_from(glossary_md);
        let files: Result<serde_json::Map<String, serde_json::Value>, String> =
            serde_json::from_str(files_json)
                .map_err(|e| format!("The model file list could not be read: {e}"));
        let abilities: Result<Abilities, String> = serde_json::from_str(abilities_json)
            .map_err(|e| format!("The browser's abilities could not be read: {e}"));
        for e in [
            strings.as_ref().err(),
            hints.as_ref().err(),
            files.as_ref().err(),
            abilities.as_ref().err(),
        ]
        .into_iter()
        .flatten()
        {
            broken.get_or_insert(e.clone());
        }
        let abilities = abilities.unwrap_or_default();
        Handle {
            strings: strings.unwrap_or_default(),
            hints: hints.unwrap_or_default(),
            files: files
                .unwrap_or_default()
                .into_iter()
                .map(|(k, v)| (k, v.as_u64().unwrap_or(0)))
                .collect(),
            screen: if abilities.worker && abilities.audio {
                Screen::Setup
            } else {
                Screen::Browser
            },
            chosen: None,
            target: None,
            run: None,
            cues: Vec::new(),
            length: 0,
            effects: Vec::new(),
            message: String::new(),
            saved: String::new(),
            save_name: String::new(),
            save_type: String::new(),
            broken,
        }
    }

    fn label(&self) -> impl Fn(usize) -> String {
        let template = self.strings.speaker_label.clone();
        move |n| fill(&template, &[("n", (n + 1).to_string())])
    }

    fn total_bytes(&self) -> u64 {
        self.files.iter().map(|(_, b)| b).sum()
    }

    /// The screen as it stands.
    pub fn ui(&self) -> String {
        match &self.broken {
            Some(e) => error(e.clone()),
            None => ok(&self.view()),
        }
    }

    fn view(&self) -> Ui {
        let s = &self.strings;
        let (screen, panels) = match self.screen {
            Screen::Browser => (
                "browser",
                vec![Panel {
                    id: "browser".into(),
                    heading: s.browser_heading.clone(),
                    lines: vec![Line::plain(s.browser_cannot.clone())],
                    ..Panel::default()
                }],
            ),
            Screen::Setup => ("setup", self.setup_panels()),
            Screen::Working => ("working", self.working_panels()),
            Screen::Done => ("done", self.done_panels()),
        };
        let job = self.run.as_ref().and_then(|r| r.pending);
        Ui {
            screen: screen.into(),
            title: s.title.clone(),
            panels,
            hints: self.hints.clone(),
            message: self.message.clone(),
            effects: self.effects.clone(),
            job,
            rate: RATE,
            file_name: self.save_name.clone(),
            file_type: self.save_type.clone(),
        }
    }

    fn setup_panels(&self) -> Vec<Panel> {
        let s = &self.strings;
        let mut recording = Panel {
            id: "recording".into(),
            fields: vec![Field {
                name: "file.chosen".into(),
                label: s.file_label.clone(),
                kind: "file".into(),
                accept: s.file_accept.clone(),
            }],
            ..Panel::default()
        };
        if let Some((name, size)) = &self.chosen {
            let size = fill(&s.size_mb, &[("mb", megabytes(*size))]);
            recording.lines.push(Line::named(
                "chosen",
                fill(&s.file_chosen, &[("name", name.clone()), ("size", size)]),
            ));
        }
        let mut choices = vec![Action {
            pressed: Some(self.target.is_none()),
            ..Action::new("speakers.set", "auto", &s.speakers_auto)
        }];
        for n in 1..=MOST_SPEAKERS {
            choices.push(Action {
                pressed: Some(self.target == Some(n)),
                ..Action::new("speakers.set", &n.to_string(), &n.to_string())
            });
        }
        vec![
            Panel {
                id: "intro".into(),
                lines: vec![Line::plain(s.intro.clone()), Line::plain(s.privacy.clone())],
                ..Panel::default()
            },
            recording,
            Panel {
                id: "speakers".into(),
                heading: s.speakers_heading.clone(),
                lines: vec![Line::plain(s.speakers_note.clone())],
                actions: choices,
                ..Panel::default()
            },
            Panel {
                id: "go".into(),
                lines: vec![
                    Line::plain(fill(
                        &s.download_note,
                        &[("mb", (self.total_bytes() / MB).to_string())],
                    )),
                    Line::plain(s.ids_note.clone()),
                ],
                actions: vec![Action {
                    enabled: self.chosen.is_some(),
                    primary: true,
                    ..Action::new("work.start", "", &s.make)
                }],
                ..Panel::default()
            },
        ]
    }

    fn working_panels(&self) -> Vec<Panel> {
        let s = &self.strings;
        let Some(run) = &self.run else {
            return Vec::new();
        };
        let name = self.chosen.as_ref().map(|c| c.0.clone()).unwrap_or_default();
        let (heading, text, value) = if !run.ready {
            let loaded: u64 = run.loaded.iter().map(|(_, b)| b).sum();
            let total = self.total_bytes().max(1);
            (
                &s.loading_heading,
                fill(
                    &s.loading_text,
                    &[
                        ("loaded", (loaded / MB).to_string()),
                        ("total", (total / MB).to_string()),
                    ],
                ),
                loaded.min(total) * METER_MAX / total,
            )
        } else if run.samples.is_none() {
            (&s.reading_heading, fill(&s.reading_text, &[("name", name)]), 0)
        } else {
            let done = run.cost_done;
            let value = done * METER_MAX / (done + run.cost_left()).max(1);
            if !run.segmented() || !run.grouped {
                let parts = run.windows.len() + run.stretches.len();
                let parts_done = run.frames.len() + run.prints.len();
                (
                    &s.voices_heading,
                    fill(
                        &s.voices_text,
                        &[("done", parts_done.to_string()), ("total", parts.to_string())],
                    ),
                    value,
                )
            } else {
                let total = run.total();
                (
                    &s.words_heading,
                    fill(
                        &s.words_text,
                        &[
                            ("done", transcript::duration(run.seek / PER_MS)),
                            ("total", transcript::duration(total / PER_MS)),
                        ],
                    ),
                    value,
                )
            }
        };
        let left = match run.left() {
            Some(ms) => fill(&s.time_left, &[("left", transcript::duration(ms))]),
            None => s.time_left_unknown.clone(),
        };
        vec![Panel {
            id: "work".into(),
            heading: heading.clone(),
            lines: vec![Line::named("stage", text), Line::named("left", left)],
            meters: vec![Meter {
                id: "progress".into(),
                label: s.progress_label.clone(),
                max: METER_MAX,
                value,
            }],
            actions: vec![Action::new("work.stop", "", &s.stop)],
            ..Panel::default()
        }]
    }

    fn done_panels(&self) -> Vec<Panel> {
        let s = &self.strings;
        let label = self.label();
        vec![
            Panel {
                id: "done".into(),
                heading: s.done_heading.clone(),
                lines: vec![
                    Line::named(
                        "summary",
                        fill(
                            &s.done_text,
                            &[
                                ("speakers", transcript::speakers(&self.cues).to_string()),
                                ("lines", self.cues.len().to_string()),
                                ("length", transcript::duration(self.length / PER_MS)),
                            ],
                        ),
                    ),
                    Line::plain(s.ids_note.clone()),
                ],
                actions: vec![
                    Action {
                        primary: true,
                        ..Action::new("file.save", "vtt", &s.save_vtt)
                    },
                    Action::new("file.save", "txt", &s.save_txt),
                    Action::new("again", "", &s.again),
                ],
                ..Panel::default()
            },
            Panel {
                id: "transcript".into(),
                cues: self
                    .cues
                    .iter()
                    .map(|c| CueView {
                        at: transcript::clock(c.start),
                        speaker: label(c.speaker),
                        text: c.text.clone(),
                    })
                    .collect(),
                ..Panel::default()
            },
        ]
    }

    /// Applies one named action.
    pub fn act(&mut self, name: &str, value: &str) -> String {
        if let Some(e) = &self.broken {
            return error(e.clone());
        }
        self.effects.clear();
        match self.apply(name, value) {
            Ok(()) => ok(&self.view()),
            Err(e) => error(e),
        }
    }

    fn apply(&mut self, name: &str, value: &str) -> Result<(), String> {
        let s = self.strings.clone();
        let working = self.screen == Screen::Working;
        match name {
            "file.chosen" if self.screen == Screen::Setup => {
                let c: Option<Chosen> = serde_json::from_str(value)
                    .map_err(|e| format!("The chosen file could not be read: {e}"))?;
                self.message.clear();
                self.chosen = None;
                match c {
                    // The picker was closed with nothing chosen.
                    None => {}
                    Some(c) if c.size == 0 => {
                        self.message = s.empty_file.clone();
                        self.effects.push("input.clear".into());
                    }
                    Some(c) => self.chosen = Some((c.name, c.size)),
                }
            }
            "speakers.set" if self.screen == Screen::Setup => {
                self.target = match value {
                    "auto" => None,
                    n => match n.parse::<usize>() {
                        Ok(n) if (1..=MOST_SPEAKERS).contains(&n) => Some(n),
                        _ => return Err(format!("There is no speaker count \"{n}\".")),
                    },
                };
            }
            "work.start" if self.screen == Screen::Setup => {
                if self.chosen.is_none() {
                    self.message = s.no_file.clone();
                    return Ok(());
                }
                self.message.clear();
                self.screen = Screen::Working;
                self.run = Some(Run::default());
                self.effects.push("worker.start".into());
                self.effects.push("audio.decode".into());
            }
            "models.progress" if working => {
                let p: Loading = serde_json::from_str(value)
                    .map_err(|e| format!("The loading report could not be read: {e}"))?;
                let key = format!("{}/{}", p.name, p.file);
                let size = self.files.iter().find(|(k, _)| *k == key).map(|(_, b)| *b);
                if let (Some(size), Some(run)) = (size, self.run.as_mut()) {
                    let loaded = p.loaded.min(size);
                    match run.loaded.iter_mut().find(|(k, _)| *k == key) {
                        Some(entry) => entry.1 = entry.1.max(loaded),
                        None => run.loaded.push((key, loaded)),
                    }
                }
            }
            "models.ready" if working => {
                let now = value
                    .parse::<u64>()
                    .map_err(|_| format!("\"{value}\" is not a time."))?;
                if let Some(run) = self.run.as_mut() {
                    run.ready = true;
                    run.now = now;
                }
                self.begin();
            }
            "audio.decoded" if working => {
                let d: Decoded = serde_json::from_str(value)
                    .map_err(|e| format!("The decoded sound could not be read: {e}"))?;
                if let Some(run) = self.run.as_mut() {
                    run.samples = Some(d.samples);
                    run.windows = diarize::windows(d.samples);
                    run.now = d.now;
                }
                self.begin();
            }
            "models.failed" | "audio.failed" | "job.failed" if working => {
                let template = match name {
                    "models.failed" => &s.models_failed,
                    "audio.failed" => &s.decode_failed,
                    _ => &s.job_failed,
                };
                self.halt(fill(template, &[("reason", value.to_string())]));
            }
            "work.stop" if working => self.halt(s.stopped.clone()),
            "file.save" if self.screen == Screen::Done => {
                let label = self.label();
                let (text, kind, extension) = match value {
                    "vtt" => (transcript::vtt(&self.cues, &label), "text/vtt", "vtt"),
                    "txt" => (transcript::txt(&self.cues, &label), "text/plain", "txt"),
                    other => return Err(format!("There is no file type \"{other}\".")),
                };
                let name = self.chosen.as_ref().map(|c| stem(&c.0)).unwrap_or_default();
                let name = if name.is_empty() {
                    s.default_name.clone()
                } else {
                    name
                };
                self.saved = text;
                self.save_name = format!("{}.{extension}", fill(&s.file_name, &[("name", name)]));
                self.save_type = format!("{kind};charset=utf-8");
                self.effects.push("file.save".into());
            }
            "again" if self.screen == Screen::Done => {
                self.screen = Screen::Setup;
                self.chosen = None;
                self.cues.clear();
                self.message.clear();
                self.effects.push("input.clear".into());
            }
            "file.chosen" | "speakers.set" | "work.start" | "models.progress" | "models.ready"
            | "audio.decoded" | "models.failed" | "audio.failed" | "job.failed" | "work.stop"
            | "file.save" | "again" => {
                // A known action at the wrong moment, such as a late report
                // from a worker that was stopped: nothing to do.
            }
            other => return Err(format!("Unknown action \"{other}\".")),
        }
        Ok(())
    }

    /// Ends the run and returns to the first screen with `message`.
    fn halt(&mut self, message: String) {
        self.run = None;
        self.screen = Screen::Setup;
        self.message = message;
        self.effects.push("worker.stop".into());
    }

    /// Once the models are loaded and the sound decoded, sends the sound and
    /// the first job.
    fn begin(&mut self) {
        let Some(run) = self.run.as_mut() else {
            return;
        };
        if !run.ready || run.samples.is_none() || run.started.is_some() {
            return;
        }
        run.started = Some(run.now);
        self.effects.push("audio.send".into());
        self.next();
    }

    /// Chooses the next job, or finishes.
    fn next(&mut self) {
        let target = self.target;
        let Some(run) = self.run.as_mut() else {
            return;
        };
        let total = run.total();
        let job = if run.frames.len() < run.windows.len() {
            let (start, end) = run.windows[run.frames.len()];
            Some(Job {
                kind: "segment",
                start,
                end,
            })
        } else if run.prints.len() < run.stretches.len() {
            let s = run.stretches[run.prints.len()];
            Some(Job {
                kind: "embed",
                start: s.start,
                end: s.end,
            })
        } else {
            if !run.grouped {
                let lengths: Vec<u64> = run.stretches.iter().map(|s| s.end - s.start).collect();
                let labels = diarize::cluster(&run.prints, &lengths, target);
                run.turns = diarize::turns(&run.frames, &run.stretches, &labels);
                run.speech = diarize::speech(&run.frames);
                run.grouped = true;
            }
            transcribe::next_block(run.seek, total, &run.speech).map(|(start, end)| Job {
                kind: "transcribe",
                start,
                end,
            })
        };
        run.pending = job;
        match job {
            Some(_) => self.effects.push("job".into()),
            None => self.finish(),
        }
    }

    fn finish(&mut self) {
        let Some(run) = self.run.take() else {
            return;
        };
        self.cues = transcript::build(&run.said, &run.turns, &run.speech);
        self.length = run.total();
        self.effects.push("worker.stop".into());
        if self.cues.is_empty() {
            self.screen = Screen::Setup;
            self.message = self.strings.no_speech.clone();
        } else {
            self.screen = Screen::Done;
            self.message.clear();
        }
    }

    /// Takes the answer to the pending job: a JSON report and, for the speaker
    /// models, the model's output as little-endian 32-bit floats.
    pub fn result(&mut self, report: &str, bytes: &[u8]) -> String {
        if let Some(e) = &self.broken {
            return error(e.clone());
        }
        self.effects.clear();
        match self.take(report, bytes) {
            Ok(()) => ok(&self.view()),
            Err(e) => {
                self.halt(fill(&self.strings.job_failed.clone(), &[("reason", e)]));
                ok(&self.view())
            }
        }
    }

    fn take(&mut self, report: &str, bytes: &[u8]) -> Result<(), String> {
        let a: Answer =
            serde_json::from_str(report).map_err(|e| format!("The report could not be read: {e}"))?;
        let Some(run) = self.run.as_mut() else {
            // A late answer from a stopped run.
            return Ok(());
        };
        let Some(job) = run.pending.take() else {
            return Err("An answer came with no job waiting.".into());
        };
        if job.kind != a.kind {
            return Err(format!("A {} answer came for a {} job.", a.kind, job.kind));
        }
        match job.kind {
            "segment" => {
                let w = diarize::read_frames(job.start, job.end, a.frames, bytes).map_err(|e| e.message())?;
                let index = run.frames.len();
                run.stretches.extend(diarize::stretches(index, &w));
                run.frames.push(w);
            }
            "embed" => {
                run.prints
                    .push(diarize::read_voiceprint(bytes, a.dims).map_err(|e| e.message())?);
            }
            _ => {
                let settled =
                    transcribe::settle(job.start, job.end, run.total(), &a.text).map_err(|e| e.message())?;
                run.said.extend(settled.lines);
                // The reading always moves on, whatever the model said.
                run.seek = settled.seek.max(job.start + 1);
            }
        }
        run.cost_done += Run::cost_of(&job);
        run.now = a.now;
        self.next();
        Ok(())
    }

    /// The text of the last file asked for.
    pub fn file(&self) -> String {
        self.saved.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diarize::{CLASSES, WINDOW};

    fn strings() -> String {
        std::str::from_utf8(include_bytes!("../../../data/strings.json"))
            .unwrap()
            .to_string()
    }

    const GLOSSARY: &str = "### Speaker\n\nA voice.\n";
    const FILES: &str = r#"{"m/a.onnx": 3000000, "m/b.onnx": 1000000}"#;
    const ABLE: &str = r#"{"worker": true, "audio": true}"#;

    fn json(text: &str) -> serde_json::Value {
        serde_json::from_str(text).unwrap()
    }

    fn handle() -> Handle {
        Handle::new(&strings(), GLOSSARY, FILES, ABLE)
    }

    fn le(words: &[u32]) -> Vec<u8> {
        words.iter().flat_map(|w| w.to_le_bytes()).collect()
    }

    /// Segmentation scores for `frames` frames: voice 1 alone in the first
    /// `ones`, voice 2 alone in the rest.
    fn scores(frames: usize, ones: usize) -> Vec<u8> {
        let (hi, lo) = (0x4000_0000u32, 0xBF80_0000u32);
        let mut w = Vec::new();
        for i in 0..frames {
            let class = if i < ones { 1 } else { 2 };
            w.extend((0..CLASSES).map(|k| if k == class { hi } else { lo }));
        }
        le(&w)
    }

    fn started(h: &mut Handle, samples: u64) -> serde_json::Value {
        h.act("file.chosen", r#"{"name": "Team meeting.mp4", "size": 2500000}"#);
        h.act("work.start", "");
        h.act("models.ready", "1000");
        json(&h.act(
            "audio.decoded",
            &format!(r#"{{"samples": {samples}, "now": 1000}}"#),
        ))
    }

    #[test]
    fn the_first_screen_offers_a_file_and_the_speaker_count() {
        let v = json(&handle().ui());
        assert_eq!(v["ok"]["screen"], "setup");
        assert_eq!(v["ok"]["title"], "MP4 to Transcript");
        assert_eq!(v["ok"]["panels"][1]["fields"][0]["kind"], "file");
        let choices = &v["ok"]["panels"][2]["actions"];
        assert_eq!(choices[0]["pressed"], true);
        assert_eq!(choices[6]["label"], "6");
        // 4,000,000 bytes of models is 4 MB.
        assert_eq!(
            v["ok"]["panels"][3]["lines"][0]["text"],
            "On first use the page downloads its speech models from this site, about 4 MB."
        );
        assert_eq!(v["ok"]["panels"][3]["actions"][0]["enabled"], false);
        assert_eq!(v["ok"]["rate"], 16000);
    }

    #[test]
    fn a_browser_without_workers_is_told_what_it_lacks() {
        let h = Handle::new(&strings(), GLOSSARY, FILES, r#"{"worker": false, "audio": true}"#);
        assert_eq!(json(&h.ui())["ok"]["screen"], "browser");
    }

    #[test]
    fn a_broken_file_is_reported_on_every_call() {
        let mut h = Handle::new(&strings(), GLOSSARY, "not json", ABLE);
        assert!(json(&h.ui())["error"]
            .as_str()
            .unwrap()
            .starts_with("The model file list could not be read"));
        assert!(json(&h.act("again", "")).get("error").is_some());
    }

    #[test]
    fn choosing_a_file_shows_its_name_and_size_and_enables_the_button() {
        let mut h = handle();
        let v = json(&h.act("file.chosen", r#"{"name": "Team meeting.mp4", "size": 2450000}"#));
        assert_eq!(
            v["ok"]["panels"][1]["lines"][0]["text"],
            "Team meeting.mp4, 2.5 MB"
        );
        assert_eq!(v["ok"]["panels"][3]["actions"][0]["enabled"], true);
        let v = json(&h.act("file.chosen", r#"{"name": "x.mp4", "size": 0}"#));
        assert_eq!(
            v["ok"]["message"],
            "That file is empty. Choose the recording again."
        );
        assert_eq!(v["ok"]["effects"], serde_json::json!(["input.clear"]));
        assert_eq!(v["ok"]["panels"][3]["actions"][0]["enabled"], false);
        h.act("file.chosen", r#"{"name": "a.mp4", "size": 9}"#);
        let v = json(&h.act("file.chosen", "null"));
        assert_eq!(v["ok"]["message"], "");
        assert_eq!(v["ok"]["panels"][3]["actions"][0]["enabled"], false);
        // A report with no byte count is accepted and counts for nothing.
        h.act("file.chosen", r#"{"name": "a.mp4", "size": 9}"#);
        h.act("work.start", "");
        let v = json(&h.act(
            "models.progress",
            r#"{"status": "initiate", "name": "m", "file": "a.onnx"}"#,
        ));
        assert_eq!(v["ok"]["panels"][0]["meters"][0]["value"], 0);
    }

    #[test]
    fn the_speaker_count_is_one_to_six_or_found_out() {
        let mut h = handle();
        let v = json(&h.act("speakers.set", "3"));
        assert_eq!(v["ok"]["panels"][2]["actions"][3]["pressed"], true);
        assert_eq!(v["ok"]["panels"][2]["actions"][0]["pressed"], false);
        assert_eq!(
            json(&h.act("speakers.set", "7"))["error"],
            "There is no speaker count \"7\"."
        );
        assert_eq!(
            json(&h.act("speakers.set", "0"))["error"],
            "There is no speaker count \"0\"."
        );
        let v = json(&h.act("speakers.set", "auto"));
        assert_eq!(v["ok"]["panels"][2]["actions"][0]["pressed"], true);
    }

    #[test]
    fn starting_loads_the_models_and_decodes_the_sound() {
        let mut h = handle();
        h.act("file.chosen", r#"{"name": "a.mp4", "size": 10}"#);
        let v = json(&h.act("work.start", ""));
        assert_eq!(v["ok"]["screen"], "working");
        assert_eq!(
            v["ok"]["effects"],
            serde_json::json!(["worker.start", "audio.decode"])
        );
        let v = json(&h.act(
            "models.progress",
            r#"{"name": "m", "file": "a.onnx", "loaded": 1500000, "total": 3000000}"#,
        ));
        assert_eq!(v["ok"]["panels"][0]["lines"][0]["text"], "1 MB of 4 MB.");
        // 1,500,000 of 4,000,000 bytes: 375 of 1000.
        assert_eq!(v["ok"]["panels"][0]["meters"][0]["value"], 375);
        // Progress for a file not in the list is ignored.
        let v = json(&h.act(
            "models.progress",
            r#"{"name": "z", "file": "a.onnx", "loaded": 9, "total": 9}"#,
        ));
        assert_eq!(v["ok"]["panels"][0]["meters"][0]["value"], 375);
    }

    #[test]
    fn starting_without_a_file_says_so() {
        let mut h = handle();
        let v = json(&h.act("work.start", ""));
        assert_eq!(v["ok"]["screen"], "setup");
        assert_eq!(v["ok"]["message"], "Choose a recording first.");
    }

    #[test]
    fn the_first_job_waits_for_both_the_models_and_the_sound() {
        let mut h = handle();
        h.act("file.chosen", r#"{"name": "a.mp4", "size": 10}"#);
        h.act("work.start", "");
        let v = json(&h.act("audio.decoded", r#"{"samples": 350000, "now": 5}"#));
        assert_eq!(v["ok"]["effects"], serde_json::json!([]));
        assert!(v["ok"]["job"].is_null());
        let v = json(&h.act("models.ready", "9"));
        assert_eq!(v["ok"]["effects"], serde_json::json!(["audio.send", "job"]));
        assert_eq!(
            v["ok"]["job"],
            serde_json::json!({"kind": "segment", "start": 0, "end": 160000})
        );
    }

    #[test]
    fn a_whole_run_reads_windows_then_voiceprints_then_speech() {
        let mut h = handle();
        // 20 s: two windows.
        let v = started(&mut h, 2 * WINDOW);
        assert_eq!(v["ok"]["job"]["kind"], "segment");
        // Window 1: voice 1 alone for 300 frames, voice 2 alone for 289.
        let v = json(&h.result(
            r#"{"kind": "segment", "frames": 589, "now": 1100}"#,
            &scores(589, 300),
        ));
        assert_eq!(
            v["ok"]["job"],
            serde_json::json!({"kind": "segment", "start": 160000, "end": 320000})
        );
        let v = json(&h.result(
            r#"{"kind": "segment", "frames": 589, "now": 1200}"#,
            &scores(589, 589),
        ));
        // Two stretches in window 1 and one in window 2.
        assert_eq!(
            v["ok"]["job"],
            serde_json::json!({"kind": "embed", "start": 360, "end": 360 + 300 * 270})
        );
        assert_eq!(v["ok"]["panels"][0]["lines"][0]["text"], "Part 2 of 5.");
        // Voiceprints: window 1 voice 1 and window 2 voice 1 point the same way.
        let x = le(&[0x3F80_0000, 0]);
        let y = le(&[0, 0x3F80_0000]);
        h.result(r#"{"kind": "embed", "dims": 2, "now": 1300}"#, &x);
        h.result(r#"{"kind": "embed", "dims": 2, "now": 1400}"#, &y);
        let v = json(&h.result(r#"{"kind": "embed", "dims": 2, "now": 1500}"#, &x));
        // Speech starts at frame 0 (sample 360), so the block starts at 0.
        assert_eq!(
            v["ok"]["job"],
            serde_json::json!({"kind": "transcribe", "start": 0, "end": 320000})
        );
        assert_eq!(v["ok"]["panels"][0]["heading"], "Writing down what is said");
        // The block is the whole recording, so it is the last.
        let answer = serde_json::json!({
            "kind": "transcribe",
            "now": 9000,
            "text": r#"{"chunks":[{"timestamp":[0,5],"text":" Hello there."},{"timestamp":[5.5,9.5],"text":" Hi."},{"timestamp":[10,19],"text":" Back to me."}]}"#
        });
        let v = json(&h.result(&answer.to_string(), &[]));
        assert_eq!(v["ok"]["screen"], "done");
        assert_eq!(v["ok"]["effects"], serde_json::json!(["worker.stop"]));
        assert_eq!(
            v["ok"]["panels"][0]["lines"][0]["text"],
            "Speakers: 2. Lines: 3. Length: 0:20."
        );
        let cues = &v["ok"]["panels"][1]["cues"];
        assert_eq!(
            cues[0],
            serde_json::json!({"at": "00:00:00", "speaker": "Speaker 1", "text": "Hello there."})
        );
        assert_eq!(cues[1]["speaker"], "Speaker 2");
        assert_eq!(cues[2]["speaker"], "Speaker 1");
        let v = json(&h.act("file.save", "vtt"));
        assert_eq!(v["ok"]["file_name"], "Team meeting transcript.vtt");
        assert_eq!(v["ok"]["file_type"], "text/vtt;charset=utf-8");
        assert!(h
            .file()
            .starts_with("WEBVTT\n\n1\n00:00:00.000 --> 00:00:05.000\nSpeaker 1: Hello there.\n"));
        let v = json(&h.act("file.save", "txt"));
        assert_eq!(v["ok"]["file_name"], "Team meeting transcript.txt");
        assert!(h
            .file()
            .starts_with("[Speaker 1] 00:00:00\nHello there.\n\n[Speaker 2] 00:00:05\nHi.\n"));
    }

    #[test]
    fn a_known_speaker_count_overrides_the_threshold() {
        let mut h = handle();
        h.act("speakers.set", "1");
        started(&mut h, WINDOW);
        h.result(
            r#"{"kind": "segment", "frames": 589, "now": 1}"#,
            &scores(589, 300),
        );
        h.result(
            r#"{"kind": "embed", "dims": 2, "now": 2}"#,
            &le(&[0x3F80_0000, 0]),
        );
        h.result(
            r#"{"kind": "embed", "dims": 2, "now": 3}"#,
            &le(&[0, 0x3F80_0000]),
        );
        let answer = serde_json::json!({"kind": "transcribe", "now": 4,
            "text": r#"{"chunks":[{"timestamp":[0,4],"text":"A."},{"timestamp":[6,9],"text":"B."}]}"#});
        let v = json(&h.result(&answer.to_string(), &[]));
        assert_eq!(
            v["ok"]["panels"][0]["lines"][0]["text"],
            "Speakers: 1. Lines: 2. Length: 0:10."
        );
    }

    #[test]
    fn a_silent_recording_returns_to_the_start_with_a_reason() {
        let mut h = handle();
        started(&mut h, WINDOW);
        let silent = le(&(0..589 * CLASSES)
            .map(|k| if k % CLASSES == 0 { 0x4000_0000 } else { 0 })
            .collect::<Vec<_>>());
        let v = json(&h.result(r#"{"kind": "segment", "frames": 589, "now": 1}"#, &silent));
        assert_eq!(v["ok"]["screen"], "setup");
        assert_eq!(
            v["ok"]["message"],
            "No speech was heard in this recording. Check that it has sound, and choose it again."
        );
        assert_eq!(v["ok"]["effects"], serde_json::json!(["worker.stop"]));
    }

    #[test]
    fn a_wrong_answer_stops_the_run_and_says_why() {
        let mut h = handle();
        started(&mut h, WINDOW);
        let v = json(&h.result(r#"{"kind": "embed", "dims": 2, "now": 1}"#, &[]));
        assert_eq!(v["ok"]["screen"], "setup");
        assert_eq!(
            v["ok"]["message"],
            "The transcriber stopped (A embed answer came for a segment job.). Reload the page and try again."
        );
        let v = json(&h.result(r#"{"kind": "segment", "frames": 2, "now": 1}"#, &[]));
        assert_eq!(v["ok"]["screen"], "setup");
    }

    #[test]
    fn stopping_ends_the_worker_and_a_late_answer_is_ignored() {
        let mut h = handle();
        started(&mut h, WINDOW);
        let v = json(&h.act("work.stop", ""));
        assert_eq!(v["ok"]["screen"], "setup");
        assert_eq!(v["ok"]["message"], "Stopped. Nothing was kept.");
        assert_eq!(v["ok"]["effects"], serde_json::json!(["worker.stop"]));
        let v = json(&h.result(r#"{"kind": "segment", "frames": 589, "now": 1}"#, &scores(589, 1)));
        assert_eq!(v["ok"]["screen"], "setup");
        let v = json(&h.act("models.ready", "5"));
        assert_eq!(v["ok"]["effects"], serde_json::json!([]));
    }

    #[test]
    fn a_failure_names_its_reason() {
        let mut h = handle();
        h.act("file.chosen", r#"{"name": "a.mp4", "size": 10}"#);
        h.act("work.start", "");
        let v = json(&h.act("audio.failed", "EncodingError"));
        assert_eq!(
            v["ok"]["message"],
            "The browser could not read the sound in this file (EncodingError). Try an MP4, M4A, MP3 or WAV file."
        );
    }

    #[test]
    fn an_unknown_action_is_an_error() {
        assert_eq!(
            json(&handle().act("screen.jump", ""))["error"],
            "Unknown action \"screen.jump\"."
        );
    }

    #[test]
    fn the_time_left_scales_the_time_so_far() {
        let mut run = Run {
            samples: Some(10 * WINDOW),
            windows: diarize::windows(10 * WINDOW),
            started: Some(0),
            now: 4_000,
            cost_done: 2_000,
            ..Run::default()
        };
        // Nothing segmented yet: 10 windows * 70, 15 voiceprints * 450, and
        // 1,600,000 samples / 96 = 16,666. Total 700 + 6,750 + 16,666.
        assert_eq!(run.cost_left(), 24_116);
        // 4 s for 2,000 cost: 2 ms each, so 48,232 ms.
        assert_eq!(run.left(), Some(48_232));
        run.cost_done = 1_999;
        assert_eq!(run.left(), None);
    }

    #[test]
    fn file_names_keep_the_recording_name() {
        assert_eq!(stem("Team meeting.mp4"), "Team meeting");
        assert_eq!(stem("a.b.mp4"), "a.b");
        assert_eq!(stem(".hidden"), ".hidden");
        assert_eq!(stem("C:\\x\\what?.mp4"), "what");
        assert_eq!(stem("noext"), "noext");
        assert_eq!(megabytes(2_450_000), "2.5");
        assert_eq!(megabytes(49_999), "0.0");
        assert_eq!(megabytes(50_000), "0.1");
    }
}
