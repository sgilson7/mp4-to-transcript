#!/usr/bin/env python3
"""Make tests/fixtures/meeting.mp4: a short three-voice conversation spoken by
macOS's `say`, with pauses, packed as AAC in an MP4 by `afconvert`. Also writes
meeting.truth.json: who speaks when, in milliseconds, for the browser check.

    python3 tests/fixtures/make_meeting.py

Needs macOS (`say`, `afconvert`). The fixture is committed, so the check does
not need either.
"""
import json
import pathlib
import subprocess
import tempfile
import wave

HERE = pathlib.Path(__file__).resolve().parent
RATE = 16000
PAUSE_MS = 700

# (voice, line). The voices differ in sex and accent, as people in a meeting do.
# Karen was tried first: her voiceprints are as close to Samantha's as one
# person's often are to their own (SECOND-ORDER.md R1.7).
SCRIPT = [
    ("Samantha", "Good morning, everyone. Thanks for joining the planning meeting for the spring course."),
    ("Daniel", "Morning. I had a look at the syllabus draft last night, and I think the first two weeks are too crowded."),
    ("Samantha", "That's fair. What would you move?"),
    ("Daniel", "I would push the probability unit back by a week, so students have time with proofs first."),
    ("Tara", "I agree with that. The lab sections also need an extra session on recursion before the first exam."),
    ("Samantha", "Okay. Tara, can you draft the lab schedule, and Daniel, can you revise the reading list?"),
    ("Tara", "Yes, I can have the schedule ready by Friday."),
    ("Daniel", "And I will send the reading list on Monday."),
    ("Samantha", "Great. Let's meet again next week to review both."),
]


def speak(voice, text, path):
    subprocess.run(
        ["say", "-v", voice, "-o", str(path), "--file-format=WAVE", f"--data-format=LEI16@{RATE}", text],
        check=True,
    )


def main():
    truth = []
    frames = b""
    with tempfile.TemporaryDirectory() as tmp:
        for i, (voice, text) in enumerate(SCRIPT):
            part = pathlib.Path(tmp) / f"{i}.wav"
            speak(voice, text, part)
            with wave.open(str(part)) as w:
                assert w.getframerate() == RATE and w.getnchannels() == 1 and w.getsampwidth() == 2
                data = w.readframes(w.getnframes())
            frames += b"\0\0" * (RATE * PAUSE_MS // 1000)
            start = len(frames) // 2 * 1000 // RATE
            frames += data
            end = len(frames) // 2 * 1000 // RATE
            truth.append({"voice": voice, "start_ms": start, "end_ms": end, "text": text})
        frames += b"\0\0" * (RATE * PAUSE_MS // 1000)
        joined = pathlib.Path(tmp) / "meeting.wav"
        with wave.open(str(joined), "wb") as w:
            w.setnchannels(1)
            w.setsampwidth(2)
            w.setframerate(RATE)
            w.writeframes(frames)
        out = HERE / "meeting.mp4"
        subprocess.run(["afconvert", "-f", "mp4f", "-d", "aac", str(joined), str(out)], check=True)
    (HERE / "meeting.truth.json").write_text(json.dumps(truth, indent=2) + "\n")
    print(f"wrote {out} ({out.stat().st_size} bytes), {len(truth)} lines")


if __name__ == "__main__":
    main()
