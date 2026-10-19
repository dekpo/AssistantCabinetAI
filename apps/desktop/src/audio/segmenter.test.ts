import { describe, expect, it } from "vitest";
import {
  CUT_SILENCE_SECONDS,
  MAX_SEGMENT_SECONDS,
  MIN_SEGMENT_SECONDS,
  MIN_SPEECH_SECONDS,
  PRE_ROLL_SECONDS,
  createSegmenter,
  type Segment,
} from "./segmenter";

const RATE = 16_000;
const BLOCK = 1_024;

/** A tone at `level` (amplitude) lasting `seconds`, as blocks of the size the worklet hands over. */
function tone(seconds: number, level = 0.2, phase = 0): Float32Array {
  const samples = new Float32Array(Math.round(seconds * RATE));
  for (let index = 0; index < samples.length; index += 1) {
    samples[index] = level * Math.sin(((index + phase) / RATE) * 2 * Math.PI * 220);
  }
  return samples;
}

function silence(seconds: number): Float32Array {
  return new Float32Array(Math.round(seconds * RATE));
}

function concat(parts: Float32Array[]): Float32Array {
  const joined = new Float32Array(parts.reduce((total, part) => total + part.length, 0));
  let offset = 0;
  for (const part of parts) {
    joined.set(part, offset);
    offset += part.length;
  }
  return joined;
}

/** Runs a whole signal through a segmenter, block by block, and returns the passages in order. */
function run(signal: Float32Array, finish = true): Segment[] {
  const passages: Segment[] = [];
  const segmenter = createSegmenter(RATE, (segment) => passages.push(segment));
  for (let from = 0; from < signal.length; from += BLOCK) {
    segmenter.push(signal.slice(from, Math.min(signal.length, from + BLOCK)));
  }
  if (finish) {
    segmenter.finish();
  }
  return passages;
}

const secondsOf = (segment: Segment) => segment.samples.length / segment.rate;

describe("cutting on a silence", () => {
  it("cuts after a silence once the passage is long enough", () => {
    const passages = run(concat([tone(2), silence(1.5), tone(2)]));

    expect(passages).toHaveLength(2);
    expect(secondsOf(passages[0] as Segment)).toBeGreaterThanOrEqual(2 + CUT_SILENCE_SECONDS);
    expect(secondsOf(passages[0] as Segment)).toBeLessThan(2 + CUT_SILENCE_SECONDS + 0.2);
  });

  it("does not cut before the passage reaches its minimum length", () => {
    const passages = run(concat([tone(0.22), silence(1.2), tone(2)]));

    expect(passages).toHaveLength(1);
    expect(secondsOf(passages[0] as Segment)).toBeGreaterThan(MIN_SEGMENT_SECONDS);
  });

  it("does not cut on a pause shorter than the silence", () => {
    const passages = run(concat([tone(2), silence(CUT_SILENCE_SECONDS - 0.2), tone(2)]));

    expect(passages).toHaveLength(1);
  });

  it("hands over nothing before the end while the passage is still being spoken", () => {
    expect(run(tone(10), false)).toHaveLength(0);
    expect(run(tone(10))).toHaveLength(1);
  });
});

describe("the hard cap", () => {
  it("cuts a passage that never pauses, at its quietest block", () => {
    const dip = 14.2;
    const signal = concat([tone(dip, 0.3), tone(0.12, 0.05), tone(30 - dip - 0.12, 0.3)]);
    const passages = run(signal);

    expect(passages.length).toBeGreaterThanOrEqual(2);
    for (const passage of passages) {
      expect(secondsOf(passage)).toBeLessThanOrEqual(MAX_SEGMENT_SECONDS + BLOCK / RATE);
    }
    const first = secondsOf(passages[0] as Segment);
    expect(first).toBeGreaterThan(dip - 0.2);
    expect(first).toBeLessThan(dip + 0.4);
  });

  it("keeps every sample when it cuts", () => {
    const signal = tone(60, 0.3);
    const passages = run(signal);

    expect(passages.length).toBeGreaterThanOrEqual(3);
    const kept = concat(passages.map((passage) => passage.samples));
    expect(kept.length).toBe(signal.length);
    expect(Array.from(kept.subarray(0, 4000))).toStrictEqual(Array.from(signal.subarray(0, 4000)));
    expect(kept[kept.length - 1]).toBe(signal[signal.length - 1]);
  });
});

describe("silence and noise", () => {
  it("drops a recording with no speech in it", () => {
    expect(run(silence(20))).toHaveLength(0);
  });

  it("drops a passage with less speech than the minimum", () => {
    expect(run(concat([silence(1), tone(MIN_SPEECH_SECONDS / 2), silence(3)]))).toHaveLength(0);
  });

  it("drops the silence between passages and keeps a short pre-roll before the next one", () => {
    const passages = run(concat([tone(2), silence(8), tone(2)]));

    expect(passages).toHaveLength(2);
    const second = passages[1] as Segment;
    expect(secondsOf(second)).toBeGreaterThanOrEqual(2 + PRE_ROLL_SECONDS - 0.1);
    expect(secondsOf(second)).toBeLessThan(2 + PRE_ROLL_SECONDS + 0.2);
  });

  it("keeps the speech of a quiet voice", () => {
    expect(run(concat([silence(0.5), tone(2, 0.02)]))).toHaveLength(1);
  });

  it("does not take a hiss for speech", () => {
    expect(run(tone(10, 0.004))).toHaveLength(0);
  });

  it("hears speech from the very first block", () => {
    const passages = run(concat([tone(2), silence(2)]));

    expect(passages).toHaveLength(1);
    expect(Array.from((passages[0] as Segment).samples.subarray(0, 100))).toStrictEqual(
      Array.from(tone(2).subarray(0, 100)),
    );
  });

  it("keeps every sample of a passage with speech, unchanged and in order", () => {
    const a = tone(2, 0.2, 0);
    const b = tone(3, 0.25, 7);
    const passages = run(concat([a, silence(1.5), b]));

    expect(passages).toHaveLength(2);
    const first = (passages[0] as Segment).samples;
    expect(Array.from(first.subarray(0, a.length))).toStrictEqual(Array.from(a));
    const second = (passages[1] as Segment).samples;
    expect(Array.from(second.subarray(second.length - b.length))).toStrictEqual(Array.from(b));
  });

  it("ignores an empty block", () => {
    const passages: Segment[] = [];
    const segmenter = createSegmenter(RATE, (segment) => passages.push(segment));
    segmenter.push(new Float32Array(0));
    segmenter.finish();

    expect(passages).toHaveLength(0);
  });
});

describe("another sample rate", () => {
  it("counts its lengths in seconds, not in samples", () => {
    const passages: Segment[] = [];
    const segmenter = createSegmenter(48_000, (segment) => passages.push(segment));
    const block = new Float32Array(3_072).fill(0.2);
    const quiet = new Float32Array(3_072);
    for (let index = 0; index < 40; index += 1) segmenter.push(block);
    for (let index = 0; index < 30; index += 1) segmenter.push(quiet);
    segmenter.finish();

    expect(passages).toHaveLength(1);
    expect(passages[0]?.rate).toBe(48_000);
  });
});
