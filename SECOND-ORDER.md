# Second-order notebook

Stage 6, kept by the agent as the work happens, not afterwards. `B3 Build in self-questioning`.

One row per assumption, surprise or open item. **Kind** is *divergence* (what was assumed, what turned out to be true, what changed), *finding*, or *worklist* (the person's to do). Number rows by round (R1.1, R1.2, then R2.1 after the first triage) so a later round does not renumber an earlier one. End each row with its status: *done*, *open*, or *person*.

| # | Kind | Row |
| --- | --- | --- |
| R1.1 | finding | A static page may fetch only from its own origin (R7), so the three models and the ONNX runtime are vendored under `web/vendor/` (134 MB, one file of 54 MB) and pinned by `web/vendor/SHA256SUMS`. GitHub refuses files over 100 MB, so the largest model that fits whole is Whisper base; `small` (8-bit decoder about 250 MB) would need splitting. *done* |
| R1.2 | finding | Transformers.js by default fetches models from huggingface.co, the ONNX runtime from cdn.jsdelivr.net, and keeps both in the browser's Cache Storage. All three are turned off in `data/models.json` (`allowRemoteModels: false`, local `wasmPaths`, `useBrowserCache` and `useWasmCache: false`). The browser check shows no off-origin request and no cache or database after a full run, in three engines. The vendored bundle still holds those addresses as text; the page never reaches them on the path the check drives. *done* |
| R1.3 | divergence | The template keeps the page's state as JSON passed back on each call. A run holds every window's frames and every voiceprint, so the state lives in `api::Handle`, which the shim holds in a `wasm_bindgen` struct, as YouTube to MP3 does. The shim still has no branch, loop or `?`. *done* |
| R1.4 | divergence | The template's browser check fails on any worker. The models take seconds per job and would freeze the page's thread, so `check.py` allows exactly one worker, `worker.js` on the page's origin; any other worker, or a service worker, still fails. Firefox reports the worker's address relative to the page, which first failed the check; the check now resolves it. A planted second worker (`other.js`), a foreign `worker.js` and `vendor/worker.js` were shown to be refused by the filter. `build.sh` stamps `new Worker("...")` URLs and fails on an unstamped one; each page's CSP adds `worker-src 'self'`. Recorded in `BUILDER-SPEC.md`. *done* |
| R1.5 | finding | ONNX Runtime refused a relative `wasmPaths` ("Failed to resolve module specifier"): it imports the runtime from inside `vendor/transformers/`. The worker builds the address from its own location (`new URL(options.wasm, self.location)`), so no address is written in the page's code. *done* |
| R1.6 | divergence | First design: one voiceprint per local voice per window, from that voice's longest solo run, trusting the segmentation model's voice labels within a window. On the test meeting a later turn by a second woman was named as the first: the segmentation model had given both voices one label in that window. Changed: every unbroken solo run of a second or more gets its own voiceprint; a pause ends a run. Then a 0.7 s scrap of one voice inherited a label across a pause from a different person with the same local label; changed again so a frame inherits only from a run it reaches without a pause. *done* |
| R1.7 | divergence | The threshold for "same speaker" was first set to 0.52 from the test meeting, whose voices Samantha and Karen (both macOS voices) average 0.48 against each other. On a recorded meeting (AMI ES2004a, four people, 17.5 min, CC BY 4.0, not in the repository) 0.52 gave 9 speakers. Replaying its 181 voiceprints: real people stay apart down to 0.25, and one person splits apart from about 0.3. Set to 0.35, and a group with under 5% of the anchor time and under 30 s joins the group it is most like, as pyannote's minimum cluster size does. The macOS voices were measured (a table of 14 voices); Karen was replaced by Tara (0.04 against Samantha, 0.08 against Daniel). A reversion averted: the synthetic test case would have tuned the tool away from real speech. *done* |
| R1.8 | finding | The loading meter needs each model file's size. `build.sh` writes `dist/data/model-files.json` from the files themselves; no size is typed. *done* |
| R1.9 | finding | The page runs ONNX Runtime on one thread: shared memory needs cross-origin isolation headers, which GitHub Pages cannot send. On an Apple M2 Max in headless Chromium, the 17.5-minute AMI meeting took 293 s (`MEASUREMENTS.md`). WebGPU could be several times faster but needs the 16- or 32-bit weights (about 150 MB more); not done. *open* |
| R1.10 | finding | A check that miscounted: `interactions.py` first named each line's voice by its start time plus half a second. Whisper starts lines early (Daniel's line at 5.76 s, his voice at 6.01 s), so the check blamed the tool for its own reading. The check now reads start and end from the saved VTT and takes the voice heard most between them. *done* |
| R1.11 | worklist | The test meeting is spoken by macOS's built-in voices (Samantha, Daniel, Tara). Apple's licence for macOS covers using them for one's own content; whether a recording of them may sit in a public repository as a test file is the person's call. If not, `tests/fixtures/make_meeting.py` can regenerate it on any Mac and the `.mp4` can be dropped from git. *person* |
| R1.12 | worklist | The speech model is Whisper base.en, English only. `whisper-base` (multilingual, same size) would serve other languages at some cost to English. *person* |
| R1.13 | finding | Two checks that could not fail, found by `break_tests.py`: the solo-run test's overlap run (5 frames) was too short to become a stretch either way; and after the small-group rule, the "short runs make no group" break was absorbed by it. Both tests were changed so their breaks are caught (29 of 29). *done* |
| R1.14 | worklist | Try the page by hand with a real recording you know, ideally a Zoom recording with Zoom's own transcript: compare who-said-what line by line. No automated check uses real speech; the AMI run was a one-off probe. *person* |
| R1.15 | finding | The browser decodes the whole file before anything runs. An hour at 16 kHz is 230 MB of samples, plus the file itself and the decoder's copy at its own rate. Recordings of several hours may fail on a machine with little memory; the page then shows the browser's error. Not measured past 17.5 minutes. *open* |
| R1.16 | finding | Whisper writes lines with timestamps a few tenths of a second out and sometimes splits a sentence across a speaker's pause. Two rules answer it: a speaker must be heard under a fifth of a line to be its speaker, and a line with no speaker under it that finishes the previous line's sentence goes with it. On AMI ES2004a, 94.8% of the transcript's time carries the right speaker, after the best one-to-one matching of numbers to people. *done* |
| R1.17 | finding | Lines over silence are dropped, and the speech model is not run on stretches where no voice is heard, because Whisper writes words such as "Thank you." into silence. Speech the segmentation model misses is therefore never transcribed. *done* |
| R1.18 | finding | `page.rs` strings, `models.json` and `page.json` are read as data; `copy.rs` reads every string in them against `TONE.md` and the glossary's `Not:` lines. Model names and settings in `models.json` are not reader-visible but are scanned too; none trips a rule. *done* |
| R1.19 | finding | GitHub's push protection refused the first push: it read the minified `transformers.min.js` as holding a Mistral API key. The match is model class names (`MistralForCausalLM` and others) beside a 32-character identifier in the npm release, not a key. Rather than override the scanner, the unminified `dist/transformers.js` from the same package and version is vendored instead (1.3 MB instead of 0.6 MB; easier to audit). `SHA256SUMS` and `LICENSES.md` updated. *done* |
| R1.20 | finding | The person allowed the push-protection false positive (R1.19) on 2026-10-14. Published from `6904409` (build `01ccb73a5e6c`); the browser check against https://sgilson7.github.io/mp4-to-transcript passed in Chromium, Firefox and WebKit, each making the test meeting's transcript (3 speakers, 9 lines) with no off-origin request. *done* |

