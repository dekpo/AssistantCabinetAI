import type { MicrophoneSupport } from "../speech/availability";
import {
  CAPTURE_SAMPLE_RATE,
  chunkLevel,
  concatPcm,
  createStatsAccumulator,
  encodeWav,
  floatToPcm16,
  type AudioStats,
} from "./wav";

/**
 * Microphone capture behind an interface, so a native fallback can replace the webview's
 * `getUserMedia` on a platform where it fails (macOS, a later phase) without touching a caller.
 * Audio stays in memory: nothing here writes it anywhere, and nothing here sends it anywhere. The
 * caller decides what to do with the WAV it gets back, and the clip is not kept once it has.
 */

/** Why a capture could not start. Each has a catalogue sentence, `speech.microphone.<failure>`. */
export type CaptureFailure = "denied" | "missing" | "unsupported" | "failed";

export class CaptureError extends Error {
  constructor(readonly failure: CaptureFailure) {
    super(failure);
    this.name = "CaptureError";
  }
}

export function captureFailureKey(failure: CaptureFailure): string {
  return `speech.microphone.${failure}`;
}

/** The failure a `getUserMedia` rejection stands for, by the name of its `DOMException`. */
export function classifyCaptureError(error: unknown): CaptureFailure {
  if (error instanceof CaptureError) {
    return error.failure;
  }
  const name = typeof error === "object" && error !== null && "name" in error ? error.name : "";
  switch (name) {
    case "NotAllowedError":
    case "SecurityError":
    case "PermissionDeniedError":
      return "denied";
    case "NotFoundError":
    case "OverconstrainedError":
    case "DevicesNotFoundError":
      return "missing";
    default:
      return "failed";
  }
}

export interface CaptureResult {
  /** A mono 16-bit WAV at [`CAPTURE_SAMPLE_RATE`]. */
  wav: Uint8Array;
  durationSeconds: number;
  stats: AudioStats;
  /** True when the audio context ran at another rate and the clip was resampled at the end. */
  resampled: boolean;
}

export interface CaptureOptions {
  /** Told of the input level (0 to 1) about every 64 ms; drives the waveform animation. */
  onLevel?: (level: number) => void;
  /**
   * Streaming mode, for a long recording: each block of the microphone goes here, with the sample
   * rate of the audio context, and nothing is kept by the capture. `stop` then resolves with an empty
   * clip, after the last block was handed over. The caller owns the audio from there.
   */
  onBlock?: (block: Float32Array, rate: number) => void;
}

export interface AudioCapture {
  /** Rejects with a [`CaptureError`]. Nothing is recorded before it resolves. */
  start: (options?: CaptureOptions) => Promise<void>;
  /** Ends the clip and releases the microphone. Rejects when nothing is recording. */
  stop: () => Promise<CaptureResult>;
  /** Drops the clip and releases the microphone. Safe to call at any stage, and twice. */
  cancel: () => void;
}

/** Resamples a clip; the webview gives the browser's own resampler, a test gives a fake. */
export type Resampler = (samples: Float32Array, from: number, to: number) => Promise<Float32Array>;

/**
 * Collects the blocks of one clip. At 16 kHz each block is converted and the floats are dropped at
 * once; at any other rate the floats are kept until `finish` resamples the whole clip (one
 * resampling for the clip, so a block boundary never shows in the result).
 */
export function createClipBuilder(inputRate: number, onLevel?: (level: number) => void) {
  const stats = createStatsAccumulator();
  const direct = inputRate === CAPTURE_SAMPLE_RATE;
  const pcm: Int16Array[] = [];
  const floats: Float32Array[] = [];
  return {
    add: (block: Float32Array) => {
      stats.add(block);
      onLevel?.(chunkLevel(block));
      if (direct) {
        pcm.push(floatToPcm16(block));
      } else {
        floats.push(block);
      }
    },
    finish: async (resample: Resampler): Promise<CaptureResult> => {
      let samples: Int16Array;
      if (direct) {
        samples = concatPcm(pcm);
      } else {
        const total = floats.reduce((sum, block) => sum + block.length, 0);
        const joined = new Float32Array(total);
        let offset = 0;
        for (const block of floats) {
          joined.set(block, offset);
          offset += block.length;
        }
        samples = floatToPcm16(await resample(joined, inputRate, CAPTURE_SAMPLE_RATE));
      }
      return {
        wav: encodeWav(samples, CAPTURE_SAMPLE_RATE),
        durationSeconds: samples.length / CAPTURE_SAMPLE_RATE,
        stats: stats.result(),
        resampled: !direct,
      };
    },
  };
}

