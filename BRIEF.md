# Brief

Stage 1. The person's own words come first; the rest can be written by the agent from them and read by the person. `C1 Frame the problem and set the rules`, `B1 Establish context`.

## The description, word for word

> Use sgilson7.github.io builder setting, which can be found one directory up. I approve the creation of a public github repo, and a public github pages to host the tool. I approve deployment to the website in order to update the tools section.  Use the shared technical setting within to create the following tool for me: an mp4 to zoom style transcript, that assigns the speakers in the transcript anonymous ids

## The tool

A static web page that turns a recording (an MP4 video, or M4A, MP3, WAV or WebM sound) into a transcript laid out like the ones Zoom saves, with each voice given an anonymous number: Speaker 1, Speaker 2, and so on, in the order they first speak. The page reads the recording and runs the speech models inside the reader's browser tab, like the PDF redactor. There is no server, no upload, no account and no request off the page's own origin: the models are files on the same site.

Three models do the listening. Whisper (base, English) writes down what is said. Pyannote's segmentation model finds where voices speak and where they change. WeSpeaker's ResNet34 model gives each stretch of one voice a voiceprint, and the core groups voiceprints that are alike into speakers. The reader may say how many people speak; otherwise the core decides from how alike the voices are.

The transcript can be saved as a WebVTT file in Zoom's cloud-recording layout (numbered cues, `Speaker 1: text`) or as text in Zoom's saved-transcript layout (`[Speaker 1] 00:01:02`, then the text).

For researchers and teachers who need a transcript of an interview, a meeting or a class without sending the recording to a transcription service, on a desktop or laptop browser.

## Done means

- The test meeting (`tests/fixtures/meeting.mp4`, three voices taking nine turns) gives a transcript naming three speakers, with every line carrying the number of the voice that said it and Speaker 1 the voice that spoke first.
- The transcript's words include phrases from each part of the meeting.
- The VTT starts `WEBVTT`, numbers its cues, and writes each as `Speaker n: text`, as Zoom's does; the text file writes `[Speaker n] HH:MM:SS` lines, as Zoom's does. Both are named after the recording.
- A file the browser cannot decode gives a message that says so and what to try, not an empty transcript.
- A recording with no speech gives a message that says no speech was heard.
- Choosing the number of speakers gives that many (or fewer, if fewer voices are heard).
- While it works, the page shows the stage, a progress bar and the time left.
- No request leaves the page's origin, and nothing is kept in the browser's storage.
- The browser check passes in Chromium, Firefox and WebKit.
- `./scripts/check.sh` passes.

## Decisions that stay with the person

- Every reader-visible string (listed at the foot of `SECOND-ORDER.md`).
- The choice of models, and the English-only speech model (`SECOND-ORDER.md`).
- The test meeting's voices, made with macOS's built-in speech (`SECOND-ORDER.md`).
- What the website's tools section says about the tool.

## Accessibility

Every state is text: the screen's heading names the stage, the progress bar has a label, the time left is written out, and errors are sentences in a live region. The pressed speaker count is marked by a tick and a heavier border as well as colour. Every control is a native button or file input, so it works with a keyboard and a screen reader. The transcript on the page is a numbered list.

## Style

`default`.

## Words

Speaker, transcript, VTT, voiceprint, speech model. Entries in `GLOSSARY.md`.

## Autonomy

The plan is approved in advance. Where this brief is silent, the agent decides, records the decision in `SECOND-ORDER.md`, and continues. The agent may not loosen a Builder requirement or a boundary test, and may not add a server, an account, analytics or any off-origin request. The person wrote: "I approve the creation of a public github repo, and a public github pages to host the tool. I approve deployment to the website in order to update the tools section." The agent may create the public repository `sgilson7/mp4-to-transcript`, publish it to GitHub Pages with `./scripts/publish.sh` once `./scripts/check.sh` passes, and list it in the tools section of `sgilson7.github.io` and deploy that site once its own checks pass, until the person withdraws this.

## Allowed core dependencies

`serde`, `serde_json` (reference default). The models and their runtime are not core dependencies: they are vendored files under `web/vendor/`, pinned by `web/vendor/SHA256SUMS` and listed in `LICENSES.md`.
