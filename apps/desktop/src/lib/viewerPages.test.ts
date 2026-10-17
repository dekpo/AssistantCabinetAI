import { describe, expect, it } from "vitest";
import { pageAtLine } from "./viewerPages";

/** Pages of 100 units, the first one starting at `origin`. */
const stack = (origin: number) => (index: number) => origin + index * 100;

describe("pageAtLine", () => {
  it("is page 1 for an empty list", () => {
    expect(pageAtLine(stack(0), 0, 50)).toBe(1);
  });

  it("is page 1 while no page has reached the line", () => {
    expect(pageAtLine(stack(200), 5, 50)).toBe(1);
  });

  it("is the last page whose top is at or above the line", () => {
    expect(pageAtLine(stack(0), 10, 0)).toBe(1);
    expect(pageAtLine(stack(0), 10, 99)).toBe(1);
    expect(pageAtLine(stack(0), 10, 100)).toBe(2);
    expect(pageAtLine(stack(0), 10, 450)).toBe(5);
  });

  it("follows the scroll, which moves every top up", () => {
    /* Scrolled 2 050 units down: the line at 100 is inside page 22 (index 21, top 2 100 - 2 050). */
    expect(pageAtLine(stack(-2050), 291, 100)).toBe(22);
  });

  it("stops at the last page", () => {
    expect(pageAtLine(stack(0), 291, 1_000_000)).toBe(291);
  });

  it("finds the same page as a plain scan", () => {
    for (const line of [-50, 0, 1, 99, 100, 101, 5_000, 29_099, 29_100]) {
      const scan = Array.from({ length: 291 }, (_, index) => index).filter(
        (index) => stack(0)(index) <= line,
      ).length;
      expect(pageAtLine(stack(0), 291, line)).toBe(Math.max(1, scan));
    }
  });
});