/** The worklet is a static file of the application (copied from `public/`), loaded from its own
 * origin: the content security policy allows it as `script-src 'self'`, with no `blob:`. */
const WORKLET_URL = "/speech/capture-worklet.js";
const WORKLET_NAME = "capture-processor";
/** Time given to the last block to arrive after the worklet is asked to flush it. */
const FLUSH_MILLISECONDS = 150;

export function microphoneSupport(): MicrophoneSupport {
  const hasStream =
    typeof navigator !== "undefined" && typeof navigator.mediaDevices?.getUserMedia === "function";
  const hasWorklet =
    typeof AudioContext !== "undefined" && "audioWorklet" in AudioContext.prototype;
  return hasStream && hasWorklet ? "supported" : "unsupported";
}

/**
 * One passage of a long recording as a WAV clip at the engine's rate, resampled by the browser when
 * the audio context ran at another one. The samples are not kept once the clip is built.
 */
export function clipFromSamples(samples: Float32Array, rate: number): Promise<CaptureResult> {
  const builder = createClipBuilder(rate);
  builder.add(samples);
  return builder.finish(resampleWithTheBrowser);
}

async function resampleWithTheBrowser(
  samples: Float32Array,
  from: number,
  to: number,
): Promise<Float32Array> {
  const length = Math.max(1, Math.round((samples.length * to) / from));
  const offline = new OfflineAudioContext(1, length, to);
  const buffer = offline.createBuffer(1, samples.length, from);
  // The blocks come from the worklet through `postMessage`, so they are never shared memory.
  buffer.copyToChannel(samples as Float32Array<ArrayBuffer>, 0);
  const source = offline.createBufferSource();
  source.buffer = buffer;
  source.connect(offline.destination);
  source.start();
  const rendered = await offline.startRendering();
  return rendered.getChannelData(0);
}

/** The webview implementation: echo cancellation, noise suppression and automatic gain off, so the
 * clip is as raw as the microphone gives it. */
export function createWebviewCapture(): AudioCapture {
  let context: AudioContext | null = null;
  let stream: MediaStream | null = null;
  let node: AudioWorkletNode | null = null;
  let clip: ReturnType<typeof createClipBuilder> | null = null;

  const release = () => {
    stream?.getTracks().forEach((track) => track.stop());
    node?.port.close();
    node?.disconnect();
    if (context !== null && context.state !== "closed") {
      void context.close().catch(() => undefined);
    }
    context = null;
    stream = null;
    node = null;
    clip = null;
  };

  return {
    start: async (options) => {
      if (context !== null || stream !== null) {
        throw new CaptureError("failed");
      }
      if (microphoneSupport() === "unsupported") {
        throw new CaptureError("unsupported");
      }
      try {
        stream = await navigator.mediaDevices.getUserMedia({
          audio: {
            channelCount: 1,
            echoCancellation: false,
            noiseSuppression: false,
            autoGainControl: false,
          },
        });
        try {
          // Ask for the engine's rate directly; the webview may answer with its own, which
          // `createClipBuilder` then resamples once at the end of the clip.
          context = new AudioContext({ sampleRate: CAPTURE_SAMPLE_RATE });
        } catch {
          context = new AudioContext();
        }
        await context.audioWorklet.addModule(WORKLET_URL);
        const rate = context.sampleRate;
        const builder = createClipBuilder(rate, options?.onLevel);
        clip = builder;
        const source = context.createMediaStreamSource(stream);
        node = new AudioWorkletNode(context, WORKLET_NAME);
        const onBlock = options?.onBlock;
        node.port.onmessage = (event: MessageEvent<Float32Array>) => {
          if (onBlock === undefined) {
            builder.add(event.data);
          } else {
            options?.onLevel?.(chunkLevel(event.data));
            onBlock(event.data, rate);
          }
        };
        source.connect(node);
        // A worklet is only pulled while it is part of the graph; it outputs silence.
        node.connect(context.destination);
      } catch (error) {
        release();
        throw new CaptureError(classifyCaptureError(error));
      }
    },
    stop: async () => {
      if (context === null || node === null || clip === null) {
        throw new CaptureError("failed");
      }
      const finished = clip;
      node.port.postMessage("flush");
      await new Promise((resolve) => setTimeout(resolve, FLUSH_MILLISECONDS));
      release();
      return finished.finish(resampleWithTheBrowser);
    },
    cancel: release,
  };
}
