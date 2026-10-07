import { describe, expect, it } from "vitest";
import {
  combineScopes,
  noDocumentsScope,
  scopedPaths,
  selectionOf,
  tickedPaths,
  toggleAll,
  toggleScopeFile,
  wholeFolderScope,
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
