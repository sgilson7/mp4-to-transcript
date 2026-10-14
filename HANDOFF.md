# Handoff

Stage 9, written by the agent. `C5 Consolidate in your own words`. A fresh session starts from this file, not from memory.

## What this is

MP4 to Transcript turns a recording (MP4, MOV, M4A, MP3, WAV, WebM) into a transcript in Zoom's layouts, with voices numbered anonymously (Speaker 1, Speaker 2, ...) in the order they first speak. Three speech models run in the reader's browser from files on the same site; nothing is uploaded. Repository `sgilson7/mp4-to-transcript`, live at https://sgilson7.github.io/mp4-to-transcript/, listed under research tools on sgilson7.github.io.

## Rules that hold it up

| Rule | Guarded by |
| --- | --- |
| The worker runs a model on the samples a job names and decides nothing; the core names every job and reads every answer. | `api.rs` tests drive whole runs with hand-made answers; `boundary.rs` scans `web/` (vendor aside) for numbers and arithmetic |
| No off-origin request, no browser storage: Transformers.js's remote models, CDN runtime and caches are switched off in `data/models.json`. | `check.py` (no foreign request, three engines); `interactions.py` (no Cache Storage or IndexedDB after a run); `no_storage.rs` |
| Vendored files match `web/vendor/SHA256SUMS`. | `build.sh` refuses to build otherwise |
| One worker only, `worker.js`. | `check.py` |
| Model floats are read as integers. | `bits.rs` tests; `boundary.rs` bans floats in core |
| Grouping: anchors (runs of 2 s or more) by average linkage at cosine 0.35; tiny groups (under 5% and under 30 s) join the most alike; short runs join the most alike. | `diarize.rs` tests; breaks in `tests/breaks.json` |

## Shape of the code

| Path | Holds |
| --- | --- |
| `crates/core/src/api.rs` | screens, actions, the job sequence, time left |
| `crates/core/src/diarize.rs` | windows, frames, stretches, voiceprints, clustering, turns |
| `crates/core/src/transcribe.rs` | 30 s blocks, Whisper's answers, which lines are kept |
| `crates/core/src/transcript.rs` | speakers per line, joining, numbering, VTT and text |
| `web/app.js`, `web/worker.js` | the page's renderer and effects; the model runner |
| `data/models.json` | the models' settings, passed to Transformers.js unchanged |
| `tests/fixtures/` | the 44 s three-voice test meeting and its truth |

## Commands

`cargo test --workspace` (91 tests, under a second); `./scripts/check.sh` (prints `BUILDER CHECK PASSED`; the browser step makes three transcripts and takes a few minutes); `python3 scripts/break_tests.py` (29 of 29); `./scripts/publish.sh` (from a clean tree); `BUILDER_ORIGIN=https://sgilson7.github.io/mp4-to-transcript .venv/bin/python tests/browser/check.py` for the live page.

## What will bite within the hour

- Tuning the threshold on the synthetic test meeting. It was done once and was wrong for real speech (R1.7). Tune on recorded speech; the test meeting's voices were chosen to be as far apart as real people are.
- Firefox names the worker by a relative address (R1.4).
- A model file over 100 MB cannot be pushed to GitHub; Whisper small and larger need splitting (R1.1).
- `web/vendor/` is not scanned by `boundary.rs`; a change there must update `SHA256SUMS` and `LICENSES.md`.

## Mistakes so far

- Diarization: trusted the segmentation model's labels within a window (R1.6); tuned on synthetic voices (R1.7). Rule: one voiceprint per unbroken solo run, and tune on recorded speech.
- Checks: a speaker check that read only start times (R1.10); two tests a break could not fail (R1.13). Rule: run `break_tests.py` after adding a rule.

## What passed

`./scripts/check.sh` on 2026-10-14: `BROWSER CHECK PASSED in chromium, firefox, webkit`, `BUILDER CHECK PASSED`. `./scripts/count-tests.sh`: 91 passed, 0 failed, 0 ignored.

## The next action

The person makes a transcript of a recording they know and compares it with what was said (R1.14), then answers the two open questions in `PLAN.md`.
