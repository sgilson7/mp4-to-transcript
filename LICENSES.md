# Licences

Every file in this repository that is not code written for it: its source, its author, its terms. The code is under the repository's licence (`LICENSE`, MIT).

## Assets

| Path | Source | Terms |
| --- | --- | --- |
| `tests/fixtures/meeting.mp4` | spoken by macOS's built-in voices (Samantha, Daniel, Karen) from the script in `tests/fixtures/make_meeting.py` | made for this repository's tests; the voices' terms are a worklist row (`SECOND-ORDER.md` R1.11) |

## Vendored code and models

All under `web/vendor/`, pinned by `web/vendor/SHA256SUMS`, which `scripts/build.sh` checks before every build. Checksums below are the first 16 hex digits of SHA-256.

| Path | Project and version | Author | Licence | Checksum |
| --- | --- | --- | --- | --- |
| `transformers/transformers.js` | Transformers.js 4.3.0 (npm `@huggingface/transformers`, `dist/transformers.js`, unminified), which bundles ONNX Runtime Web 1.31.0-dev.20260914 | Hugging Face | Apache-2.0 (`transformers/LICENSE`) | `43f86bbcab7cb088` |
| `ort/ort-wasm-simd-threaded.asyncify.mjs`, `.wasm` | ONNX Runtime Web 1.31.0-dev.20260914-8d85527a0 (npm `onnxruntime-web`) | Microsoft | MIT (`ort/LICENSE`) | `49871f5a44095197` (wasm) |
| `models/whisper-base.en/` | Whisper base.en, ONNX export `onnx-community/whisper-base.en` at `51eefc0`, 8-bit encoder and merged decoder | OpenAI; export by Hugging Face | MIT (`models/whisper-base.en/LICENSE`) | `dd4761a3f7add26a` (decoder) |
| `models/pyannote-segmentation-3.0/` | pyannote segmentation 3.0, ONNX export `onnx-community/pyannote-segmentation-3.0` at `733a93b` | pyannote (Hervé Bredin and contributors) | MIT (stated in `models/pyannote-segmentation-3.0/README.md`) | `057ee564753071c0` |
| `models/wespeaker-voxceleb-resnet34-LM/` | WeSpeaker ResNet34-LM trained on VoxCeleb, ONNX export `onnx-community/wespeaker-voxceleb-resnet34-LM` at `6a61a18` | WeSpeaker (Hongji Wang, Chengdong Liang, Shuai Wang and others), packaged by pyannote | CC BY 4.0 (attribution: this row, and the page's README) | `3955447b0499dc9e` |

## Code taken from other repositories

| From | What | Licence |
| --- | --- | --- |
| `sgilson7/builder-setting` 0.2.0, `template/` | the project's scripts, tests, boundary checks and page skeleton | the Builder repository's licence |

A file with no licence, a likeness, a recording or any third-party picture is a worklist row in `SECOND-ORDER.md` for the person to decide, and stays out of git until they do.
