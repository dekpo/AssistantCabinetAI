import { describe, expect, it } from "vitest";
import {
  initialTrace,
  minutesAndSeconds,
  progressView,
  roundRemaining,
  stageFraction,
  stageOf,
  stepTrace,
  traceOf,
  type ProgressSample,
} from "./analysisProgress";
import type { IndexProgress } from "./ipc";

function reading(done: number, total: number, extra: Partial<IndexProgress> = {}): IndexProgress {
  return { processedFiles: done, totalFiles: total, batchIndex: 0, batchTotal: 0, ...extra };
}

function meaning(done: number, total: number, files = 10): IndexProgress {
  return reading(files, files, { vectorsDone: done, vectorsTotal: total });
}

/** One progress message per `everyMs`, the number read at time `t` being `readAt(t)`. */
function series(
  total: number,
  until: number,
  readAt: (seconds: number) => number,
  everyMs = 1000,
): ProgressSample[] {
  const samples: ProgressSample[] = [];
  for (let at = 0; at <= until; at += everyMs) {
    samples.push({ at, progress: reading(Math.min(total, Math.floor(readAt(at / 1000))), total) });
  }
  return samples;
}

describe("the stage and its percentage", () => {
  it("tells the three stages apart", () => {
    expect(stageOf(reading(1, 10))).toBe("reading");
    expect(stageOf(reading(10, 10, { readingNames: true }))).toBe("names");
    expect(stageOf(meaning(5, 100))).toBe("meaning");
  });

  it("moves inside a long file by its batches, never past the file's own share", () => {
    expect(stageFraction(reading(3, 10, { batchIndex: 2, batchTotal: 4 }))).toBeCloseTo(0.35);
    expect(stageFraction(reading(3, 10, { batchIndex: 9, batchTotal: 4 }))).toBeCloseTo(0.4);
  });

  it("is never above 100 and calls a stage with nothing to do done", () => {
    expect(stageFraction(reading(12, 10))).toBe(1);
    expect(stageFraction(reading(0, 0))).toBe(1);
    expect(stageFraction(meaning(50, 40))).toBe(1);
    const view = progressView(traceOf(0, [{ at: 1000, progress: reading(12, 10) }]), 1000, "documents");
    expect(view?.percent).toBe(100);
    expect(view?.done).toBe(10);
  });

  it("numbers the stages: two for documents, one for the workbooks", () => {
    const first = traceOf(0, [{ at: 0, progress: reading(1, 10) }]);
    const names = traceOf(0, [{ at: 0, progress: reading(1, 10, { readingNames: true }) }]);
    const later = traceOf(0, [{ at: 0, progress: meaning(1, 10) }]);

    expect(progressView(first, 0, "documents")).toMatchObject({ stageIndex: 1, stageCount: 2 });
    expect(progressView(names, 0, "documents")).toMatchObject({ stageIndex: 1, stageCount: 2 });
    expect(progressView(later, 0, "documents")).toMatchObject({ stageIndex: 2, stageCount: 2 });
    expect(progressView(first, 0, "tables")).toMatchObject({ stageIndex: 1, stageCount: 1 });
  });

  it("counts passages, not files, in the meaning stage", () => {
    const view = progressView(traceOf(0, [{ at: 0, progress: meaning(250, 1000, 40) }]), 0, "documents");

    expect(view).toMatchObject({ stage: "meaning", done: 250, total: 1000, percent: 25 });
  });

  it("says nothing before the pass has reported anything, but its clock runs", () => {
    expect(progressView(initialTrace(5000), 9000, "documents")).toBeNull();
  });

  it("measures the elapsed time from the press, not from the first message", () => {
    const view = progressView(traceOf(1000, [{ at: 4000, progress: reading(1, 10) }]), 7000, "documents");

    expect(view?.elapsedMs).toBe(6000);
  });
});

