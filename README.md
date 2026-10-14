# MP4 to Transcript

Turn a recording of a meeting, interview or lecture into a transcript laid out like Zoom's, with each voice given an anonymous number: Speaker 1, Speaker 2, and so on, in the order the voices first speak.

**Use it:** https://sgilson7.github.io/mp4-to-transcript/

The recording stays on your computer. The page reads it and runs the speech models inside your browser tab; it sends the recording to no server and keeps nothing once the tab is closed. The models (about 112 MB) are files on the same site, downloaded the first time you make a transcript.

## Using it

1. Choose a recording: an MP4 or MOV video, or M4A, MP3, WAV or WebM sound.
2. If you know how many people speak, choose the number. Otherwise leave **Find out** pressed.
3. Press **Make the transcript**. The page shows each stage, a progress bar and the time left.
4. Save the transcript as **VTT**, in the layout of a Zoom cloud recording's transcript, or as **text**, in the layout of a transcript saved from a Zoom meeting.

On an Apple M2 Max laptop, a 17.5-minute meeting took about five minutes (`MEASUREMENTS.md`); slower machines take longer. The page is checked in the engines of Chrome and Edge, Firefox, and Safari.

## What to expect

On a recorded four-person meeting from the AMI corpus, the page found four speakers and put 95% of the transcript's time under the right one. The speaker numbers come from how voices sound. Two people with similar voices may share a number, and one person may get two, most often when they speak only briefly. Choosing the number of speakers removes the page's guess about how many there are. Speech in which two people talk at once goes to whoever is heard most.

The speech model is Whisper base, English only. It writes clear English speech well and mishears names, jargon and crosstalk; read the transcript against the recording before quoting it.

## How it works

Three models run in a web worker, from files on this site:

| Model | Job |
| --- | --- |
| Whisper base.en (OpenAI), 8-bit | writes down what is said, thirty seconds at a time |
| pyannote segmentation 3.0 | finds where voices speak, overlap and change, ten seconds at a time |
| WeSpeaker ResNet34-LM | makes a voiceprint of each stretch of one voice |

Every decision is made in a Rust core compiled to WebAssembly: which samples each model reads next, which lines to keep, which voiceprints belong to one speaker (average-linkage clustering on cosine similarity), which speaker said each line, and how the files are laid out. The worker runs a model on the samples the core names and hands back the raw output. The core reads the models' 32-bit floats as integers, so a given recording and set of answers always gives the same transcript.

This is a [Builder](https://github.com/sgilson7/builder-setting) project: the core holds every rule and does no input or output, the shim only passes values across, and the page draws what the core describes.

## Build, test and serve

```bash
./scripts/setup.sh              # once: Python venv with Playwright and three browsers
cargo test --workspace          # the core, boundary, copy and storage tests, in seconds
./scripts/build.sh              # release WebAssembly build into dist/
python3 scripts/serve.py        # http://127.0.0.1:8000/
./scripts/check.sh              # everything, before publishing
python3 scripts/break_tests.py  # each row of tests/breaks.json must make its test fail
./scripts/count-tests.sh        # the test count to quote in a document
./scripts/publish.sh            # push dist/ to gh-pages, from a clean tree
```

Needs Rust (pinned in `rust-toolchain.toml`), `wasm-bindgen-cli` 0.2.127, and Python 3. The browser check makes three transcripts of `tests/fixtures/meeting.mp4`, one in each engine, and takes a few minutes.

## Layout

| Path | Holds |
| --- | --- |
| `crates/core` | every rule: `api.rs` (the door and the screens), `diarize.rs` (who speaks when), `transcribe.rs` (what is read next, which lines are kept), `transcript.rs` (speakers, numbering, Zoom's layouts), `bits.rs` (model output read as integers) |
| `crates/shim` | the `wasm-bindgen` methods the page calls; each is one call into the core |
| `web/` | the page (`app.js`), the model worker (`worker.js`), and `vendor/`: the models and their runtime, pinned by `SHA256SUMS` |
| `data/` | reader-visible text, the models' settings, presentation numbers |
| `GLOSSARY.md`, `TONE.md` | what each term means; how every string is written |
| `tests/fixtures/` | the test meeting and who speaks when in it |
| `tests/browser/` | the browser check and this tool's interactions |

## Licences

The code is MIT. The models and runtime keep their own licences, listed in `LICENSES.md`: Whisper and pyannote segmentation are MIT, ONNX Runtime is MIT, Transformers.js is Apache-2.0, and the WeSpeaker voiceprint model is CC BY 4.0 (WeSpeaker, Hongji Wang and others, packaged by pyannote).
