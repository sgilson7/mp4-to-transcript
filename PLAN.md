# Plan

Stage 2, written by the agent from `BRIEF.md`. `B2 Require step-by-step justification`.

The brief approves the plan in advance, so this records the decisions as they were made; the person reads it at stage 3 (`C3 Interrogate the claim`) and may revise any of them.

| # | Decision | Reason | Test that shows it holds |
| --- | --- | --- | --- |
| 1 | Run the speech models in the browser, from files on the tool's own site. | The brief asks for a tool like the PDF redactor: no server, no upload. A static page may fetch only from its own origin (R7), so the models are vendored under `web/vendor/`, pinned by checksum. | `check.py`: no off-origin request at any point; `build.sh` refuses a vendored file that does not match `SHA256SUMS`. |
| 2 | Transformers.js 4.3.0 with ONNX Runtime Web, in one module worker. | The models take seconds per job; in the page's own thread they would freeze it. Transformers.js loads the three model families used here. | `check.py` allows exactly one worker, `worker.js`; no service worker. |
| 3 | Whisper base.en (8-bit), pyannote segmentation 3.0, WeSpeaker ResNet34. | Whisper base is the largest that runs at several times real speed in WebAssembly on one thread (`MEASUREMENTS.md`); the speaker models are the ones pyannote's own pipeline uses. About 107 MB in all. | `MEASUREMENTS.md` runs; the browser check's full transcript. |
| 4 | The core names every job (which samples, which model) and reads every answer. The worker only runs a model on the samples a job names. | Builder R1/R3: every rule in core, where `cargo test` reaches it. | `api.rs` tests drive a whole run with hand-made answers; `boundary.rs`. |
| 5 | Model outputs come to the core as raw 32-bit float bytes and are read as integers: an order key for comparing scores, fixed point for voiceprints. | Reference default D2: no floats in core. Both readings are exact integer operations. | `bits.rs` tests with hand-worked IEEE 754 patterns. |
| 6 | Speaker turns: every unbroken solo run of a second or more gets a voiceprint; runs of two seconds or more are grouped by average-linkage clustering on cosine; shorter runs join the closest group. | The segmentation model can give two similar voices one label within a window (`SECOND-ORDER.md` R1.6). Long runs give steady voiceprints. | `diarize.rs` tests; the browser check's three voices on nine lines. |
| 7 | Speech is read in 30-second blocks that start just before the next speech; a line cut off at a block's end is left for the next block. Lines over silence and bracketed sound tags are dropped. | Whisper's own long-form method; Whisper writes words into silence. | `transcribe.rs` tests. |
| 8 | Each line goes to the speaker heard most under it; consecutive lines from one speaker that split a sentence are joined; speakers are numbered by first speech. | Zoom's transcripts are one remark per cue, named by speaker. | `transcript.rs` tests. |
| 9 | Save as VTT (Zoom's cloud-recording layout) and as text (Zoom's saved-transcript layout). | The two layouts Zoom gives people. | `transcript.rs` tests; the browser check reads both saved files. |
| 10 | The reader may set the number of speakers (one to six) or let the page find out. | Clustering by threshold can split or merge similar voices; a known count removes that guess. | `api.rs` test with a known count. |
| 11 | The shim holds the core's `Handle`; the core keeps the run's state. | A run holds thousands of model answers, which are not worth passing back and forth as JSON. | `boundary.rs`: the shim still has no branch. |

## Open questions for the person

1. Is English-only right? `whisper-base` (multilingual) is the same size and could replace `whisper-base.en`, at some cost to English accuracy.
2. Is the test meeting's use of macOS voices acceptable in a public repository (`SECOND-ORDER.md` R1.11)?

## Status

- [ ] Read by the person on:
