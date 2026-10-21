import { describe, expect, it } from "vitest";
import { menuHeight, menuPlacement, nextMenuIndex } from "./explorerMenu";

const viewport = { width: 1180, height: 780 };

describe("where the menu opens", () => {
  it("opens under the button, right-aligned with it", () => {
    const placement = menuPlacement({ top: 100, bottom: 124, right: 320 }, viewport, 3);

    expect(placement).toEqual({ top: 126, right: 860 });
  });

  it("opens above the button when there is no room below and there is above", () => {
    const placement = menuPlacement({ top: 700, bottom: 724, right: 320 }, viewport, 3);

    expect(placement.top).toBe(700 - 2 - menuHeight(3));
  });

  it("stays under the button when there is room neither below nor above", () => {
    const placement = menuPlacement({ top: 10, bottom: 34, right: 320 }, { width: 1180, height: 80 }, 3);

    expect(placement.top).toBe(36);
  });

  it("never gives a negative distance from the right edge", () => {
    expect(menuPlacement({ top: 0, bottom: 20, right: 2000 }, viewport, 3).right).toBe(0);
  });
});

describe("moving between the entries by keyboard", () => {
  it("wraps with the arrows", () => {
    expect(nextMenuIndex(0, 3, "ArrowDown")).toBe(1);
    expect(nextMenuIndex(2, 3, "ArrowDown")).toBe(0);
    expect(nextMenuIndex(0, 3, "ArrowUp")).toBe(2);
  });

  it("goes to the first and the last with Home and End", () => {
    expect(nextMenuIndex(1, 3, "Home")).toBe(0);
    expect(nextMenuIndex(1, 3, "End")).toBe(2);
  });

  it("ignores any other key and an empty menu", () => {
    expect(nextMenuIndex(0, 3, "a")).toBeNull();
    expect(nextMenuIndex(-1, 0, "ArrowDown")).toBeNull();
  });

  it("starts from the first entry when none has the focus", () => {
    expect(nextMenuIndex(-1, 3, "ArrowDown")).toBe(0);
  });
});
