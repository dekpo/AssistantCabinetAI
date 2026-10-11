import { describe, expect, it } from "vitest";
import {
  addScopeFiles,
  combineScopes,
  combinedHasSelection,
  noDocumentsScope,
  scopedPaths,
  selectionOf,
  tickedPaths,
  toggleAll,
  toggleScopeFile,
  wholeFolderScope,
  documentsSelected,
} from "./analysisScope";
import type { FileRecord } from "./ipc";

function file(relativePath: string, id: string): FileRecord {
  return { id, relativePath } as FileRecord;
}

/* The Data Folder's selection is the Documents Folder's, cloned: the same tri-state, the same
   "unticking the last file returns to none" rule. Every case below runs for both lists. */
describe.each([
  { list: "document", names: ["a.pdf", "b.pdf", "c.pdf"] },
  { list: "data", names: ["a.csv", "b.xlsx", "c.csv"] },
])("$list selection", ({ names }) => {
  const [nameA, nameB, nameC] = names as [string, string, string];
  const A = file(nameA, "id-a");
  const B = file(nameB, "id-b");
  const C = file(nameC, "id-c");
  const ANALYSED = [A, B, C];

  it("starts with nothing selected", () => {
    const scope = noDocumentsScope(10);

    expect(scope.mode).toStrictEqual({ kind: "explicit", entries: [] });
    expect(selectionOf(scope)).toBe("none");
    expect(tickedPaths(scope, ANALYSED)).toStrictEqual([]);
  });

  it("pins the id of a file when it is ticked", () => {
    const scope = toggleScopeFile(noDocumentsScope(10), A, ANALYSED, 20);

    expect(scope.mode).toStrictEqual({
      kind: "explicit",
      entries: [{ relativePath: nameA, pinnedId: "id-a", addedAt: 20 }],
    });
    expect(selectionOf(scope)).toBe("some");
    expect(scope.createdAt).toBe(10);
    expect(scope.updatedAt).toBe(20);
  });

  it("keeps the other files when one is unticked", () => {
    let scope = toggleScopeFile(noDocumentsScope(10), A, ANALYSED, 20);
    scope = toggleScopeFile(scope, B, ANALYSED, 30);
    scope = toggleScopeFile(scope, A, ANALYSED, 40);

    expect(scopedPaths(scope)).toStrictEqual([nameB]);
  });

  it("returns to none, never to the whole folder, when the last file is unticked", () => {
    let scope = toggleScopeFile(noDocumentsScope(10), A, ANALYSED, 20);
    scope = toggleScopeFile(scope, A, ANALYSED, 30);

    expect(selectionOf(scope)).toBe("none");
    expect(scope.createdAt).toBe(10);
    expect(scope.updatedAt).toBe(30);
  });

  it("becomes the whole folder once every analysed file is ticked", () => {
    let scope = noDocumentsScope(10);
    for (const each of ANALYSED) {
      scope = toggleScopeFile(scope, each, ANALYSED, 20);
    }

    expect(scope.mode).toStrictEqual({ kind: "whole_folder" });
    expect(tickedPaths(scope, ANALYSED)).toStrictEqual([nameA, nameB, nameC]);
  });

  it("keeps every other analysed file, freshly pinned, when one is unticked from the whole folder", () => {
    const scope = toggleScopeFile(wholeFolderScope(10), B, ANALYSED, 50);

    expect(scope.mode).toStrictEqual({
      kind: "explicit",
      entries: [
        { relativePath: nameA, pinnedId: "id-a", addedAt: 50 },
        { relativePath: nameC, pinnedId: "id-c", addedAt: 50 },
      ],
    });
  });

  it("selects every file from none or some, and none from all", () => {
    const some = toggleScopeFile(noDocumentsScope(10), A, ANALYSED, 20);

    expect(selectionOf(toggleAll(noDocumentsScope(10), 30))).toBe("all");
    expect(selectionOf(toggleAll(some, 30))).toBe("all");
    expect(selectionOf(toggleAll(wholeFolderScope(10), 30))).toBe("none");
    expect(toggleAll(some, 30).createdAt).toBe(10);
  });

  it("does not call a single ticked file the whole folder when nothing else is analysed yet", () => {
    // With one analysed file, ticking it is ticking every analysed file.
    expect(selectionOf(toggleScopeFile(noDocumentsScope(10), A, [A], 20))).toBe("all");
    // With none analysed there is nothing "every" could mean, so a stray tick stays a choice.
    expect(selectionOf(toggleScopeFile(noDocumentsScope(10), A, [], 20))).toBe("some");
  });
});

