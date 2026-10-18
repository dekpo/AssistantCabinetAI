import { describe, expect, it } from "vitest";
import { barHeights, formatClock } from "./waveform";

describe("the bars of the recording pill", () => {
  it("rest at their minimum in silence, and never disappear", () => {
    expect(barHeights(0).every((height) => height === 12)).toBe(true);
  });

  it("reach the full height in the middle at the loudest level", () => {
    const heights = barHeights(1);

    expect(Math.max(...heights)).toBe(100);
    expect(heights[3]).toBe(100);
    expect(heights[0]).toBeLessThan(heights[3]!);
  });

  it("grow with the level and stay symmetric", () => {
    const quiet = barHeights(0.2);
    const loud = barHeights(0.8);

    expect(loud.every((height, index) => height > quiet[index]!)).toBe(true);
    expect(loud).toStrictEqual([...loud].reverse());
  });

  it("read a level that is out of range or not a number as the nearest valid one", () => {
    expect(barHeights(5)).toStrictEqual(barHeights(1));
    expect(barHeights(-1)).toStrictEqual(barHeights(0));
    expect(barHeights(Number.NaN)).toStrictEqual(barHeights(0));
  });
});

describe("the clock of the recording pill", () => {
  it("writes minutes and two-digit seconds", () => {
    expect(formatClock(0)).toBe("0:00");
    expect(formatClock(7_900)).toBe("0:07");
    expect(formatClock(65_000)).toBe("1:05");
    expect(formatClock(-5)).toBe("0:00");
  });
});