## Strings written by the agent

Each reader-visible string the agent wrote, with its key, for the person to read. All are in `data/strings.json`.

| Key | String |
| --- | --- |
| title | MP4 to Transcript |
| intro | Turn a recording of a meeting, interview or lecture into a transcript laid out like Zoom's. Each voice gets a number instead of a name: Speaker 1, Speaker 2, and so on. |
| privacy | The recording stays on this computer. The page reads and transcribes it inside this browser tab and sends it to no server. |
| file_label | Recording (MP4, M4A, MP3, WAV or WebM) |
| speakers_heading | Number of speakers |
| speakers_auto | Find out |
| speakers_note | If you know how many people speak, choose the number. Otherwise the page decides from how alike the voices sound. |
| make | Make the transcript |
| download_note | On first use the page downloads its speech models from this site, about {mb} MB. |
| ids_note | Numbers go to voices in the order they first speak. They come from how the voices sound, so two people with similar voices may share a number, and one person may get two. |
| browser_heading | This browser cannot run the transcriber |
| browser_cannot | The transcriber needs WebAssembly, web workers and the Web Audio API. Current versions of Chrome, Edge, Firefox and Safari on a computer have all three. |
| loading_heading, loading_text | Loading the speech models / {loaded} MB of {total} MB. |
| reading_heading, reading_text | Reading the recording / Taking the sound out of {name}. |
| voices_heading, voices_text | Finding who speaks / Part {done} of {total}. |
| words_heading, words_text | Writing down what is said / Up to {done} of {total}. |
| time_left, time_left_unknown | About {left} left. / Working out the time left. |
| done_heading, done_text | Transcript ready / Speakers: {speakers}. Lines: {lines}. Length: {length}. |
| save_vtt, save_txt, again | Save as VTT / Save as text / Transcribe another recording |
| speaker_label | Speaker {n} |
| file_name | {name} transcript |
| empty_file | That file is empty. Choose the recording again. |
| no_file | Choose a recording first. |
| no_speech | No speech was heard in this recording. Check that it has sound, and choose it again. |
| decode_failed | The browser could not read the sound in this file ({reason}). Try an MP4, M4A, MP3 or WAV file. |
| models_failed | The speech models did not load ({reason}). Reload the page and try again. |
| job_failed | The transcriber stopped ({reason}). Reload the page and try again. |
| stopped | Stopped. Nothing was kept. |