describe("combining the two lists", () => {
  it("sends the documents as the mode and the workbooks as the data mode", () => {
    const documents = toggleScopeFile(noDocumentsScope(10), file("a.pdf", "id-a"), [], 20);
    const data = wholeFolderScope(5);

    const combined = combineScopes(documents, data);

    expect(combined.mode).toStrictEqual(documents.mode);
    expect(combined.dataMode).toStrictEqual({ kind: "whole_folder" });
    expect(combined.createdAt).toBe(5);
    expect(combined.updatedAt).toBe(20);
  });

  it("starts with no document and no table", () => {
    const combined = combineScopes(noDocumentsScope(1), noDocumentsScope(1));

    expect(combined.mode).toStrictEqual({ kind: "explicit", entries: [] });
    expect(combined.dataMode).toStrictEqual({ kind: "explicit", entries: [] });
  });
});

/* "Add to my selection" under the name picker: several files at once, with the ticking rules the
   listing's checkboxes already follow. Both lists behave the same. */
describe.each([
  { list: "document", names: ["a.pdf", "b.pdf", "c.pdf", "d.pdf"] },
  { list: "data", names: ["a.csv", "b.xlsx", "c.csv", "d.csv"] },
])("adding files to the $list selection", ({ names }) => {
  const [nameA, nameB, nameC, nameD] = names as [string, string, string, string];
  const A = file(nameA, "id-a");
  const B = file(nameB, "id-b");
  const C = file(nameC, "id-c");
  const D = file(nameD, "id-d");
  const ANALYSED = [A, B, C, D];

  it("adds the files after the ones already ticked, each pinned to its content", () => {
    const start = toggleScopeFile(noDocumentsScope(10), A, ANALYSED, 20);

    const next = addScopeFiles(start, [C, B], ANALYSED, 30);

    expect(next.mode).toStrictEqual({
      kind: "explicit",
      entries: [
        { relativePath: nameA, pinnedId: "id-a", addedAt: 20 },
        { relativePath: nameC, pinnedId: "id-c", addedAt: 30 },
        { relativePath: nameB, pinnedId: "id-b", addedAt: 30 },
      ],
    });
    expect(next.createdAt).toBe(10);
    expect(next.updatedAt).toBe(30);
  });

  it("does not add a file twice, and changes nothing when everything is already ticked", () => {
    const start = toggleScopeFile(toggleScopeFile(noDocumentsScope(10), A, ANALYSED, 20), B, ANALYSED, 21);

    expect(addScopeFiles(start, [B, A], ANALYSED, 40)).toBe(start);
    expect(scopedPaths(addScopeFiles(start, [C, C], ANALYSED, 40))).toStrictEqual([
      nameA,
      nameB,
      nameC,
    ]);
  });

  it("becomes the whole folder when it completes the analysed files", () => {
    const start = toggleScopeFile(noDocumentsScope(10), A, ANALYSED, 20);

    const next = addScopeFiles(start, [B, C, D], ANALYSED, 30);

    expect(next.mode).toStrictEqual({ kind: "whole_folder" });
  });

  it("leaves the whole folder as it is", () => {
    const all = wholeFolderScope(10);

    expect(addScopeFiles(all, [A], ANALYSED, 30)).toBe(all);
  });

  it("starts a selection from none", () => {
    const next = addScopeFiles(noDocumentsScope(10), [B], ANALYSED, 30);

    expect(selectionOf(next)).toBe("some");
    expect(scopedPaths(next)).toStrictEqual([nameB]);
  });

  it("adds nothing for no file", () => {
    const none = noDocumentsScope(10);

    expect(addScopeFiles(none, [], ANALYSED, 30)).toBe(none);
  });
});

describe("whether anything is selected", () => {
  it("is false only when both lists are empty", () => {
    const some = toggleScopeFile(noDocumentsScope(1), file("a.pdf", "id-a"), [], 2);

    expect(combinedHasSelection(combineScopes(noDocumentsScope(1), noDocumentsScope(1)))).toBe(false);
    expect(combinedHasSelection(combineScopes(some, noDocumentsScope(1)))).toBe(true);
    expect(combinedHasSelection(combineScopes(noDocumentsScope(1), some))).toBe(true);
    expect(combinedHasSelection(combineScopes(wholeFolderScope(1), noDocumentsScope(1)))).toBe(true);
  });
});


describe("which card shows the suggestions", () => {
  const base = { createdAt: 1, updatedAt: 1 };

  it("is the documents card as soon as a document is selected, whatever the data side holds", () => {
    expect(
      documentsSelected({
        ...base,
        mode: { kind: "whole_folder" },
        dataMode: { kind: "explicit", entries: [] },
      }),
    ).toBe(true);
    expect(
      documentsSelected({
        ...base,
        mode: { kind: "explicit", entries: [{ relativePath: "a.txt", pinnedId: "x", addedAt: 1 }] },
        dataMode: { kind: "whole_folder" },
      }),
    ).toBe(true);
  });

  it("is the data card when no document is selected", () => {
    expect(
      documentsSelected({
        ...base,
        mode: { kind: "explicit", entries: [] },
        dataMode: { kind: "whole_folder" },
      }),
    ).toBe(false);
  });
});
