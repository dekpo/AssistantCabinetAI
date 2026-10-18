// Runs on the audio thread. It hands the microphone's samples to the page in blocks of FRAMES, and
// nothing else: no analysis, no network, no storage. Loaded from the application's own origin
// (`audioWorklet.addModule("/speech/capture-worklet.js")`), so the content security policy needs
// nothing beyond `script-src 'self'`.
const FRAMES = 1024;

class CaptureProcessor extends AudioWorkletProcessor {
  constructor() {
    super();
    this.block = new Float32Array(FRAMES);
    this.filled = 0;
    // On "flush" the page asks for the samples of the block still being filled, at the end of a clip.
    this.port.onmessage = (event) => {
      if (event.data === "flush" && this.filled > 0) {
        const partial = this.block.slice(0, this.filled);
        this.filled = 0;
        this.port.postMessage(partial, [partial.buffer]);
      }
    };
  }

  process(inputs) {
    const channel = inputs[0] && inputs[0][0];
    if (!channel) {
      return true;
    }
    let read = 0;
    while (read < channel.length) {
      const take = Math.min(channel.length - read, FRAMES - this.filled);
      this.block.set(channel.subarray(read, read + take), this.filled);
      this.filled += take;
      read += take;
      if (this.filled === FRAMES) {
        const full = this.block;
        this.block = new Float32Array(FRAMES);
        this.filled = 0;
        this.port.postMessage(full, [full.buffer]);
      }
    }
    return true;
  }
}

registerProcessor("capture-processor", CaptureProcessor);
