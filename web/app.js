// The page draws what the core returns and decides nothing.
//
// The core describes the screen (its panels, fields, meters, actions and the
// glossary's definitions), and this file builds the DOM from that
// description without knowing what any panel means. Every button and field
// sends its action name and value back through `act`, the core's one door.
// After drawing, the page carries out the effects the core names: start or
// stop the worker, decode the recording, send it the sound, send it a job,
// save a file. (After Peter's Mandible.)
//
// The worker runs the speech models on the samples each job names. Its
// messages are either an action for `act` or a job's answer for `result`.
import init, { Tool } from "./pkg/tool_shim.js";

const el = (id) => document.getElementById(id);
const make = (tag, text) => {
  const node = document.createElement(tag);
  if (text !== undefined) node.textContent = text;
  return node;
};

let tool = null;
let ui = null;
let models = null;
let options = null;
let worker = null;
let file = null;
let sound = null;
let picker = null;
const drawn = {};

function send(name, value) {
  draw(JSON.parse(tool.act(name, value)));
}

// A label whose text is a glossary term shows the definition on hover.
function mark(node) {
  const hint = ui.hints.find((h) => h.term === node.textContent);
  if (hint) {
    node.dataset.term = hint.term;
    node.title = hint.definition;
  }
}

// The file picker keeps its node across redraws, so the chosen file stays.
function field(f) {
  if (!picker) {
    picker = make("input");
    picker.id = "file";
    picker.type = f.kind;
    picker.addEventListener("change", () => {
      file = picker.files[0] || null;
      send(f.name, JSON.stringify(file && { name: file.name, size: file.size }));
    });
  }
  picker.accept = f.accept;
  const label = make("label", f.label);
  label.htmlFor = picker.id;
  const row = make("div");
  row.className = "field";
  row.append(label, picker);
  return row;
}

function meter(m) {
  const label = make("label", m.label);
  label.htmlFor = m.id;
  const bar = make("progress");
  bar.id = m.id;
  bar.max = m.max;
  bar.value = m.value;
  const row = make("div");
  row.className = "meter";
  row.append(label, bar);
  return row;
}

function cue(c) {
  const item = make("li");
  const head = make("p");
  head.className = "cue-head";
  head.append(make("strong", c.speaker), " ", make("time", c.at));
  item.append(head, make("p", c.text));
  return item;
}

function panel(p) {
  const box = make("section");
  box.id = p.id;
  if (p.heading) box.append(make("h2", p.heading));
  (p.lines || []).forEach((l) => {
    const node = make("p", l.text);
    if (l.id) node.id = l.id;
    box.append(node);
  });
  (p.fields || []).forEach((f) => box.append(field(f)));
  (p.meters || []).forEach((m) => box.append(meter(m)));
  if (p.actions) {
    const row = make("div");
    row.className = "actions";
    p.actions.forEach((a) => {
      const button = make("button", a.label);
      button.type = "button";
      button.disabled = !a.enabled;
      if (a.pressed !== undefined) button.setAttribute("aria-pressed", String(a.pressed));
      if (a.primary) button.className = "primary";
      button.addEventListener("click", () => send(a.name, a.value));
      row.append(button);
    });
    box.append(row);
  }
  if (p.cues) {
    const list = make("ol");
    list.className = "cues";
    p.cues.forEach((c) => list.append(cue(c)));
    box.append(list);
  }
  return box;
}

// Only a panel whose description changed is rebuilt.
function drawPanels(panels) {
  const active = document.activeElement;
  const focused = active && active.id;
  const nodes = panels.map((p) => {
    const text = JSON.stringify(p);
    const old = el(p.id);
    if (old && drawn[p.id] === text) return old;
    drawn[p.id] = text;
    return panel(p);
  });
  el("panels").replaceChildren(...nodes);
  const again = focused && el(focused);
  if (again && again !== document.activeElement) again.focus();
  document.querySelectorAll("#panels h2, #panels button").forEach(mark);
}

function draw(result) {
  if (result.error) {
    el("message").textContent = result.error;
    return;
  }
  ui = result.ok;
  document.title = ui.title;
  el("title").textContent = ui.title;
  drawPanels(ui.panels);
  el("message").textContent = ui.message;
  document.body.dataset.screen = ui.screen;
  ui.effects.forEach((name) => effects[name]());
}

function hear(e) {
  const m = e.data;
  if (m.act !== undefined) {
    send(m.act, m.value);
  } else {
    draw(JSON.parse(tool.result(m.report, m.bytes)));
  }
}

function start() {
  worker = new Worker("worker.js", { type: "module" });
  worker.addEventListener("message", hear);
  worker.addEventListener("error", (e) => send("job.failed", String(e.message)));
  worker.postMessage({ type: "load", options: models });
}

function stop() {
  if (worker) worker.terminate();
  worker = null;
  sound = null;
}

// The browser decodes the file and converts it to one channel at the
// core's rate; the samples stay here until the core asks for them to be sent.
async function decode() {
  try {
    const data = await file.arrayBuffer();
    const decoded = await new OfflineAudioContext(1, 1, ui.rate).decodeAudioData(data);
    const mix = new OfflineAudioContext(1, decoded.length, ui.rate);
    const source = mix.createBufferSource();
    source.buffer = decoded;
    source.connect(mix.destination);
    source.start();
    sound = (await mix.startRendering()).getChannelData(0);
    send("audio.decoded", JSON.stringify({ samples: sound.length, now: Date.now() }));
  } catch (error) {
    send("audio.failed", String(error.name || error));
  }
}

function hand() {
  worker.postMessage({ type: "audio", sound }, [sound.buffer]);
  sound = null;
}

function job() {
  worker.postMessage({ type: "job", job: ui.job });
}

function save() {
  const url = URL.createObjectURL(new Blob([tool.file()], { type: ui.file_type }));
  const a = make("a");
  a.href = url;
  a.download = ui.file_name;
  document.body.append(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(url), options.revoke_ms);
}

function clear() {
  if (picker) picker.value = "";
  file = null;
}

const effects = {
  "worker.start": start,
  "worker.stop": stop,
  "audio.decode": decode,
  "audio.send": hand,
  job,
  "file.save": save,
  "input.clear": clear,
};

async function main() {
  await init();
  const text = async (path) => (await fetch(path)).text();
  const strings = await text("data/strings.json");
  const glossary = await text("data/glossary.md");
  const files = await text("data/model-files.json");
  models = JSON.parse(await text("data/models.json"));
  options = JSON.parse(await text("data/page.json"));
  const abilities = {
    worker: typeof Worker === "function",
    audio: typeof OfflineAudioContext === "function",
  };
  tool = new Tool(strings, glossary, files, JSON.stringify(abilities));
  draw(JSON.parse(tool.ui()));
  document.body.dataset.ready = "true";
}

main();
