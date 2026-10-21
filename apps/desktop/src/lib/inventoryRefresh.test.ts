import { describe, expect, it } from "vitest";
import { inventoryRefresh } from "./inventoryRefresh";

const NONE = { documentsPasses: 0, dataPasses: 0, notesToken: 0, documentsClears: 0, tablesClears: 0 };

describe("when the two inventories are read again", () => {
  it("does nothing before any pass", () => {
    expect(inventoryRefresh(NONE)).toStrictEqual({ passes: 0, token: 0 });
  });

  it("moves both inventories' counters when the tabular pass finishes", () => {
    // The workbooks of the Documents folder are in the Documents inventory: it must be read again too.
    const before = inventoryRefresh(NONE);
    const after = inventoryRefresh({ ...NONE, dataPasses: 1 });

    expect(after.passes).toBeGreaterThan(before.passes);
  });

  it("moves both inventories' counters when the document pass finishes", () => {
    const after = inventoryRefresh({ ...NONE, documentsPasses: 1 });

    expect(after.passes).toBeGreaterThan(0);
  });

  it("moves the token when either family's analysis is cleared, or a note is saved", () => {
    for (const change of [{ documentsClears: 1 }, { tablesClears: 1 }, { notesToken: 1 }]) {
      expect(inventoryRefresh({ ...NONE, ...change }).token).toBeGreaterThan(0);
    }
  });

  it("changes at every new pass, so each one is read again", () => {
    const first = inventoryRefresh({ ...NONE, documentsPasses: 1, dataPasses: 1 });
    const second = inventoryRefresh({ ...NONE, documentsPasses: 2, dataPasses: 1 });

    expect(second.passes).not.toBe(first.passes);
  });
});
