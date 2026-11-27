import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { runInThisContext } from "node:vm";

globalThis.sampleRate = 44100;
globalThis.AudioWorkletProcessor = class {
  constructor() {
    this.port = { onmessage: null };
  }
};
let Processor;
globalThis.registerProcessor = (name, constructor) => {
  assert.equal(name, "foundation-pcm-mixer");
  Processor = constructor;
};
runInThisContext(readFileSync(new URL("../assets/audio_worklet.js", import.meta.url), "utf8"));

function makeMixer() {
  assert.ok(Processor, "AudioWorklet processor was registered");
  return new Processor();
}
function send(mixer, message) {
  mixer.onMessage(message);
}
function render(mixer, frames) {
  const left = new Float32Array(frames);
  const right = new Float32Array(frames);
  mixer.process([], [[left, right]]);
  return [Array.from(left, value => Math.round(value * 32768)),
          Array.from(right, value => Math.round(value * 32768))];
}

{
  const mixer = makeMixer();
  send(mixer, { type: "register", clip: 1, pcm: new Int16Array([1, 10, 2, 20, 3, 30]) });
  send(mixer, { type: "register", clip: 2, pcm: new Int16Array([4, 40, 5, 50, 6, 60]) });
  send(mixer, { type: "play", clip: 1, looped: false });
  send(mixer, { type: "play", clip: 2, looped: false });
  const [left, right] = render(mixer, 3);
  assert.deepEqual(left, [5, 7, 9]);
  assert.deepEqual(right, [50, 70, 90]);
}

{
  const mixer = makeMixer();
  send(mixer, { type: "register", clip: 7, pcm: new Int16Array([1, 10, 2, 20]) });
  send(mixer, { type: "play", clip: 7, looped: true });
  const [left, right] = render(mixer, 5);
  assert.deepEqual(left, [1, 2, 1, 2, 1]);
  assert.deepEqual(right, [10, 20, 10, 20, 10]);
}

{
  const mixer = makeMixer();
  send(mixer, { type: "register", clip: 3, pcm: new Int16Array([100, -100, 200, -200]) });
  send(mixer, { type: "play", clip: 3, looped: true });
  render(mixer, 1);
  send(mixer, { type: "play", clip: 3, looped: false });
  const [left, right] = render(mixer, 2);
  assert.deepEqual(left, [100, 200]);
  assert.deepEqual(right, [-100, -200]);
}

{
  const mixer = makeMixer();
  send(mixer, { type: "register", clip: 1, pcm: new Int16Array([20000, -20000]) });
  send(mixer, { type: "register", clip: 2, pcm: new Int16Array([20000, -20000]) });
  send(mixer, { type: "play", clip: 1, looped: false });
  send(mixer, { type: "play", clip: 2, looped: false });
  const [left, right] = render(mixer, 1);
  assert.deepEqual(left, [32767]);
  assert.deepEqual(right, [-32768]);
}

{
  const mixer = makeMixer();
  for (let id = 0; id < 48; id += 1) {
    send(mixer, { type: "register", clip: id, pcm: new Int16Array([id + 1, id + 1]) });
    send(mixer, { type: "play", clip: id, looped: false });
  }
  send(mixer, { type: "register", clip: 48, pcm: new Int16Array([49, 49]) });
  send(mixer, { type: "play", clip: 48, looped: false });
  const [left, right] = render(mixer, 1);
  assert.deepEqual(left, [1224]);
  assert.deepEqual(right, [1224]);
}

{
  const mixer = makeMixer();
  send(mixer, { type: "register", clip: 1, pcm: new Int16Array([100, 200, 300, 400]) });
  send(mixer, { type: "play", clip: 1, looped: true });
  send(mixer, { type: "stop", clip: 1 });
  const [left, right] = render(mixer, 2);
  assert.deepEqual(left, [0, 0]);
  assert.deepEqual(right, [0, 0]);
}

{
  const mixer = makeMixer();
  send(mixer, { type: "register", clip: 1, pcm: new Int16Array([100, 200]) });
  send(mixer, { type: "play", clip: 1, looped: true });
  send(mixer, { type: "stop-all" });
  const [left, right] = render(mixer, 2);
  assert.deepEqual(left, [0, 0]);
  assert.deepEqual(right, [0, 0]);
}

{
  // 48 kHz device output is resampled only after the canonical integer mix.
  globalThis.sampleRate = 48_000;
  const mixer = makeMixer();
  send(mixer, {
    type: "register",
    clip: 77,
    pcm: new Int16Array([0, 0, 10_000, -10_000, 20_000, -20_000, 30_000, -30_000]),
  });
  send(mixer, { type: "play", clip: 77, looped: false });
  const [left, right] = render(mixer, 4);
  assert.deepEqual(left, [0, 9188, 18375, 27563]);
  assert.deepEqual(right, [0, -9187, -18375, -27562]);
}

globalThis.sampleRate = 44_100;
console.log("AudioWorklet PCM mixer tests passed.");
