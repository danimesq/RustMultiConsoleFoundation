/* Foundation deterministic 44.1 kHz S16 stereo mixer.
 * Keep this algorithm in lockstep with crates/foundation-runtime/src/audio.rs.
 * WebAudio's final float conversion and device-rate resampling happen only after
 * the same integer sum, slot ordering, voice stealing, and i16 saturation.
 */
class FoundationPcmMixer extends AudioWorkletProcessor {
  constructor() {
    super();
    this.samples = new Map();
    this.voices = Array(48).fill(null);
    this.nextSequence = 0;
    this.sourceFrameA = null;
    this.sourceFrameB = null;
    this.outputRate = Math.max(1, Math.round(sampleRate));
    this.sourcePositionRemainder = 0;
    this.pendingSourceAdvance = 0;
    this.port.onmessage = (event) => this.onMessage(event.data || {});
  }

  onMessage(message) {
    const clip = String(message.clip);
    switch (message.type) {
      case "register": {
        const input = message.pcm;
        const pcm = input instanceof Int16Array
          ? new Int16Array(input)
          : new Int16Array(input);
        this.samples.set(clip, pcm);
        break;
      }
      case "unregister":
        this.samples.delete(clip);
        this.stop(clip);
        break;
      case "play":
        this.play(clip, Boolean(message.looped));
        break;
      case "stop":
        this.stop(clip);
        break;
      case "stop-all":
        this.voices.fill(null);
        break;
      default:
        break;
    }
  }

  play(clip, looped) {
    const pcm = this.samples.get(clip);
    if (!pcm || pcm.length < 2) return false;

    // Replaying the same clip removes every older voice of that clip first.
    this.stop(clip);

    let slot = this.voices.indexOf(null);
    if (slot < 0) {
      // Match Rust's (candidate.wrapping_sub(oldest) as i32) < 0.
      let oldest = 0;
      for (let candidate = 1; candidate < this.voices.length; candidate += 1) {
        const difference = (this.voices[candidate].sequence - this.voices[oldest].sequence) | 0;
        if (difference < 0) oldest = candidate;
      }
      slot = oldest;
    }

    this.nextSequence = (this.nextSequence + 1) >>> 0;
    this.voices[slot] = {
      clip,
      pcm,
      positionFrames: 0,
      looped,
      sequence: this.nextSequence,
    };
    return true;
  }

  stop(clip) {
    for (let slot = 0; slot < this.voices.length; slot += 1) {
      const voice = this.voices[slot];
      if (voice && voice.clip === clip) this.voices[slot] = null;
    }
  }

  mixFrame() {
    let left = 0;
    let right = 0;

    // Slot order is observable and intentionally matches AudioMixer::mix_voices_into.
    for (let slot = 0; slot < this.voices.length; slot += 1) {
      let voice = this.voices[slot];
      if (!voice) continue;

      const sampleFrames = Math.floor(voice.pcm.length / 2);
      let ended = sampleFrames === 0;
      if (!ended && voice.positionFrames >= sampleFrames) {
        if (voice.looped) voice.positionFrames = 0;
        else ended = true;
      }

      if (!ended) {
        const offset = voice.positionFrames * 2;
        left += voice.pcm[offset];
        right += voice.pcm[offset + 1];
        voice.positionFrames += 1;
        if (voice.positionFrames >= sampleFrames) {
          if (voice.looped) voice.positionFrames = 0;
          else ended = true;
        }
      }

      if (ended) this.voices[slot] = null;
    }

    return [
      Math.max(-32768, Math.min(32767, left)),
      Math.max(-32768, Math.min(32767, right)),
    ];
  }

  nextSourceFrame() {
    return this.mixFrame();
  }

  process(_inputs, outputs) {
    const output = outputs[0];
    if (!output || output.length === 0) return true;
    const left = output[0];
    const right = output[1] || output[0];

    for (let frame = 0; frame < left.length; frame += 1) {
      if (this.sourceFrameA === null) {
        this.sourceFrameA = this.nextSourceFrame();
      }
      // Apply advances lazily at the next output frame. This preserves replay
      // and stop timing when control messages arrive between process quanta.
      while (this.pendingSourceAdvance > 0) {
        this.sourceFrameA = this.sourceFrameB || this.nextSourceFrame();
        this.sourceFrameB = null;
        this.pendingSourceAdvance -= 1;
      }

      const fraction = this.sourcePositionRemainder / this.outputRate;
      if (fraction > 0 && this.sourceFrameB === null) {
        this.sourceFrameB = this.nextSourceFrame();
      }

      if (fraction === 0 || this.sourceFrameB === null) {
        left[frame] = this.sourceFrameA[0] / 32768;
        if (output.length > 1) right[frame] = this.sourceFrameA[1] / 32768;
      } else {
        left[frame] = (
          this.sourceFrameA[0] +
          (this.sourceFrameB[0] - this.sourceFrameA[0]) * fraction
        ) / 32768;
        if (output.length > 1) {
          right[frame] = (
            this.sourceFrameA[1] +
            (this.sourceFrameB[1] - this.sourceFrameA[1]) * fraction
          ) / 32768;
        }
      }

      // Rational 44.1 kHz -> device-rate stepping avoids long-session
      // floating-point phase drift. Shift the cached source frame only after
      // the current interpolated output frame has been written.
      this.sourcePositionRemainder += 44100;
      this.pendingSourceAdvance += Math.floor(
        this.sourcePositionRemainder / this.outputRate,
      );
      this.sourcePositionRemainder %= this.outputRate;
    }
    return true;
  }
}

registerProcessor("foundation-pcm-mixer", FoundationPcmMixer);
