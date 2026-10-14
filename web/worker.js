// Runs the three models on the samples each job names, and nothing else.
//
// The page sends the models' settings (data/models.json), then the recording
// as samples, then one job at a time. Each job comes back as an answer: the
// segmentation model's scores and the voiceprints as raw 32-bit floats, the
// speech model's lines as JSON. What a job reads and what its answer means is
// decided by the core.
import {
  env,
  pipeline,
  AutoModel,
  AutoModelForAudioFrameClassification,
  AutoProcessor,
} from "./vendor/transformers/transformers.js";

let options = null;
let sound = null;
const models = {};

const tell = (name, value) => self.postMessage({ act: name, value });

function raw(tensor) {
  const data = tensor.data;
  return new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
}

function answer(report, bytes) {
  self.postMessage({ report: JSON.stringify({ ...report, now: Date.now() }), bytes });
}

async function load(message) {
  options = message.options;
  Object.assign(env, options.env);
  env.backends.onnx.wasm.wasmPaths = new URL(options.wasm, self.location).href;
  env.backends.onnx.wasm.numThreads = options.threads;
  const progress_callback = (p) => tell("models.progress", JSON.stringify(p));
  const settings = (part) => ({ dtype: part.dtype, device: options.device, progress_callback });
  try {
    models.asr = await pipeline("automatic-speech-recognition", options.asr.model, settings(options.asr));
    models.segmentReader = await AutoProcessor.from_pretrained(options.segment.model, { progress_callback });
    models.segment = await AutoModelForAudioFrameClassification.from_pretrained(options.segment.model, settings(options.segment));
    models.embedReader = await AutoProcessor.from_pretrained(options.embed.model, { progress_callback });
    models.embed = await AutoModel.from_pretrained(options.embed.model, settings(options.embed));
    tell("models.ready", String(Date.now()));
  } catch (error) {
    tell("models.failed", String(error.message || error));
  }
}

const jobs = {
  async segment(samples) {
    const { logits } = await models.segment(await models.segmentReader(samples));
    answer({ kind: "segment", frames: logits.dims[1] }, raw(logits));
  },
  async embed(samples) {
    const output = (await models.embed(await models.embedReader(samples)))[options.embed.output];
    answer({ kind: "embed", dims: output.data.length }, raw(output));
  },
  async transcribe(samples) {
    const said = await models.asr(samples, options.asr.call);
    answer({ kind: "transcribe", text: JSON.stringify(said) }, new Uint8Array(0));
  },
};

async function run(message) {
  const j = message.job;
  try {
    await jobs[j.kind](sound.subarray(j.start, j.end));
  } catch (error) {
    tell("job.failed", String(error.message || error));
  }
}

const handlers = {
  load,
  audio: (message) => {
    sound = message.sound;
  },
  job: run,
};

self.addEventListener("message", (e) => handlers[e.data.type](e.data));
