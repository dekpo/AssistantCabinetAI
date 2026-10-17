import { describe, expect, it } from "vitest";
import type { PanelLayouts } from "./ipc";
import {
  DEFAULT_COLLAPSED_TRIPLE,
  DEFAULT_OPEN_TRIPLE,
  NO_LAYOUTS,
  isCentreOpen,
  layoutToApply,
  layoutToTriple,
  sameLayouts,
  savedTriple,
  storeLayout,
  tripleToLayout,
} from "./panelLayout";

describe("the layout shapes", () => {
  it("goes from the library's object to a triple and back", () => {
    const layout = { resources: 20, workspace: 40, chat: 40 };

    expect(layoutToTriple(layout)).toEqual([20, 40, 40]);
    expect(tripleToLayout([20, 40, 40])).toEqual(layout);
  });

  it("refuses a layout that lacks a panel or holds nonsense", () => {
    expect(layoutToTriple({ resources: 20, chat: 80 })).toBeNull();
    expect(layoutToTriple({ resources: 20, workspace: Number.NaN, chat: 80 })).toBeNull();
    expect(layoutToTriple({ resources: -1, workspace: 21, chat: 80 })).toBeNull();
    expect(layoutToTriple({ resources: 20, workspace: 40, chat: 140 })).toBeNull();
  });

  it("calls the centre open only when it takes room", () => {
    expect(isCentreOpen({ resources: 20, workspace: 40, chat: 40 })).toBe(true);
    expect(isCentreOpen({ resources: 25, workspace: 0, chat: 75 })).toBe(false);
    expect(isCentreOpen({ resources: 25, workspace: 0.00001, chat: 75 })).toBe(false);
    expect(isCentreOpen({ resources: 25, chat: 75 })).toBe(false);
  });
});

describe("which saved triple applies", () => {
  const layouts: PanelLayouts = { open: [18, 42, 40], collapsed: [30, 0, 70] };

  it("takes the open triple when the centre is shown and the collapsed one when it is not", () => {
    expect(savedTriple(layouts, true)).toEqual([18, 42, 40]);
    expect(savedTriple(layouts, false)).toEqual([30, 0, 70]);
    expect(layoutToApply(layouts, true)).toEqual({ resources: 18, workspace: 42, chat: 40 });
    expect(layoutToApply(layouts, false)).toEqual({ resources: 30, workspace: 0, chat: 70 });
  });

  it("gives none, and the default layout, when nothing usable is saved", () => {
    expect(savedTriple(NO_LAYOUTS, true)).toBeNull();
    expect(savedTriple(NO_LAYOUTS, false)).toBeNull();
    expect(layoutToApply(NO_LAYOUTS, true)).toEqual(tripleToLayout(DEFAULT_OPEN_TRIPLE));
    expect(layoutToApply(NO_LAYOUTS, false)).toEqual(tripleToLayout(DEFAULT_COLLAPSED_TRIPLE));
  });

  it("does not trust a triple of the wrong state or with a bad value", () => {
    expect(savedTriple({ open: [25, 0, 75], collapsed: null }, true)).toBeNull();
    expect(savedTriple({ open: null, collapsed: [20, 40, 40] }, false)).toBeNull();
    expect(savedTriple({ open: [Number.NaN, 40, 60], collapsed: null }, true)).toBeNull();
    expect(savedTriple({ open: [20, 40], collapsed: null } as never, true)).toBeNull();
  });

  it("has defaults that are valid for their own state", () => {
    expect(DEFAULT_OPEN_TRIPLE.reduce((a, b) => a + b, 0)).toBe(100);
    expect(DEFAULT_COLLAPSED_TRIPLE.reduce((a, b) => a + b, 0)).toBe(100);
    expect(DEFAULT_OPEN_TRIPLE[1]).toBeGreaterThan(0);
    expect(DEFAULT_COLLAPSED_TRIPLE[1]).toBe(0);
  });
});

describe("what is stored after the user resized", () => {
  it("replaces the open triple while the centre is open and leaves the other alone", () => {
    const before: PanelLayouts = { open: [20, 40, 40], collapsed: [25, 0, 75] };
    const after = storeLayout(before, { resources: 24, workspace: 36, chat: 40 });

    expect(after).toEqual({ open: [24, 36, 40], collapsed: [25, 0, 75] });
  });

  it("stores a collapsed centre as exactly zero", () => {
    const after = storeLayout(NO_LAYOUTS, { resources: 30, workspace: 0.00001, chat: 69.99999 });

    expect(after).toEqual({ open: null, collapsed: [30, 0, 69.99999] });
  });

  it("stores nothing from an unusable layout", () => {
    expect(storeLayout(NO_LAYOUTS, { resources: 30, chat: 70 })).toBeNull();
  });

  it("compares layouts by value", () => {
    expect(sameLayouts(NO_LAYOUTS, { open: null, collapsed: null })).toBe(true);
    expect(sameLayouts({ open: [1, 2, 97], collapsed: null }, { open: [1, 2, 97], collapsed: null })).toBe(
      true,
    );
    expect(sameLayouts({ open: [1, 2, 97], collapsed: null }, NO_LAYOUTS)).toBe(false);
  });
});
