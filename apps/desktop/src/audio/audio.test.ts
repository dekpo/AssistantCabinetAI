import { describe, expect, it } from "vitest";
import {
  CaptureError,
  captureFailureKey,
  classifyCaptureError,
  createClipBuilder,
} from "./recorder";
import {
  CAPTURE_SAMPLE_RATE,
  chunkLevel,
  concatPcm,
  createStatsAccumulator,
  encodeWav,
  floatToPcm16,
} from "./wav";

function sine(frames: number, amplitude: number): Float32Array {
  return Float32Array.from({ length: frames }, (_, index) =>
    amplitude * Math.sin((2 * Math.PI * 440 * index) / CAPTURE_SAMPLE_RATE),
  );
}

describe("16-bit conversion", () => {
  it("maps the extremes and the middle, and clamps what is out of range", () => {
    const pcm = floatToPcm16(Float32Array.of(0, 1, -1, 0.5, 2, -2));

    expect([...pcm]).toStrictEqual([0, 32767, -32768, 16384, 32767, -32768]);
  });

  it("keeps the length", () => {
    expect(floatToPcm16(new Float32Array(1234)).length).toBe(1234);
  });
});

describe("the WAV container", () => {
  const pcm = Int16Array.of(1, -2, 300, -32768, 32767);
  const wav = encodeWav(pcm);
  const view = new DataView(wav.buffer);
  const text = (offset: number, length: number) =>
    String.fromCharCode(...wav.slice(offset, offset + length));

  it("has the RIFF header of a mono 16-bit 16 kHz file", () => {
    expect(text(0, 4)).toBe("RIFF");
    expect(text(8, 4)).toBe("WAVE");
    expect(text(12, 4)).toBe("fmt ");
    expect(view.getUint16(20, true)).toBe(1);
    expect(view.getUint16(22, true)).toBe(1);
    expect(view.getUint32(24, true)).toBe(16000);
    expect(view.getUint32(28, true)).toBe(32000);
    expect(view.getUint16(32, true)).toBe(2);
    expect(view.getUint16(34, true)).toBe(16);
    expect(text(36, 4)).toBe("data");
  });

  it("states sizes that match the bytes", () => {
    expect(wav.length).toBe(44 + pcm.length * 2);
    expect(view.getUint32(4, true)).toBe(wav.length - 8);
    expect(view.getUint32(40, true)).toBe(pcm.length * 2);
  });

  it("writes the samples little-endian, in order", () => {
    const read = Array.from({ length: pcm.length }, (_, index) =>
      view.getInt16(44 + index * 2, true),
    );

    expect(read).toStrictEqual([...pcm]);
  });

  it("encodes an empty clip as a header only", () => {
    expect(encodeWav(new Int16Array(0)).length).toBe(44);
  });
});

describe("clip statistics", () => {
  it("reports the peak and a clipped share of zero for a quiet signal", () => {
    const stats = createStatsAccumulator();
    stats.add(sine(1600, 0.25));

    const result = stats.result();
    expect(result.peak).toBeCloseTo(0.25, 2);
    expect(result.clippedShare).toBe(0);
    expect(result.samples).toBe(1600);
  });

  it("counts the samples at full scale, across chunks", () => {
    const stats = createStatsAccumulator();
    stats.add(Float32Array.of(1, -1, 0, 0));
    stats.add(Float32Array.of(0.5, 0.99, 0, 0));

    const result = stats.result();
    expect(result.peak).toBe(1);
    expect(result.clippedShare).toBe(3 / 8);
  });

  it("is all zero for no audio", () => {
    expect(createStatsAccumulator().result()).toStrictEqual({
      samples: 0,
      peak: 0,
      clippedShare: 0,
    });
  });

  it("reads the level as the root mean square", () => {
    expect(chunkLevel(new Float32Array(0))).toBe(0);
    expect(chunkLevel(new Float32Array(100))).toBe(0);
    expect(chunkLevel(sine(16000, 1))).toBeCloseTo(Math.SQRT1_2, 2);
    expect(chunkLevel(new Float32Array(10).fill(1))).toBe(1);
  });
});

describe("a clip built from blocks", () => {
  it("joins blocks at 16 kHz into one WAV and tells the level of each", async () => {
    const levels: number[] = [];
    const clip = createClipBuilder(CAPTURE_SAMPLE_RATE, (level) => levels.push(level));
    clip.add(sine(1024, 0.5));
    clip.add(sine(512, 0.5));

    const result = await clip.finish(() => Promise.reject(new Error("no resampling expected")));

    expect(levels).toHaveLength(2);
    expect(result.resampled).toBe(false);
    expect(result.durationSeconds).toBeCloseTo(1536 / CAPTURE_SAMPLE_RATE, 6);
    expect(result.wav.length).toBe(44 + 1536 * 2);
    expect(result.stats.peak).toBeCloseTo(0.5, 2);
  });

  it("resamples the whole clip once when the context ran at another rate", async () => {
    const calls: Array<[number, number, number]> = [];
    const clip = createClipBuilder(48_000);
    clip.add(new Float32Array(480));
    clip.add(new Float32Array(480));

    const result = await clip.finish(async (samples, from, to) => {
      calls.push([samples.length, from, to]);
      return new Float32Array(Math.round((samples.length * to) / from));
    });

    expect(calls).toStrictEqual([[960, 48_000, 16_000]]);
    expect(result.resampled).toBe(true);
    expect(result.durationSeconds).toBeCloseTo(320 / CAPTURE_SAMPLE_RATE, 6);
  });

  it("makes an empty clip when nothing arrived", async () => {
    const result = await createClipBuilder(CAPTURE_SAMPLE_RATE).finish(() => Promise.reject());

    expect(result.durationSeconds).toBe(0);
    expect(result.wav.length).toBe(44);
  });

  it("joins pcm chunks in order", () => {
    expect([...concatPcm([Int16Array.of(1, 2), Int16Array.of(3)])]).toStrictEqual([1, 2, 3]);
  });
});

describe("why a capture could not start", () => {
  it.each([
    ["NotAllowedError", "denied"],
    ["SecurityError", "denied"],
    ["NotFoundError", "missing"],
    ["OverconstrainedError", "missing"],
    ["NotReadableError", "failed"],
    ["AbortError", "failed"],
  ])("reads %s as %s", (name, failure) => {
    expect(classifyCaptureError({ name })).toBe(failure);
  });

  it("keeps the failure of a CaptureError and calls anything else failed", () => {
    expect(classifyCaptureError(new CaptureError("unsupported"))).toBe("unsupported");
    expect(classifyCaptureError("boom")).toBe("failed");
    expect(classifyCaptureError(null)).toBe("failed");
  });

  it("has a catalogue key per failure", () => {
    expect(captureFailureKey("denied")).toBe("speech.microphone.denied");
  });
});
