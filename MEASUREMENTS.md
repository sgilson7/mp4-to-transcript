# Measurements

Stage 4, recorded by the agent before building and again at handoff. `B2 Require step-by-step justification`.

The state measured rather than recalled, so later changes can be compared with it. A probe prints numbers and asserts loose bounds at most; record its output here with the command that produced it.

| What | Command | Result | When |
| --- | --- | --- | --- |
| Toolchain | `rustc --version; wasm-bindgen --version` | rustc 1.99.0, wasm-bindgen 0.2.127 | 2026-10-14 |
| Machine | `sysctl -n machdep.cpu.brand_string` | Apple M2 Max | 2026-10-14 |
| Core tests | `./scripts/count-tests.sh` | 91 passed, 0 failed, 0 ignored | 2026-10-14 |
| Breaks caught | `python3 scripts/break_tests.py` | 29 of 29 | 2026-10-14 |
| WebAssembly size | printed by `./scripts/build.sh` | 389 KB (core and shim) | 2026-10-14 |
| Vendored files | `du -sh web/vendor` | 135 MiB on disk: models 107 MiB (112 MB, the figure the page shows, from `model-files.json`), ONNX Runtime 27 MiB, Transformers.js 1.3 MB (unminified, R1.19) | 2026-10-14 |
| Spike: each model alone, test meeting (44 s) | headless Chromium, one thread | Whisper 9.8 s for 44 s in 30 s blocks; segmentation 0.34 s for 5 windows; voiceprints 3.9 s for 9 utterances | 2026-10-14 |
| Recorded meeting, AMI ES2004a (17:29, four people), first build | probe in headless Chromium, nothing else running | 293 s (3.6 times faster than real time); 9 speakers found; 91.4% of transcript time on the right speaker | 2026-10-14 |
| Same, after R1.7 and R1.16 | same probe | 294 s; 4 speakers found; 94.8% of transcript time on the right speaker (162 of 186 lines) | 2026-10-14 |
| Full check | `./scripts/check.sh` | see `HANDOFF.md` | 2026-10-14 |

The AMI score: each line's reference speaker is the one heard most during it in the corpus's word-aligned reference (pyannote's `AMI-diarization-setup`, `only_words`); the tool's numbers are matched one-to-one to people to maximise agreeing time. It is not a diarization error rate, which counts every 10 ms.

## Not measured

- A real Zoom recording, and any recording longer than 17.5 minutes (memory, R1.15).
- Safari and Firefox speed; the browser check runs them on the 44-second meeting only.
- Windows, Linux, and machines slower than an M2 Max.
- Languages other than English, accents beyond the AMI speakers and the test voices, children's voices, and noisy rooms.
- Meetings with more than four people.