describe("the remaining time", () => {
  it("is not given before 10 % of the stage, even long after 8 s", () => {
    // One file in 100 per 10 s: 1 % after 10 s.
    const samples = series(100, 20_000, (s) => s / 10);
    const view = progressView(traceOf(0, samples), 20_000, "documents");

    expect(view?.percent).toBe(2);
    expect(view?.remainingMs).toBeNull();
  });

  it("is not given before 8 s, even past 10 %", () => {
    // 5 files a second: 10 % after 2 s.
    const samples = series(100, 6000, (s) => s * 5);
    const early = progressView(traceOf(0, samples), 6000, "documents");
    expect(early?.percent).toBeGreaterThanOrEqual(10);
    expect(early?.remainingMs).toBeNull();

    const later = progressView(traceOf(0, series(100, 9000, (s) => s * 5)), 9000, "documents");
    expect(later?.remainingMs).not.toBeNull();
  });

  it("follows a steady rate", () => {
    // 2 files a second out of 200: 30 s in, 60 are read and 70 s are left.
    const view = progressView(traceOf(0, series(200, 30_000, (s) => s * 2)), 30_000, "documents");

    expect(view?.percent).toBe(30);
    expect(view?.remainingMs ?? 0).toBeGreaterThan(68_000);
    expect(view?.remainingMs ?? 0).toBeLessThan(72_000);
  });

  it("counts down as the clock runs between two messages", () => {
    const trace = traceOf(0, series(200, 30_000, (s) => s * 2));
    const at30 = progressView(trace, 30_000, "documents")?.remainingMs ?? 0;
    const at35 = progressView(trace, 35_000, "documents")?.remainingMs ?? 0;

    expect(at30 - at35).toBeCloseTo(5000, -2);
  });

  it("follows the recent rate, not the overall average, when scans slow the pass down", () => {
    // 10 files a second for 5 s, then one a second: after 35 s, 80 of 100 are read.
    const samples = series(100, 35_000, (s) => (s <= 5 ? s * 10 : 50 + (s - 5)));
    const view = progressView(traceOf(0, samples), 35_000, "documents");
    const overallAverage = (20 * 35_000) / 80;

    expect(view?.percent).toBe(80);
    expect(overallAverage).toBeLessThan(10_000);
    expect(view?.remainingMs ?? 0).toBeGreaterThan(15_000);
  });

  it("does not jump when the rate changes once", () => {
    const steady = series(1000, 60_000, (s) => s * 5);
    const before = progressView(traceOf(0, steady), 60_000, "documents")?.remainingMs ?? 0;
    // The next 5 s bring one file instead of 25.
    const slowed = [...steady, { at: 65_000, progress: reading(301, 1000) }];
    const after = progressView(traceOf(0, slowed), 65_000, "documents")?.remainingMs ?? 0;
    const raw = (1000 - 301) * 5; // the estimate at 1 file in 5 s, in seconds

    expect(after).toBeGreaterThan(before - 5000);
    expect(after).toBeLessThan(raw * 1000);
  });

  it("gives no estimate while nothing moves over the window", () => {
    const stuck = [
      ...series(100, 20_000, (s) => s * 2),
      { at: 40_000, progress: reading(40, 100) },
      { at: 60_000, progress: reading(40, 100) },
    ];
    const view = progressView(traceOf(0, stuck), 60_000, "documents");

    expect(view?.remainingMs).toBeNull();
  });

  it("is zero, never negative, when the pass is late or the stage is complete", () => {
    const trace = traceOf(0, series(200, 30_000, (s) => s * 2));
    expect(progressView(trace, 400_000, "documents")?.remainingMs).toBe(0);

    const done = traceOf(0, [...series(10, 9000, (s) => s), { at: 10_000, progress: reading(10, 10) }]);
    expect(progressView(done, 10_000, "documents")?.remainingMs).toBe(0);
  });
});

describe("a stage that changes or starts over", () => {
  it("starts the next stage's estimate again, with its own clock", () => {
    const first = series(100, 30_000, (s) => s * 3);
    const trace = traceOf(0, [...first, { at: 34_000, progress: meaning(0, 5000) }]);

    const view = progressView(trace, 34_000, "documents");
    expect(view).toMatchObject({ stage: "meaning", percent: 0, remainingMs: null });
    expect(trace.stageStartedAt).toBe(34_000);
  });

  it("gives the meaning stage its own estimate once its gate opens", () => {
    // 100 passages a second out of 2000, from t = 0.
    const samples = [...Array(21).keys()].map((second) => ({
      at: second * 1000,
      progress: meaning(second * 100, 2000),
    }));
    const view = progressView(traceOf(0, samples), 20_000, "documents");

    expect(view?.percent).toBe(100);
    expect(view?.remainingMs).toBe(0);
    const half = progressView(traceOf(0, samples.slice(0, 11)), 10_000, "documents");
    expect(half?.remainingMs ?? 0).toBeGreaterThan(9000);
    expect(half?.remainingMs ?? 0).toBeLessThan(11_000);
  });

  it("forgets what it measured when the share goes back, and never shows a negative time", () => {
    const first = series(100, 30_000, (s) => s * 3);
    const restarted = traceOf(0, [...first, { at: 31_000, progress: reading(2, 100) }]);
    const view = progressView(restarted, 31_000, "documents");

    expect(restarted.stageStartedAt).toBe(31_000);
    expect(view?.remainingMs).toBeNull();
    expect(view?.percent).toBe(2);
  });
});

describe("a small step back", () => {
  it("is jitter, not a restart: the stage's clock and estimate go on", () => {
    const samples = [
      ...series(200, 30_000, (s) => s * 2),
      { at: 31_000, progress: reading(61, 200, { batchIndex: 0, batchTotal: 0 }) },
      { at: 32_000, progress: reading(60, 200) },
    ];
    const trace = traceOf(0, samples);

    expect(trace.stageStartedAt).toBe(0);
    expect(progressView(trace, 32_000, "documents")?.percent).toBe(30);
  });
});

describe("the series stays small", () => {
  it("keeps one sample per quarter of a second at most, and only the window", () => {
    let trace = initialTrace(0);
    for (let at = 0; at <= 120_000; at += 10) {
      trace = stepTrace(trace, { at, progress: reading(Math.floor(at / 100), 100_000) });
    }

    expect(trace.recent.length).toBeLessThan(80);
    expect(trace.recent[trace.recent.length - 1]?.at).toBe(120_000);
  });
});

describe("how a time is said", () => {
  it("rounds a remaining time as it is spoken", () => {
    expect(roundRemaining(0)).toEqual({ unit: "underMinute" });
    expect(roundRemaining(59_000)).toEqual({ unit: "underMinute" });
    expect(roundRemaining(60_000)).toEqual({ unit: "minutes", count: 1 });
    expect(roundRemaining(125_000)).toEqual({ unit: "minutes", count: 2 });
    expect(roundRemaining(89 * 60_000)).toEqual({ unit: "minutes", count: 89 });
    expect(roundRemaining(90 * 60_000)).toEqual({ unit: "hours", count: 2 });
    expect(roundRemaining(5 * 3_600_000)).toEqual({ unit: "hours", count: 5 });
  });

  it("splits an elapsed time into minutes and seconds", () => {
    expect(minutesAndSeconds(228_400)).toEqual({ minutes: 3, seconds: 48 });
    expect(minutesAndSeconds(5000)).toEqual({ minutes: 0, seconds: 5 });
    expect(minutesAndSeconds(-3)).toEqual({ minutes: 0, seconds: 0 });
  });
});
