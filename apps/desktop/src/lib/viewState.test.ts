import { describe, expect, it } from "vitest";
import { FRESH_VIEW_STATE, clampRatio, createViewStateStore, type TabViewState } from "./viewState";

const state = (patch: Partial<TabViewState>): TabViewState => ({ ...FRESH_VIEW_STATE, ...patch });

describe("clampRatio", () => {
  it("keeps a ratio inside 0 to 1", () => {
    expect(clampRatio(-0.5)).toBe(0);
    expect(clampRatio(0.4)).toBe(0.4);
    expect(clampRatio(3)).toBe(1);
  });

  it("takes anything that is not a number for the top", () => {
    expect(clampRatio(Number.NaN)).toBe(0);
    expect(clampRatio(Number.POSITIVE_INFINITY)).toBe(0);
  });
});

describe("the view state store", () => {
  it("gives back what was saved, for the same content", () => {
    const store = createViewStateStore();
    store.save("a", "h1", state({ scrollRatio: 0.4, zoom: 1.5, sheetIndex: 2, mode: "preview" }));
    expect(store.recall("a", "h1")).toEqual(
      state({ scrollRatio: 0.4, zoom: 1.5, sheetIndex: 2, mode: "preview" }),
    );
  });

  it("knows nothing of a tab it never saw", () => {
    expect(createViewStateStore().recall("a", "h1")).toBeUndefined();
  });

  it("does not consume an entry when it is read (every effect runs twice in development)", () => {
    const store = createViewStateStore();
    store.save("a", "h1", state({ scrollRatio: 0.5 }));
    store.recall("a", "h1");
    expect(store.recall("a", "h1")?.scrollRatio).toBe(0.5);
  });

  it("drops the state of a file whose content is another", () => {
    const store = createViewStateStore();
    store.save("a", "h1", state({ scrollRatio: 0.5 }));
    expect(store.recall("a", "h2")).toBeUndefined();
  });

  it("clamps the ratio it stores", () => {
    const store = createViewStateStore();
    store.save("a", "h1", state({ scrollRatio: 7 }));
    store.save("b", "h1", state({ scrollRatio: -1 }));
    expect(store.recall("a", "h1")?.scrollRatio).toBe(1);
    expect(store.recall("b", "h1")?.scrollRatio).toBe(0);
  });

  it("hands out copies, so a caller cannot change what is kept", () => {
    const store = createViewStateStore();
    store.save("a", "h1", state({ scrollRatio: 0.5 }));
    const copy = store.recall("a", "h1");
    if (copy !== undefined) {
      copy.scrollRatio = 0.9;
    }
    expect(store.recall("a", "h1")?.scrollRatio).toBe(0.5);
  });

  it("clears one tab, keeps the others, and clears everything", () => {
    const store = createViewStateStore();
    store.save("a", "h", state({}));
    store.save("b", "h", state({}));
    store.clear("a");
    expect(store.recall("a", "h")).toBeUndefined();
    expect(store.recall("b", "h")).toBeDefined();
    store.clearAll();
    expect(store.recall("b", "h")).toBeUndefined();
  });

  it("retains only the tabs that are still open", () => {
    const store = createViewStateStore();
    store.save("a", "h", state({}));
    store.save("b", "h", state({}));
    store.save("c", "h", state({}));
    store.retain(["b"]);
    expect(store.recall("a", "h")).toBeUndefined();
    expect(store.recall("b", "h")).toBeDefined();
    expect(store.recall("c", "h")).toBeUndefined();
  });
});
