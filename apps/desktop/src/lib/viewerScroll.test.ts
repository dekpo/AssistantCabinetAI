import { describe, expect, it } from "vitest";
import {
  RESTORE_LIMIT_MS,
  RESTORE_START,
  STABLE_FRAMES,
  restoreStep,
  scrollRange,
  scrollRatio,
  scrollTopFor,
  type RestoreProgress,
} from "./viewerScroll";

const box = (scrollHeight: number, clientHeight = 500) => ({ scrollHeight, clientHeight });

describe("scrollRatio", () => {
  it("is the position over the scrollable height", () => {
    expect(scrollRatio(250, box(1000))).toBe(0.5);
    expect(scrollRatio(0, box(1000))).toBe(0);
    expect(scrollRatio(500, box(1000))).toBe(1);
  });

  it("is clamped to 0..1", () => {
    expect(scrollRatio(900, box(1000))).toBe(1);
    expect(scrollRatio(-20, box(1000))).toBe(0);
  });

  it("is 0 for a box that does not scroll", () => {
    expect(scrollRange(box(400))).toBe(0);
    expect(scrollRatio(0, box(400))).toBe(0);
    expect(scrollRatio(30, box(500))).toBe(0);
  });
});

describe("scrollTopFor", () => {
  it("puts the box at the ratio of its range, whatever its height", () => {
    expect(scrollTopFor(0.5, box(1000))).toBe(250);
    expect(scrollTopFor(0.5, box(3000))).toBe(1250);
  });

  it("is clamped and is the top for a box that does not scroll", () => {
    expect(scrollTopFor(2, box(1000))).toBe(500);
    expect(scrollTopFor(-1, box(1000))).toBe(0);
    expect(scrollTopFor(0.5, box(400))).toBe(0);
  });
});

describe("restoreStep", () => {
  it("waits while the box cannot scroll yet", () => {
    const step = restoreStep(0.5, box(400), RESTORE_START, 10);
    expect(step.scrollTop).toBeNull();
    expect(step.done).toBe(false);
  });

  it("sets the position on every frame and finishes once the height stopped growing", () => {
    let progress: RestoreProgress = RESTORE_START;
    let done = false;
    const heights = [1000, 1600, 2400, 2400, 2400, 2400];
    const tops: Array<number | null> = [];
    heights.forEach((height, frame) => {
      const step = restoreStep(0.5, box(height), progress, frame * 16);
      progress = step.progress;
      tops.push(step.scrollTop);
      done = step.done;
    });
    expect(tops).toEqual([250, 550, 950, 950, 950, 950]);
    expect(done).toBe(true);
  });

  it("does not finish before the height was stable for the required frames", () => {
    let progress: RestoreProgress = RESTORE_START;
    let last = restoreStep(0.5, box(1000), progress, 0);
    for (let frame = 1; frame < STABLE_FRAMES; frame += 1) {
      progress = last.progress;
      last = restoreStep(0.5, box(1000), progress, frame * 16);
      expect(last.done).toBe(false);
    }
    last = restoreStep(0.5, box(1000), last.progress, STABLE_FRAMES * 16);
    expect(last.done).toBe(true);
  });

  it("restarts the count when the height changes again", () => {
    let progress: RestoreProgress = RESTORE_START;
    for (const height of [1000, 1000, 1000]) {
      progress = restoreStep(0.5, box(height), progress, 0).progress;
    }
    const step = restoreStep(0.5, box(1200), progress, 0);
    expect(step.progress.stableFrames).toBe(0);
    expect(step.done).toBe(false);
  });

  it("gives up at the time limit even if the box never scrolls", () => {
    expect(restoreStep(0.5, box(400), RESTORE_START, RESTORE_LIMIT_MS).done).toBe(true);
  });
});
