# Glossary

The source of truth for what each word in this tool means. Strings, tests, the plan and the README use a term in the sense given here. The page shows a definition when a reader hovers the term.

Write each entry as a `### Term` heading, then one or two plain sentences, then an optional `Not:` line listing words that must not be used for it. A number a rule decides is a `{placeholder}` the core fills, so the glossary cannot disagree with the rules. Keep entries in alphabetical order. `crates/core/tests/copy.rs` checks the format, the placeholders and the `Not:` words; `TONE.md` governs the prose.

<!--
An example entry. It sits inside this comment, so it does not count.

### Spout

Where slush comes out. Each line has up to {line.spouts} spouts.

Not: tap, dispenser, nozzle
-->

### Speaker

A voice in the recording, numbered from one in the order the voices first speak. The number comes from how the voice sounds, not from who the person is, so it is anonymous.

Not: participant, attendee, user

### Speech model

A program trained on recorded speech that the page runs inside the browser. One writes down the words, one finds where voices speak and change, and one makes voiceprints.

Not: AI, engine, service

### Transcript

The words said in the recording, line by line, each line with the time it starts and the speaker who said it.

Not: captions, subtitles, minutes

### Voiceprint

A list of numbers the speech model makes from a stretch of one voice. Stretches from the same voice give voiceprints that point the same way, which is how the page tells speakers apart.

Not: fingerprint, identity

### VTT

WebVTT, the timed-text file format Zoom uses for the transcript of a cloud recording. Each line has a number, a start and end time, and the text, which here begins with the speaker.

Not: subtitle file, SRT
