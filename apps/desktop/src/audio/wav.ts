/**
 * Pure helpers for the audio the microphone gives: the 16-bit conversion, the WAV container Rust
 * reads, and the statistics (peak, clipped share, level) that say whether a clip is usable. They
 * work on plain arrays, so they are tested on synthetic signals and need no browser.
 */

/** Sample rate every clip is sent at: what the speech-to-text engine expects. */
export const CAPTURE_SAMPLE_RATE = 16_000;

/** A float sample at or above this magnitude counts as clipped. */
export const CLIPPING_LEVEL = 0.99;

/** Floating samples (-1 to 1) as 16-bit PCM, rounded and clamped. */
export function floatToPcm16(samples: Float32Array): Int16Array {
  const pcm = new Int16Array(samples.length);
  for (let index = 0; index < samples.length; index += 1) {
    const clamped = Math.max(-1, Math.min(1, samples[index] as number));
    pcm[index] = Math.round(clamped < 0 ? clamped * 0x8000 : clamped * 0x7fff);
  }
  return pcm;
}

/** A mono 16-bit PCM WAV file (44-byte header, little-endian). */
export function encodeWav(pcm: Int16Array, sampleRate: number = CAPTURE_SAMPLE_RATE): Uint8Array {
  const dataBytes = pcm.length * 2;
  const bytes = new Uint8Array(44 + dataBytes);
  const view = new DataView(bytes.buffer);
  const tag = (offset: number, text: string) => {
    for (let index = 0; index < text.length; index += 1) {
      view.setUint8(offset + index, text.charCodeAt(index));
    }
  };
  tag(0, "RIFF");
  view.setUint32(4, 36 + dataBytes, true);
  tag(8, "WAVE");
  tag(12, "fmt ");
  view.setUint32(16, 16, true); // size of the format block
  view.setUint16(20, 1, true); // PCM
  view.setUint16(22, 1, true); // mono
  view.setUint32(24, sampleRate, true);
  view.setUint32(28, sampleRate * 2, true); // bytes per second
  view.setUint16(32, 2, true); // bytes per frame
  view.setUint16(34, 16, true); // bits per sample
  tag(36, "data");
  view.setUint32(40, dataBytes, true);
  for (let index = 0; index < pcm.length; index += 1) {
    view.setInt16(44 + index * 2, pcm[index] as number, true);
  }
  return bytes;
}

/** Root-mean-square level of a chunk, 0 to 1: what the waveform animation follows. */
export function chunkLevel(samples: Float32Array): number {
  if (samples.length === 0) {
    return 0;
  }
  let sum = 0;
  for (let index = 0; index < samples.length; index += 1) {
    const value = samples[index] as number;
    sum += value * value;
  }
  return Math.min(1, Math.sqrt(sum / samples.length));
}

export interface AudioStats {
  samples: number;
  /** Highest magnitude seen, 0 to 1. */
  peak: number;
  /** Share of samples at or above [`CLIPPING_LEVEL`], 0 to 1. */
  clippedShare: number;
}

/** Statistics over a clip that arrives in chunks: nothing of the audio is kept. */
export function createStatsAccumulator(): {
  add: (chunk: Float32Array) => void;
  result: () => AudioStats;
} {
  let samples = 0;
  let peak = 0;
  let clipped = 0;
  return {
    add: (chunk) => {
      for (let index = 0; index < chunk.length; index += 1) {
        const magnitude = Math.abs(chunk[index] as number);
        if (magnitude > peak) {
          peak = magnitude;
        }
        if (magnitude >= CLIPPING_LEVEL) {
          clipped += 1;
        }
      }
      samples += chunk.length;
    },
    result: () => ({
      samples,
      peak: Math.min(1, peak),
      clippedShare: samples === 0 ? 0 : clipped / samples,
    }),
  };
}

/** Joins the chunks of one clip into a single array. */
export function concatPcm(chunks: Int16Array[]): Int16Array {
  const joined = new Int16Array(chunks.reduce((total, chunk) => total + chunk.length, 0));
  let offset = 0;
  for (const chunk of chunks) {
    joined.set(chunk, offset);
    offset += chunk.length;
  }
  return joined;
}
