"""The project's own browser interactions, run by check.py in every engine.

check.py has already loaded the page, waited for it to report ready, and let
it idle. This drives the tool the way a person would: it chooses a file that
is not a recording, then the test meeting, makes the transcript, reads it on
the page and saves it in both layouts.

Expected values are worked out by hand from tests/fixtures/meeting.truth.json
(C4), never copied from the tool's own output. The fixture is three voices
taking turns: Samantha, Daniel, Samantha, Daniel, Karen, Samantha, Karen,
Daniel, Samantha. So the transcript must name three speakers, Speaker 1 must
be Samantha (she speaks first), and every line must carry the number of the
voice that said it.
"""

import json
import pathlib

ROOT = pathlib.Path(__file__).resolve().parents[2]
STRINGS = json.loads((ROOT / "data" / "strings.json").read_text())
FIXTURE = ROOT / "tests" / "fixtures" / "meeting.mp4"
TRUTH = json.loads((ROOT / "tests" / "fixtures" / "meeting.truth.json").read_text())
# The slowest engine on a laptop takes about a minute; a CI machine may take
# several.
WORK_MS = 600_000


def seconds(stamp):
    h, m, s = stamp.split(":")
    return int(h) * 3600 + int(m) * 60 + float(s)


def voice_of(a, b):
    """The fixture voice that speaks most between a and b seconds."""
    return max(TRUTH, key=lambda u: min(b, u["end_ms"] / 1000) - max(a, u["start_ms"] / 1000))["voice"]


def cues_of(vtt):
    """(start, end, speaker label) for each cue of a WebVTT file."""
    out = []
    for block in vtt.split("\n\n")[1:]:
        lines = block.strip().split("\n")
        a, b = lines[1].split(" --> ")
        out.append((seconds(a), seconds(b), lines[2].split(": ", 1)[0]))
    return out


def run(page, check, engine):
    check(page.inner_text("#title") == STRINGS["title"], "the title is drawn")
    check(page.get_attribute("body", "data-screen") == "setup", "the first screen is the setup screen")
    make = page.get_by_role("button", name=STRINGS["make"])
    check(make.is_disabled(), "the make button waits for a file")

    # A file the browser cannot decode: the reason is shown, and the page
    # returns to the setup screen.
    page.set_input_files("#file", str(ROOT / "tests" / "fixtures" / "meeting.truth.json"))
    make.click()
    page.wait_for_function(
        "() => document.body.dataset.screen === 'setup' && document.getElementById('message').textContent !== ''",
        timeout=WORK_MS,
    )
    message = page.inner_text("#message")
    head = STRINGS["decode_failed"].split("{reason}")[0]
    check(message.startswith(head), f"a file with no sound is refused with a reason ({message!r})")

    page.set_input_files("#file", str(FIXTURE))
    check("meeting.mp4" in page.inner_text("#chosen"), "the chosen file is named")
    make.click()
    page.wait_for_selector("body[data-screen='working']", state="attached")
    check(page.locator("#progress").count() == 1, "a progress bar is shown while working")
    page.wait_for_selector("body[data-screen='done']", state="attached", timeout=WORK_MS)

    summary = page.inner_text("#summary")
    check(summary.startswith("Speakers: 3. "), f"three speakers are found ({summary!r})")

    heads = page.locator(".cue-head").all_inner_texts()
    texts = page.locator(".cues li > p:nth-child(2)").all_inner_texts()
    check(heads and heads[0].startswith("Speaker 1 "), "the first line on the page is Speaker 1")
    words = " ".join(texts).lower()
    for phrase in ["probability unit", "recursion", "friday", "reading list"]:
        check(phrase in words, f"the transcript has the words {phrase!r}")

    with page.expect_download() as d:
        page.get_by_role("button", name=STRINGS["save_vtt"]).click()
    vtt = d.value
    body = pathlib.Path(vtt.path()).read_text()
    check(vtt.suggested_filename == "meeting transcript.vtt", f"the VTT is named after the recording ({vtt.suggested_filename})")
    check(body.startswith("WEBVTT\n\n1\n00:00:0"), "the VTT starts like Zoom's")
    check("\nSpeaker 1: Good morning" in body, "VTT lines read 'Speaker n: text'")
    cues = cues_of(body)
    check(len(cues) == len(heads), f"the VTT holds the lines the page shows ({len(cues)}, {len(heads)})")
    # Each line's voice: the fixture voice it overlaps most.
    pairs = {(label, voice_of(a, b)) for a, b, label in cues}
    by_label = {}
    for label, voice in pairs:
        by_label.setdefault(label, set()).add(voice)
    one_voice_each = all(len(v) == 1 for v in by_label.values())
    distinct = len({next(iter(v)) for v in by_label.values()}) == len(by_label)
    check(one_voice_each and distinct, f"each speaker number is one voice and each voice one number {sorted(pairs)}")
    check(by_label.get("Speaker 1") == {"Samantha"}, "Speaker 1 is the voice that spoke first")
    with page.expect_download() as d:
        page.get_by_role("button", name=STRINGS["save_txt"]).click()
    txt = pathlib.Path(d.value.path()).read_text()
    check(txt.startswith("[Speaker 1] 00:00:0"), "the text file starts like Zoom's")

    stored = page.evaluate(
        "async () => (await caches.keys()).length + (indexedDB.databases ? (await indexedDB.databases()).length : 0)"
    )
    check(stored == 0, f"nothing is kept in the browser's storage ({stored})")

    page.get_by_role("button", name=STRINGS["again"]).click()
    check(page.get_attribute("body", "data-screen") == "setup", "the page can start again")
