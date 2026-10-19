import { describe, expect, it } from "vitest";
import {
  OVERSCAN_ROWS,
  ROW_HEIGHT,
  ROWS_BOTH_OPEN,
  ROWS_ONE_OPEN,
  rowBudget,
  shownRows,
  visibleWindow,
} from "./explorerWindow";

describe("the height budget", () => {
  it("gives six rows to each list when both are open", () => {
    expect(rowBudget({ documents: true, tables: true })).toStrictEqual({
      documents: ROWS_BOTH_OPEN,
      tables: ROWS_BOTH_OPEN,
    });
    expect(ROWS_BOTH_OPEN).toBe(6);
  });

  it("gives twelve rows to the one list that is open", () => {
    expect(rowBudget({ documents: true, tables: false })).toStrictEqual({
      documents: ROWS_ONE_OPEN,
      tables: 0,
    });
    expect(rowBudget({ documents: false, tables: true })).toStrictEqual({
      documents: 0,
      tables: ROWS_ONE_OPEN,
    });
    expect(ROWS_ONE_OPEN).toBe(12);
  });

  it("leaves only the headers when both are folded", () => {
    expect(rowBudget({ documents: false, tables: false })).toStrictEqual({
      documents: 0,
      tables: 0,
    });
  });

  it("does not pad a short list up to the budget", () => {
    expect(shownRows(6, 3)).toBe(3);
    expect(shownRows(6, 1000)).toBe(6);
    expect(shownRows(6, 0)).toBe(0);
    expect(shownRows(0, 10)).toBe(0);
  });

  it("knows the two densities", () => {
    expect(ROW_HEIGHT).toStrictEqual({ compact: 24, comfortable: 32 });
  });
});

describe("the window of rows drawn", () => {
  it.each([24, 32])("covers the whole list with its padding, at any position (rows of %i px)", (rowHeight) => {
    const total = 1000;
    const viewport = 6 * rowHeight;

    for (const scrollTop of [0, 1, rowHeight - 1, rowHeight, 777, 5000, total * rowHeight, 1e9, -50]) {
      const slice = visibleWindow(total, rowHeight, scrollTop, viewport);

      expect(slice.first).toBeGreaterThanOrEqual(0);
      expect(slice.first + slice.count).toBeLessThanOrEqual(total);
      expect(slice.before + slice.count * rowHeight + slice.after).toBe(total * rowHeight);
      expect(slice.before).toBe(slice.first * rowHeight);
      // Only a few dozen rows are ever drawn, never the thousand.
      expect(slice.count).toBeLessThanOrEqual(6 + 2 * OVERSCAN_ROWS + 1);
    }
  });

  it("draws every visible row, at any position", () => {
    const rowHeight = 24;
    const viewport = 144;

    for (let scrollTop = 0; scrollTop <= 1000 * rowHeight - viewport; scrollTop += 37) {
      const slice = visibleWindow(1000, rowHeight, scrollTop, viewport);
      const firstVisible = Math.floor(scrollTop / rowHeight);
      const lastVisible = Math.ceil((scrollTop + viewport) / rowHeight) - 1;

      expect(slice.first).toBeLessThanOrEqual(firstVisible);
      expect(slice.first + slice.count - 1).toBeGreaterThanOrEqual(lastVisible);
    }
  });

  it("starts at the first row at the top and ends at the last row at the bottom", () => {
    const top = visibleWindow(50, 24, 0, 144);
    expect(top.first).toBe(0);
    expect(top.before).toBe(0);

    const bottom = visibleWindow(50, 24, 50 * 24 - 144, 144);
    expect(bottom.first + bottom.count).toBe(50);
    expect(bottom.after).toBe(0);
  });

  it("draws a short list whole", () => {
    const slice = visibleWindow(3, 24, 0, 72);

    expect(slice).toStrictEqual({ first: 0, count: 3, before: 0, after: 0 });
  });

  it("draws nothing for an empty list or a row of no height", () => {
    expect(visibleWindow(0, 24, 0, 144)).toStrictEqual({ first: 0, count: 0, before: 0, after: 0 });
    expect(visibleWindow(10, 0, 0, 144)).toStrictEqual({ first: 0, count: 0, before: 0, after: 0 });
  });

  it("clamps a stale position past the end after the list shrank", () => {
    const slice = visibleWindow(20, 24, 100_000, 144);

    expect(slice.first + slice.count).toBe(20);
    expect(slice.after).toBe(0);
  });

  it("works with the overscan switched off", () => {
    const slice = visibleWindow(100, 24, 48, 72, 0);

    expect(slice).toStrictEqual({ first: 2, count: 3, before: 48, after: 95 * 24 });
  });
});
